//! SQLite persistence. All writes are transactional, attempts are append-only,
//! and answering a question updates the attempt, review/skill state, and the
//! session cursor atomically.

mod schema {
    pub const V1: &str = include_str!("schema.sql");
}

use std::path::Path;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::types::Type;
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::de::DeserializeOwned;

use crate::content::ContentBlock;
use crate::domain::{
    AnswerForm, AnswerSpec, AttemptId, CardId, Difficulty, Group, GroupId, LearningCard,
    LearningKind, Outcome, Plan, PlanId, PlanTarget, Question, QuestionId, Rating, Response,
    ReviewState, Session, SessionId, SessionItem, SessionStatus, SessionView, SkillId, SkillState,
    Tag, TagId, TagKind, Workbook, WorkbookId,
};
use crate::seed;
use crate::study::fsrs::{Fsrs6Scheduler, RecallScheduler};
use crate::study::skill::{
    SimpleSkillEstimator, SkillEstimator, SkillObservation, initial_skill_state,
};

pub const CURRENT_SCHEMA_VERSION: i64 = 1;

pub struct Repository {
    conn: Connection,
    #[cfg(test)]
    fail_after_attempt_insert: bool,
}

#[derive(Clone, Debug)]
pub struct WorkbookSummary {
    pub id: WorkbookId,
    pub title: String,
    pub description: String,
    pub card_count: usize,
    pub question_count: usize,
    pub due_count: usize,
}

#[derive(Clone, Debug)]
pub struct WorkbookDetail {
    pub workbook: Workbook,
    pub tags: Vec<Tag>,
    pub cards: Vec<CardSummary>,
    pub questions: Vec<QuestionSummary>,
}

#[derive(Clone, Debug)]
pub struct CardSummary {
    pub id: CardId,
    pub title: String,
}

#[derive(Clone, Debug)]
pub struct QuestionSummary {
    pub id: QuestionId,
    pub form: AnswerForm,
    pub kind: LearningKind,
    pub difficulty: Difficulty,
    pub preview: String,
    pub skill_id: Option<SkillId>,
    pub estimated_seconds: u32,
}

#[derive(Clone, Debug)]
pub struct Dashboard {
    pub due_recall: usize,
    pub total_questions: usize,
    pub total_workbooks: usize,
    pub active_session: Option<SessionId>,
    pub skill_count: usize,
    pub mastered_skills: usize,
}

#[derive(Clone, Debug)]
pub struct AttemptDraft {
    pub attempt_id: AttemptId,
    pub session_id: SessionId,
    pub question_id: QuestionId,
    pub response: Response,
    pub outcome: Outcome,
    pub rating: Rating,
    pub score: f32,
    pub hint_used: bool,
    pub near_miss_retry: bool,
    pub elapsed_ms: u64,
}

#[derive(Clone, Debug)]
pub enum SubmitOutcome {
    Recorded {
        attempt_id: AttemptId,
        first_exposure: bool,
        rating: Rating,
        due_at: Option<DateTime<Utc>>,
        skill_mastery: Option<f32>,
    },
    Duplicate {
        attempt_id: AttemptId,
    },
    AlreadyAnswered {
        attempt_id: Option<AttemptId>,
    },
}

impl Repository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create data directory {}", parent.display()))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("open SQLite database {}", path.display()))?;
        Self::from_connection(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::from_connection(conn)
    }

    fn from_connection(mut conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let _: String = conn.query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))?;
        migrate(&mut conn)?;
        Ok(Self {
            conn,
            #[cfg(test)]
            fail_after_attempt_insert: false,
        })
    }

    pub fn schema_version(&self) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )?)
    }

    pub fn seed_sample_content(&mut self) -> Result<()> {
        let tx = self.conn.transaction()?;
        seed::seed_sample_content(&tx)?;
        tx.commit()?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Reads
    // ------------------------------------------------------------------

    pub fn list_workbooks(&self) -> Result<Vec<WorkbookSummary>> {
        let now = fmt_time(Utc::now());
        let mut statement = self.conn.prepare(
            "SELECT w.id, w.title, w.description,
                    (SELECT count(*) FROM learning_cards c WHERE c.workbook_id = w.id),
                    (SELECT count(*) FROM questions q WHERE q.workbook_id = w.id),
                    (SELECT count(*) FROM questions q
                       JOIN review_states r ON r.question_id = q.id
                      WHERE q.workbook_id = w.id AND q.kind = 'recall' AND r.due_at <= ?1)
               FROM workbooks w
              ORDER BY w.id",
        )?;
        let rows = statement.query_map(params![now], |row| {
            Ok(WorkbookSummary {
                id: WorkbookId::new(row.get::<_, String>(0)?),
                title: row.get(1)?,
                description: row.get(2)?,
                card_count: row.get::<_, i64>(3)? as usize,
                question_count: row.get::<_, i64>(4)? as usize,
                due_count: row.get::<_, i64>(5)? as usize,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_workbook(&self, id: &WorkbookId) -> Result<Option<Workbook>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, title, description, created_at FROM workbooks WHERE id = ?1",
                params![id.as_str()],
                |row| {
                    Ok(Workbook {
                        id: WorkbookId::new(row.get::<_, String>(0)?),
                        title: row.get(1)?,
                        description: row.get(2)?,
                        created_at: sql_time(&row.get::<_, String>(3)?)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn workbook_detail(&self, id: &WorkbookId) -> Result<Option<WorkbookDetail>> {
        let Some(workbook) = self.get_workbook(id)? else {
            return Ok(None);
        };
        let tags = {
            let mut statement = self.conn.prepare(
                "SELECT t.id, t.kind, t.name
                   FROM tags t JOIN workbook_tags wt ON wt.tag_id = t.id
                  WHERE wt.workbook_id = ?1 ORDER BY t.id",
            )?;
            let rows = statement.query_map(params![id.as_str()], |row| {
                Ok(Tag {
                    id: TagId::new(row.get::<_, String>(0)?),
                    kind: TagKind::parse(&row.get::<_, String>(1)?).unwrap_or(TagKind::Other),
                    name: row.get(2)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        let cards = {
            let mut statement = self.conn.prepare(
                "SELECT id, title FROM learning_cards WHERE workbook_id = ?1 ORDER BY id",
            )?;
            let rows = statement.query_map(params![id.as_str()], |row| {
                Ok(CardSummary {
                    id: CardId::new(row.get::<_, String>(0)?),
                    title: row.get(1)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        let questions = {
            let mut statement = self.conn.prepare(
                "SELECT id, form, kind, difficulty, prompt_json, skill_id, estimated_seconds
                   FROM questions WHERE workbook_id = ?1 ORDER BY id",
            )?;
            let rows = statement.query_map(params![id.as_str()], |row| {
                let prompt: Vec<ContentBlock> =
                    serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default();
                let preview = prompt
                    .first()
                    .map(ContentBlock::plain_text)
                    .unwrap_or_default();
                Ok(QuestionSummary {
                    id: QuestionId::new(row.get::<_, String>(0)?),
                    form: AnswerForm::parse(&row.get::<_, String>(1)?)
                        .unwrap_or(AnswerForm::ShortAnswer),
                    kind: LearningKind::parse(&row.get::<_, String>(2)?)
                        .unwrap_or(LearningKind::Recall),
                    difficulty: Difficulty::parse(&row.get::<_, String>(3)?)
                        .unwrap_or(Difficulty::Beginner),
                    preview: truncate_chars(&preview, 96),
                    skill_id: row.get::<_, Option<String>>(5)?.map(SkillId::new),
                    estimated_seconds: row.get::<_, i64>(6)? as u32,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        Ok(Some(WorkbookDetail {
            workbook,
            tags,
            cards,
            questions,
        }))
    }

    pub fn get_question(&self, id: &QuestionId) -> Result<Option<Question>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, workbook_id, form, kind, difficulty, prompt_json, answer_json,
                        explanation_json, hint, card_id, skill_id, tags_json,
                        estimated_seconds, created_at
                   FROM questions WHERE id = ?1",
                params![id.as_str()],
                row_to_question,
            )
            .optional()?)
    }

    pub fn get_card(&self, id: &CardId) -> Result<Option<LearningCard>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, workbook_id, title, blocks_json, created_at
                   FROM learning_cards WHERE id = ?1",
                params![id.as_str()],
                |row| {
                    Ok(LearningCard {
                        id: CardId::new(row.get::<_, String>(0)?),
                        workbook_id: WorkbookId::new(row.get::<_, String>(1)?),
                        title: row.get(2)?,
                        blocks: sql_json(&row.get::<_, String>(3)?)?,
                        created_at: sql_time(&row.get::<_, String>(4)?)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn dashboard(&self) -> Result<Dashboard> {
        let now = fmt_time(Utc::now());
        let due_recall = self.conn.query_row(
            "SELECT count(*) FROM questions q JOIN review_states r ON r.question_id = q.id
              WHERE q.kind = 'recall' AND r.due_at <= ?1",
            params![now],
            |row| row.get::<_, i64>(0),
        )? as usize;
        let total_questions = self
            .conn
            .query_row("SELECT count(*) FROM questions", [], |row| {
                row.get::<_, i64>(0)
            })? as usize;
        let total_workbooks = self
            .conn
            .query_row("SELECT count(*) FROM workbooks", [], |row| {
                row.get::<_, i64>(0)
            })? as usize;
        let active_session = self
            .conn
            .query_row(
                "SELECT id FROM sessions WHERE status = 'active' ORDER BY started_at DESC LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(SessionId::new);
        let skill_count = self
            .conn
            .query_row("SELECT count(*) FROM skill_states", [], |row| {
                row.get::<_, i64>(0)
            })? as usize;
        let mastered_skills = self.conn.query_row(
            "SELECT count(*) FROM skill_states WHERE mastery >= 0.6",
            [],
            |row| row.get::<_, i64>(0),
        )? as usize;
        Ok(Dashboard {
            due_recall,
            total_questions,
            total_workbooks,
            active_session,
            skill_count,
            mastered_skills,
        })
    }

    pub fn active_session(&self) -> Result<Option<SessionView>> {
        let id = self
            .conn
            .query_row(
                "SELECT id FROM sessions WHERE status = 'active' ORDER BY started_at DESC LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        match id {
            Some(id) => self.load_session(&SessionId::new(id)),
            None => Ok(None),
        }
    }

    pub fn load_session(&self, id: &SessionId) -> Result<Option<SessionView>> {
        let session = self
            .conn
            .query_row(
                "SELECT s.id, s.workbook_id, s.budget_minutes, s.status, s.started_at,
                        s.ended_at, s.current_index, COALESCE(w.title, 'すべてのWorkbook')
                   FROM sessions s LEFT JOIN workbooks w ON w.id = s.workbook_id
                  WHERE s.id = ?1",
                params![id.as_str()],
                |row| {
                    Ok((
                        Session {
                            id: SessionId::new(row.get::<_, String>(0)?),
                            workbook_id: row.get::<_, Option<String>>(1)?.map(WorkbookId::new),
                            budget_minutes: row.get::<_, i64>(2)? as u32,
                            status: SessionStatus::parse(&row.get::<_, String>(3)?)
                                .unwrap_or(SessionStatus::Active),
                            started_at: sql_time(&row.get::<_, String>(4)?)?,
                            ended_at: match row.get::<_, Option<String>>(5)? {
                                Some(value) => Some(sql_time(&value)?),
                                None => None,
                            },
                            current_index: row.get::<_, i64>(6)? as usize,
                        },
                        row.get::<_, String>(7)?,
                    ))
                },
            )
            .optional()?;
        let Some((session, workbook_title)) = session else {
            return Ok(None);
        };
        let items = {
            let mut statement = self.conn.prepare(
                "SELECT session_id, question_id, position, show_card, card_seen, answered,
                        attempt_id, selection_reason
                   FROM session_items WHERE session_id = ?1 ORDER BY position",
            )?;
            let rows = statement.query_map(params![id.as_str()], |row| {
                Ok(SessionItem {
                    session_id: SessionId::new(row.get::<_, String>(0)?),
                    question_id: QuestionId::new(row.get::<_, String>(1)?),
                    position: row.get::<_, i64>(2)? as usize,
                    show_card: row.get::<_, i64>(3)? != 0,
                    card_seen: row.get::<_, i64>(4)? != 0,
                    answered: row.get::<_, i64>(5)? != 0,
                    attempt_id: row.get::<_, Option<String>>(6)?.map(AttemptId::new),
                    selection_reason: row.get(7)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        Ok(Some(SessionView {
            session,
            workbook_title,
            items,
        }))
    }

    pub fn review_state(&self, question_id: &QuestionId) -> Result<Option<ReviewState>> {
        Ok(self
            .conn
            .query_row(
                "SELECT question_id, stability, difficulty, due_at, reps, lapses,
                        last_rating, last_reviewed_at
                   FROM review_states WHERE question_id = ?1",
                params![question_id.as_str()],
                |row| {
                    Ok(ReviewState {
                        question_id: QuestionId::new(row.get::<_, String>(0)?),
                        stability: row.get::<_, f64>(1)? as f32,
                        difficulty: row.get::<_, f64>(2)? as f32,
                        due_at: sql_time(&row.get::<_, String>(3)?)?,
                        reps: row.get::<_, i64>(4)? as u32,
                        lapses: row.get::<_, i64>(5)? as u32,
                        last_rating: Rating::parse(row.get::<_, i64>(6)?).unwrap_or(Rating::Again),
                        last_reviewed_at: sql_time(&row.get::<_, String>(7)?)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn skill_state(&self, skill_id: &SkillId) -> Result<Option<SkillState>> {
        read_skill_state(&self.conn, skill_id)
    }

    pub fn attempt_count(&self, question_id: &QuestionId) -> Result<usize> {
        Ok(self.conn.query_row(
            "SELECT count(*) FROM attempts WHERE question_id = ?1",
            params![question_id.as_str()],
            |row| row.get::<_, i64>(0),
        )? as usize)
    }

    pub fn attempt_count_total(&self) -> Result<usize> {
        Ok(self
            .conn
            .query_row("SELECT count(*) FROM attempts", [], |row| {
                row.get::<_, i64>(0)
            })? as usize)
    }

    pub fn list_plans(&self) -> Result<Vec<Plan>> {
        let mut statement = self.conn.prepare(
            "SELECT id, name, target_type, target_id, due_date, daily_minutes, goal, created_at
               FROM plans ORDER BY created_at",
        )?;
        let rows = statement.query_map([], |row| {
            let target_type = row.get::<_, String>(2)?;
            let target_id = row.get::<_, Option<String>>(3)?;
            let target = match (target_type.as_str(), target_id) {
                ("workbook", Some(id)) => PlanTarget::Workbook(WorkbookId::new(id)),
                ("group", Some(id)) => PlanTarget::Group(GroupId::new(id)),
                _ => PlanTarget::All,
            };
            Ok(Plan {
                id: PlanId::new(row.get::<_, String>(0)?),
                name: row.get(1)?,
                target,
                due_date: match row.get::<_, Option<String>>(4)? {
                    Some(value) => Some(sql_time(&value)?),
                    None => None,
                },
                daily_minutes: row.get::<_, i64>(5)? as u32,
                goal: row.get(6)?,
                created_at: sql_time(&row.get::<_, String>(7)?)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn list_groups(&self) -> Result<Vec<Group>> {
        let mut statement = self
            .conn
            .prepare("SELECT id, name, description, created_at FROM groups ORDER BY id")?;
        let base = statement
            .query_map([], |row| {
                Ok(Group {
                    id: GroupId::new(row.get::<_, String>(0)?),
                    name: row.get(1)?,
                    description: row.get(2)?,
                    workbook_ids: vec![],
                    created_at: sql_time(&row.get::<_, String>(3)?)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut groups = base;
        for group in &mut groups {
            let mut statement = self.conn.prepare(
                "SELECT workbook_id FROM group_workbooks WHERE group_id = ?1 ORDER BY position",
            )?;
            group.workbook_ids = statement
                .query_map(params![group.id.as_str()], |row| {
                    Ok(WorkbookId::new(row.get::<_, String>(0)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
        }
        Ok(groups)
    }

    // ------------------------------------------------------------------
    // Session operations
    // ------------------------------------------------------------------

    pub fn start_session(
        &mut self,
        workbook_id: Option<&WorkbookId>,
        budget_minutes: u32,
        now: DateTime<Utc>,
    ) -> Result<SessionView> {
        if budget_minutes == 0 {
            bail!("time budget must be greater than zero");
        }
        let candidates = self.plan_candidates(workbook_id)?;
        let plan = crate::study::planner::plan_session(&candidates, budget_minutes, now);
        if plan.is_empty() {
            bail!("no questions are available for this selection");
        }

        let id = SessionId::new(uuid::Uuid::new_v4().to_string());
        let stamp = fmt_time(now);
        let tx = self.conn.transaction()?;
        tx.execute(
            "UPDATE sessions SET status = 'abandoned', ended_at = ?1 WHERE status = 'active'",
            params![stamp],
        )?;
        tx.execute(
            "INSERT INTO sessions(id, workbook_id, budget_minutes, status, started_at,
                                   ended_at, current_index)
             VALUES (?1, ?2, ?3, 'active', ?4, NULL, 0)",
            params![
                id.as_str(),
                workbook_id.map(WorkbookId::as_str),
                budget_minutes as i64,
                stamp
            ],
        )?;
        for (position, selection) in plan.selections.iter().enumerate() {
            let show_card = selection.card_intro;
            tx.execute(
                "INSERT INTO session_items(session_id, question_id, position, show_card,
                                           card_seen, answered, attempt_id, selection_reason)
                 VALUES (?1, ?2, ?3, ?4, 0, 0, NULL, ?5)",
                params![
                    id.as_str(),
                    selection.question_id.as_str(),
                    position as i64,
                    i64::from(show_card),
                    selection.reason.label()
                ],
            )?;
        }
        tx.commit()?;
        self.load_session(&id)?
            .context("newly created session could not be loaded")
    }

    fn plan_candidates(
        &self,
        workbook_id: Option<&WorkbookId>,
    ) -> Result<Vec<crate::study::PlanCandidate>> {
        let mut statement = self.conn.prepare(
            "SELECT q.id, q.workbook_id, q.kind, q.difficulty, q.estimated_seconds,
                    r.due_at,
                    (SELECT max(a.submitted_at) FROM attempts a WHERE a.question_id = q.id),
                    (SELECT count(*) FROM attempts a WHERE a.question_id = q.id),
                    s.mastery,
                    (q.card_id IS NOT NULL)
               FROM questions q
               LEFT JOIN review_states r ON r.question_id = q.id
               LEFT JOIN skill_states s ON s.skill_id = q.skill_id
              WHERE (?1 IS NULL OR q.workbook_id = ?1)",
        )?;
        let rows = statement.query_map(params![workbook_id.map(WorkbookId::as_str)], |row| {
            let kind =
                LearningKind::parse(&row.get::<_, String>(2)?).unwrap_or(LearningKind::Recall);
            let difficulty =
                Difficulty::parse(&row.get::<_, String>(3)?).unwrap_or(Difficulty::Beginner);
            Ok(crate::study::PlanCandidate {
                question_id: QuestionId::new(row.get::<_, String>(0)?),
                workbook_id: WorkbookId::new(row.get::<_, String>(1)?),
                kind,
                difficulty,
                estimated_seconds: row.get::<_, i64>(4)? as u32,
                due_at: match row.get::<_, Option<String>>(5)? {
                    Some(value) => Some(sql_time(&value)?),
                    None => None,
                },
                last_attempt_at: match row.get::<_, Option<String>>(6)? {
                    Some(value) => Some(sql_time(&value)?),
                    None => None,
                },
                attempts: row.get::<_, i64>(7)? as u32,
                skill_mastery: row.get::<_, Option<f64>>(8)?.map(|value| value as f32),
                has_card: row.get::<_, i64>(9)? != 0,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn mark_card_seen(
        &mut self,
        session_id: &SessionId,
        question_id: &QuestionId,
    ) -> Result<()> {
        let changed = self.conn.execute(
            "UPDATE session_items SET card_seen = 1
              WHERE session_id = ?1 AND question_id = ?2",
            params![session_id.as_str(), question_id.as_str()],
        )?;
        if changed == 0 {
            bail!("session item not found");
        }
        Ok(())
    }

    pub fn finish_session(&mut self, session_id: &SessionId, now: DateTime<Utc>) -> Result<()> {
        let changed = self.conn.execute(
            "UPDATE sessions
                SET status = 'completed',
                    ended_at = ?2,
                    current_index = (SELECT count(*) FROM session_items WHERE session_id = ?1)
              WHERE id = ?1",
            params![session_id.as_str(), fmt_time(now)],
        )?;
        if changed == 0 {
            bail!("session not found");
        }
        Ok(())
    }

    /// Record one logical answer. Reusing `attempt_id`, or submitting another
    /// answer for an already answered session item, never double-counts.
    pub fn submit_attempt(
        &mut self,
        draft: AttemptDraft,
        now: DateTime<Utc>,
    ) -> Result<SubmitOutcome> {
        #[cfg(test)]
        let fail_after_insert = self.fail_after_attempt_insert;
        let tx = self.conn.transaction()?;

        let duplicate: Option<String> = tx
            .query_row(
                "SELECT attempt_id FROM attempts WHERE attempt_id = ?1",
                params![draft.attempt_id.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        if duplicate.is_some() {
            return Ok(SubmitOutcome::Duplicate {
                attempt_id: draft.attempt_id.clone(),
            });
        }

        let item: Option<(i64, i64, Option<String>)> = tx
            .query_row(
                "SELECT answered, position, attempt_id FROM session_items
                  WHERE session_id = ?1 AND question_id = ?2",
                params![draft.session_id.as_str(), draft.question_id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()?;
        let Some((answered, position, existing_attempt)) = item else {
            bail!("session item not found");
        };
        if answered != 0 {
            return Ok(SubmitOutcome::AlreadyAnswered {
                attempt_id: existing_attempt.map(AttemptId::new),
            });
        }

        let session_status: String = tx.query_row(
            "SELECT status FROM sessions WHERE id = ?1",
            params![draft.session_id.as_str()],
            |row| row.get(0),
        )?;
        if session_status != "active" {
            bail!("session is not active");
        }

        let (kind, skill_id): (String, Option<String>) = tx.query_row(
            "SELECT kind, skill_id FROM questions WHERE id = ?1",
            params![draft.question_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let first_exposure: i64 = tx.query_row(
            "SELECT count(*) FROM attempts WHERE question_id = ?1",
            params![draft.question_id.as_str()],
            |row| row.get(0),
        )?;
        let first_exposure = first_exposure == 0;

        tx.execute(
            "INSERT INTO attempts(
                attempt_id, session_id, question_id, submitted_at, response_json,
                outcome, rating, score, hint_used, near_miss_retry, first_exposure, elapsed_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                draft.attempt_id.as_str(),
                draft.session_id.as_str(),
                draft.question_id.as_str(),
                fmt_time(now),
                serde_json::to_string(&draft.response)?,
                outcome_str(draft.outcome),
                draft.rating.as_i64(),
                draft.score as f64,
                i64::from(draft.hint_used),
                i64::from(draft.near_miss_retry),
                i64::from(first_exposure),
                draft.elapsed_ms as i64,
            ],
        )?;

        #[cfg(test)]
        if fail_after_insert {
            bail!("injected failure after attempt insert");
        }

        let mut due_at = None;
        let mut skill_mastery = None;

        if kind == "recall" {
            let prior = tx
                .query_row(
                    "SELECT stability, difficulty, due_at, reps, lapses, last_rating, last_reviewed_at
                       FROM review_states WHERE question_id = ?1",
                    params![draft.question_id.as_str()],
                    |row| {
                        Ok((
                            row.get::<_, f64>(0)? as f32,
                            row.get::<_, f64>(1)? as f32,
                            sql_time(&row.get::<_, String>(2)?)?,
                            row.get::<_, i64>(3)? as u32,
                            row.get::<_, i64>(4)? as u32,
                            sql_time(&row.get::<_, String>(6)?)?,
                        ))
                    },
                )
                .optional()?;
            let scheduler = Fsrs6Scheduler::new();
            let update = scheduler.review(
                prior.as_ref().map(|(stability, difficulty, _, _, _, _)| {
                    crate::study::ReviewMemory {
                        stability: *stability,
                        difficulty: *difficulty,
                    }
                }),
                prior
                    .as_ref()
                    .map(|(_, _, _, _, _, last_reviewed_at)| *last_reviewed_at),
                draft.rating,
                now,
            );
            let max_interval = update.interval_days;
            let due = update.due_at;
            tx.execute(
                "INSERT INTO review_states(question_id, stability, difficulty, due_at, reps,
                                           lapses, last_rating, last_reviewed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(question_id) DO UPDATE SET
                    stability = excluded.stability,
                    difficulty = excluded.difficulty,
                    due_at = excluded.due_at,
                    reps = excluded.reps,
                    lapses = excluded.lapses,
                    last_rating = excluded.last_rating,
                    last_reviewed_at = excluded.last_reviewed_at",
                params![
                    draft.question_id.as_str(),
                    update.memory.stability as f64,
                    update.memory.difficulty as f64,
                    fmt_time(due),
                    (prior.as_ref().map(|prior| prior.3).unwrap_or(0) + update.reps_delta) as i64,
                    (prior.as_ref().map(|prior| prior.4).unwrap_or(0) + update.lapses_delta) as i64,
                    draft.rating.as_i64(),
                    fmt_time(now),
                ],
            )?;
            let _ = max_interval;
            due_at = Some(due);
        } else {
            let skill = skill_id.context("skill question has no skill id")?;
            let skill_id = SkillId::new(skill);
            let prior = read_skill_state(&tx, &skill_id)?
                .unwrap_or_else(|| initial_skill_state(skill_id.clone(), now));
            let state = SimpleSkillEstimator.update(
                prior,
                SkillObservation {
                    outcome: draft.outcome,
                    score: draft.score,
                    assisted: draft.hint_used || draft.near_miss_retry,
                    hint_used: draft.hint_used,
                    first_exposure,
                },
                now,
            );
            tx.execute(
                "INSERT INTO skill_states(skill_id, attempts, distinct_questions,
                    first_exposure_attempts, first_exposure_successes, hint_uses,
                    recent_json, mastery, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(skill_id) DO UPDATE SET
                    attempts = excluded.attempts,
                    distinct_questions = excluded.distinct_questions,
                    first_exposure_attempts = excluded.first_exposure_attempts,
                    first_exposure_successes = excluded.first_exposure_successes,
                    hint_uses = excluded.hint_uses,
                    recent_json = excluded.recent_json,
                    mastery = excluded.mastery,
                    updated_at = excluded.updated_at",
                params![
                    state.skill_id.as_str(),
                    state.attempts as i64,
                    state.distinct_questions as i64,
                    state.first_exposure_attempts as i64,
                    state.first_exposure_successes as i64,
                    state.hint_uses as i64,
                    serde_json::to_string(&state.recent_successes)?,
                    state.mastery as f64,
                    fmt_time(state.updated_at),
                ],
            )?;
            skill_mastery = Some(state.mastery);
        }

        tx.execute(
            "UPDATE session_items SET answered = 1, attempt_id = ?3
              WHERE session_id = ?1 AND question_id = ?2",
            params![
                draft.session_id.as_str(),
                draft.question_id.as_str(),
                draft.attempt_id.as_str()
            ],
        )?;
        tx.execute(
            "UPDATE sessions SET current_index = MAX(current_index, ?2) WHERE id = ?1",
            params![draft.session_id.as_str(), position + 1],
        )?;

        tx.commit()?;
        Ok(SubmitOutcome::Recorded {
            attempt_id: draft.attempt_id,
            first_exposure,
            rating: draft.rating,
            due_at,
            skill_mastery,
        })
    }
}

fn migrate(conn: &mut Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
         ) STRICT;",
    )?;
    let applied: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )?;
    if applied > CURRENT_SCHEMA_VERSION {
        bail!(
            "database schema version {} is newer than supported version {}",
            applied,
            CURRENT_SCHEMA_VERSION
        );
    }
    if applied < 1 {
        let tx = conn.transaction()?;
        tx.execute_batch(schema::V1)?;
        tx.execute(
            "INSERT INTO schema_migrations(version, applied_at) VALUES (1, ?1)",
            params![fmt_time(Utc::now())],
        )?;
        tx.pragma_update(None, "user_version", 1)?;
        tx.commit()?;
    }
    Ok(())
}

fn read_skill_state(conn: &Connection, skill_id: &SkillId) -> Result<Option<SkillState>> {
    Ok(conn
        .query_row(
            "SELECT skill_id, attempts, distinct_questions, first_exposure_attempts,
                    first_exposure_successes, hint_uses, recent_json, mastery, updated_at
               FROM skill_states WHERE skill_id = ?1",
            params![skill_id.as_str()],
            |row| {
                Ok(SkillState {
                    skill_id: SkillId::new(row.get::<_, String>(0)?),
                    attempts: row.get::<_, i64>(1)? as u32,
                    distinct_questions: row.get::<_, i64>(2)? as u32,
                    first_exposure_attempts: row.get::<_, i64>(3)? as u32,
                    first_exposure_successes: row.get::<_, i64>(4)? as u32,
                    hint_uses: row.get::<_, i64>(5)? as u32,
                    recent_successes: sql_json(&row.get::<_, String>(6)?)?,
                    mastery: row.get::<_, f64>(7)? as f32,
                    updated_at: sql_time(&row.get::<_, String>(8)?)?,
                })
            },
        )
        .optional()?)
}

fn row_to_question(row: &Row<'_>) -> rusqlite::Result<Question> {
    let answer: AnswerSpec = sql_json(&row.get::<_, String>(6)?)?;
    let tags: Vec<String> = sql_json(&row.get::<_, String>(11)?)?;
    Ok(Question {
        id: QuestionId::new(row.get::<_, String>(0)?),
        workbook_id: WorkbookId::new(row.get::<_, String>(1)?),
        form: AnswerForm::parse(&row.get::<_, String>(2)?).unwrap_or(answer.form()),
        kind: LearningKind::parse(&row.get::<_, String>(3)?).unwrap_or(LearningKind::Recall),
        difficulty: Difficulty::parse(&row.get::<_, String>(4)?).unwrap_or(Difficulty::Beginner),
        prompt: sql_json(&row.get::<_, String>(5)?)?,
        answer,
        explanation: sql_json(&row.get::<_, String>(7)?)?,
        hint: row.get(8)?,
        card_id: row.get::<_, Option<String>>(9)?.map(CardId::new),
        skill_id: row.get::<_, Option<String>>(10)?.map(SkillId::new),
        tags: tags.into_iter().map(TagId::new).collect(),
        estimated_seconds: row.get::<_, i64>(12)? as u32,
        created_at: sql_time(&row.get::<_, String>(13)?)?,
    })
}

fn sql_time(value: &str) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|time| time.with_timezone(&Utc))
        .map_err(|error| rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(error)))
}

fn sql_json<T: DeserializeOwned>(value: &str) -> rusqlite::Result<T> {
    serde_json::from_str(value)
        .map_err(|error| rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(error)))
}

fn outcome_str(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Correct => "correct",
        Outcome::Partial => "partial",
        Outcome::Incorrect => "incorrect",
        Outcome::Ungradable => "ungradable",
    }
}

pub fn fmt_time(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub fn parse_time(value: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(value)
        .with_context(|| format!("invalid timestamp {value}"))?
        .with_timezone(&Utc))
}

fn truncate_chars(value: &str, limit: usize) -> String {
    let mut result = value.chars().take(limit).collect::<String>();
    if value.chars().count() > limit {
        result.push('…');
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AttemptId, Response};
    use chrono::Duration;

    fn seeded() -> Repository {
        let mut repo = Repository::open_in_memory().unwrap();
        repo.seed_sample_content().unwrap();
        repo
    }

    fn draft(session_id: &SessionId, question_id: &QuestionId, attempt_id: &str) -> AttemptDraft {
        AttemptDraft {
            attempt_id: AttemptId::new(attempt_id),
            session_id: session_id.clone(),
            question_id: question_id.clone(),
            response: Response::Text {
                text: "test".to_owned(),
            },
            outcome: Outcome::Correct,
            rating: Rating::Good,
            score: 1.0,
            hint_used: false,
            near_miss_retry: false,
            elapsed_ms: 1000,
        }
    }

    #[test]
    fn migration_and_seed_are_idempotent() {
        let mut repo = seeded();
        assert_eq!(repo.schema_version().unwrap(), 1);
        repo.seed_sample_content().unwrap();
        assert_eq!(repo.list_workbooks().unwrap().len(), 5);
        repo.seed_sample_content().unwrap();
        assert_eq!(repo.list_workbooks().unwrap().len(), 5);
        assert_eq!(repo.dashboard().unwrap().total_questions, 21);
    }

    #[test]
    fn duplicate_and_second_attempt_do_not_double_count() {
        let mut repo = seeded();
        let session = repo
            .start_session(Some(&WorkbookId::new("wb-01-math")), 10, Utc::now())
            .unwrap();
        let item = session.items[0].clone();
        let first = draft(&session.session.id, &item.question_id, "attempt-1");
        let outcome = repo.submit_attempt(first.clone(), Utc::now()).unwrap();
        assert!(matches!(outcome, SubmitOutcome::Recorded { .. }));

        let duplicate = repo.submit_attempt(first, Utc::now()).unwrap();
        assert!(matches!(duplicate, SubmitOutcome::Duplicate { .. }));

        let mut second = draft(&session.session.id, &item.question_id, "attempt-2");
        second.rating = Rating::Again;
        let already = repo.submit_attempt(second, Utc::now()).unwrap();
        assert!(matches!(already, SubmitOutcome::AlreadyAnswered { .. }));

        assert_eq!(repo.attempt_count_total().unwrap(), 1);
        assert_eq!(repo.attempt_count(&item.question_id).unwrap(), 1);
        let review = repo.review_state(&item.question_id).unwrap().unwrap();
        assert_eq!(review.reps, 1);
        let reloaded = repo.load_session(&session.session.id).unwrap().unwrap();
        assert_eq!(reloaded.session.current_index, 1);
    }

    #[test]
    fn injected_failure_rolls_back_attempt_and_state() {
        let mut repo = seeded();
        let session = repo
            .start_session(Some(&WorkbookId::new("wb-01-math")), 10, Utc::now())
            .unwrap();
        let item = session.items[0].clone();
        repo.fail_after_attempt_insert = true;
        let result = repo.submit_attempt(
            draft(&session.session.id, &item.question_id, "attempt-rollback"),
            Utc::now(),
        );
        assert!(result.is_err());
        assert_eq!(repo.attempt_count_total().unwrap(), 0);
        assert!(repo.review_state(&item.question_id).unwrap().is_none());
        let reloaded = repo.load_session(&session.session.id).unwrap().unwrap();
        assert!(!reloaded.items[0].answered);
        assert_eq!(reloaded.session.current_index, 0);
    }

    #[test]
    fn recall_and_skill_states_are_separated() {
        let mut repo = seeded();
        let session = repo
            .start_session(Some(&WorkbookId::new("wb-01-math")), 10, Utc::now())
            .unwrap();
        let recall_item = session
            .items
            .iter()
            .find(|item| item.question_id.as_str() == "q-math-slope-choice")
            .unwrap()
            .clone();
        let skill_item = session
            .items
            .iter()
            .find(|item| item.question_id.as_str() == "q-math-slope-skill")
            .unwrap()
            .clone();

        repo.submit_attempt(
            draft(&session.session.id, &recall_item.question_id, "recall-1"),
            Utc::now(),
        )
        .unwrap();
        assert!(
            repo.review_state(&recall_item.question_id)
                .unwrap()
                .is_some()
        );
        assert!(
            repo.skill_state(&SkillId::new("skill:math-linear-slope"))
                .unwrap()
                .is_none()
        );

        repo.submit_attempt(
            draft(&session.session.id, &skill_item.question_id, "skill-1"),
            Utc::now(),
        )
        .unwrap();
        let skill = repo
            .skill_state(&SkillId::new("skill:math-linear-slope"))
            .unwrap()
            .unwrap();
        assert_eq!(skill.attempts, 1);
        assert!(
            repo.review_state(&skill_item.question_id)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn session_resume_and_review_state_survive_reopen() {
        let path =
            std::env::temp_dir().join(format!("workbook3-test-{}.sqlite3", uuid::Uuid::new_v4()));
        let session_id;
        let question_id;
        {
            let mut repo = Repository::open(&path).unwrap();
            repo.seed_sample_content().unwrap();
            let session = repo
                .start_session(Some(&WorkbookId::new("wb-01-math")), 10, Utc::now())
                .unwrap();
            session_id = session.session.id.clone();
            question_id = session.items[0].question_id.clone();
            repo.mark_card_seen(&session_id, &question_id).unwrap();
            repo.submit_attempt(
                draft(&session_id, &question_id, "resume-1"),
                Utc::now() + Duration::seconds(1),
            )
            .unwrap();
        }
        {
            let repo = Repository::open(&path).unwrap();
            let active = repo.active_session().unwrap().unwrap();
            assert_eq!(active.session.id, session_id);
            assert_eq!(active.session.current_index, 1);
            assert!(active.items[0].answered);
            assert!(active.items[0].card_seen);
            assert!(repo.review_state(&question_id).unwrap().is_some());
        }
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("sqlite3-wal"));
        let _ = std::fs::remove_file(path.with_extension("sqlite3-shm"));
    }

    #[test]
    fn finish_session_marks_completed() {
        let mut repo = seeded();
        let session = repo
            .start_session(Some(&WorkbookId::new("wb-01-math")), 10, Utc::now())
            .unwrap();
        repo.finish_session(&session.session.id, Utc::now())
            .unwrap();
        let loaded = repo.load_session(&session.session.id).unwrap().unwrap();
        assert_eq!(loaded.session.status, SessionStatus::Completed);
        assert!(repo.active_session().unwrap().is_none());
    }
}
