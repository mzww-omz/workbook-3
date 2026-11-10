//! One deterministic planner for Recall reviews, Skill practice, new material,
//! and a time budget. Phase 1 has no Exam-specific priority; Phase 3 will add
//! exam urgency here, without altering FSRS due dates.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::{Difficulty, LearningKind, QuestionId, WorkbookId};

pub const CARD_INTRO_SECONDS: u32 = 30;

#[derive(Clone, Debug)]
pub struct PlanCandidate {
    pub question_id: QuestionId,
    pub workbook_id: WorkbookId,
    pub kind: LearningKind,
    pub difficulty: Difficulty,
    pub estimated_seconds: u32,
    pub due_at: Option<DateTime<Utc>>,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub attempts: u32,
    pub skill_mastery: Option<f32>,
    pub has_card: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum SelectionReason {
    DueRecall { overdue_hours: i64 },
    WeakSkill { mastery_percent: u32 },
    NewMaterial,
    Practice,
}

impl SelectionReason {
    pub fn label(&self) -> String {
        match self {
            Self::DueRecall { overdue_hours } => {
                format!("復習期限（{}時間超過）", (*overdue_hours).max(0))
            }
            Self::WeakSkill { mastery_percent } => {
                format!("弱点Skill（習得度 {}%）", mastery_percent)
            }
            Self::NewMaterial => "未学習".to_owned(),
            Self::Practice => "追加練習".to_owned(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PlanSelection {
    pub question_id: QuestionId,
    pub reason: SelectionReason,
    pub estimated_seconds: u32,
    pub card_intro: bool,
}

#[derive(Clone, Debug)]
pub struct Plan {
    pub selections: Vec<PlanSelection>,
    pub budget_minutes: u32,
    pub estimated_seconds: u32,
    pub over_budget: bool,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.selections.is_empty()
    }
}

/// Select high-value work that fits the time budget. The function is pure and
/// deterministic for a fixed `now` and candidate set.
pub fn plan_session(candidates: &[PlanCandidate], budget_minutes: u32, now: DateTime<Utc>) -> Plan {
    let budget_seconds = budget_minutes.saturating_mul(60);
    let mut ranked = candidates
        .iter()
        .map(|candidate| (sort_key(candidate, now), candidate))
        .collect::<Vec<_>>();
    ranked.sort_by(|(left_key, left), (right_key, right)| {
        left_key
            .cmp(right_key)
            .then_with(|| left.question_id.cmp(&right.question_id))
    });

    let mut selections = Vec::new();
    let mut used = 0u32;
    for (_, candidate) in ranked {
        let card_intro = candidate.attempts == 0 && candidate.has_card;
        let cost = candidate.estimated_seconds.saturating_add(if card_intro {
            CARD_INTRO_SECONDS
        } else {
            0
        });
        if selections.is_empty() {
            selections.push(selection(candidate, now, cost, card_intro));
            used = cost;
            continue;
        }
        if used.saturating_add(cost) <= budget_seconds {
            selections.push(selection(candidate, now, cost, card_intro));
            used += cost;
        }
    }

    Plan {
        over_budget: used > budget_seconds,
        selections,
        budget_minutes,
        estimated_seconds: used,
    }
}

fn selection(
    candidate: &PlanCandidate,
    now: DateTime<Utc>,
    estimated_seconds: u32,
    card_intro: bool,
) -> PlanSelection {
    PlanSelection {
        question_id: candidate.question_id.clone(),
        reason: reason(candidate, now),
        estimated_seconds,
        card_intro,
    }
}

fn reason(candidate: &PlanCandidate, now: DateTime<Utc>) -> SelectionReason {
    if candidate.kind == LearningKind::Recall
        && let Some(due_at) = candidate.due_at
        && due_at <= now
    {
        return SelectionReason::DueRecall {
            overdue_hours: (now - due_at).num_hours(),
        };
    }
    if candidate.kind == LearningKind::Skill
        && candidate.attempts > 0
        && candidate.skill_mastery.unwrap_or(0.0) < 0.6
    {
        return SelectionReason::WeakSkill {
            mastery_percent: (candidate.skill_mastery.unwrap_or(0.0) * 100.0).round() as u32,
        };
    }
    if candidate.attempts == 0 {
        return SelectionReason::NewMaterial;
    }
    SelectionReason::Practice
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct SortKey {
    bucket: u8,
    due_at: Option<DateTime<Utc>>,
    mastery_milli: u32,
    last_attempt: Option<DateTime<Utc>>,
}

fn sort_key(candidate: &PlanCandidate, now: DateTime<Utc>) -> SortKey {
    let bucket = if candidate.kind == LearningKind::Recall
        && candidate.due_at.is_some_and(|due| due <= now)
    {
        0
    } else if candidate.kind == LearningKind::Skill
        && candidate.attempts > 0
        && candidate.skill_mastery.unwrap_or(0.0) < 0.6
    {
        1
    } else if candidate.attempts == 0 {
        2
    } else {
        3
    };
    SortKey {
        bucket,
        due_at: candidate.due_at,
        mastery_milli: (candidate.skill_mastery.unwrap_or(0.0) * 1000.0).round() as u32,
        last_attempt: candidate.last_attempt_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn candidate(id: &str, kind: LearningKind, seconds: u32) -> PlanCandidate {
        PlanCandidate {
            question_id: QuestionId::new(id),
            workbook_id: WorkbookId::new("w"),
            kind,
            difficulty: Difficulty::Beginner,
            estimated_seconds: seconds,
            due_at: None,
            last_attempt_at: None,
            attempts: 0,
            skill_mastery: None,
            has_card: false,
        }
    }

    #[test]
    fn budget_limits_selection() {
        let now = Utc::now();
        let candidates = (0..10)
            .map(|index| candidate(&format!("q{index}"), LearningKind::Recall, 60))
            .collect::<Vec<_>>();
        let plan = plan_session(&candidates, 3, now);
        assert_eq!(plan.selections.len(), 3);
        assert_eq!(plan.estimated_seconds, 180);
        assert!(!plan.over_budget);
    }

    #[test]
    fn due_recall_then_weak_skill_then_new() {
        let now = Utc::now();
        let mut due = candidate("q-due", LearningKind::Recall, 60);
        due.due_at = Some(now - Duration::hours(5));
        due.attempts = 2;
        let mut weak = candidate("q-weak", LearningKind::Skill, 60);
        weak.attempts = 3;
        weak.skill_mastery = Some(0.2);
        let new = candidate("q-new", LearningKind::Recall, 60);
        let mut stronger = candidate("q-strong", LearningKind::Skill, 60);
        stronger.attempts = 2;
        stronger.skill_mastery = Some(0.8);

        let plan = plan_session(&[new, stronger, weak, due], 20, now);
        assert_eq!(plan.selections[0].question_id.as_str(), "q-due");
        assert_eq!(plan.selections[1].question_id.as_str(), "q-weak");
        assert_eq!(plan.selections[2].question_id.as_str(), "q-new");
        assert!(matches!(
            plan.selections[0].reason,
            SelectionReason::DueRecall { .. }
        ));
        assert!(matches!(
            plan.selections[1].reason,
            SelectionReason::WeakSkill { .. }
        ));
    }

    #[test]
    fn card_intro_consumes_budget() {
        let now = Utc::now();
        let mut first = candidate("q1", LearningKind::Recall, 90);
        first.has_card = true;
        let mut second = candidate("q2", LearningKind::Recall, 90);
        second.has_card = true;
        let plan = plan_session(&[first, second], 3, now);
        assert_eq!(plan.selections.len(), 1);
        assert_eq!(plan.estimated_seconds, 120);
        assert!(plan.selections[0].card_intro);
    }

    #[test]
    fn one_question_is_selected_even_if_budget_is_tiny() {
        let now = Utc::now();
        let candidates = vec![candidate("q1", LearningKind::Recall, 90)];
        let plan = plan_session(&candidates, 1, now);
        assert_eq!(plan.selections.len(), 1);
        assert!(plan.over_budget);
    }

    #[test]
    fn deterministic_for_same_inputs() {
        let now = Utc::now();
        let candidates = vec![
            candidate("q-b", LearningKind::Recall, 60),
            candidate("q-a", LearningKind::Recall, 60),
        ];
        let first = plan_session(&candidates, 10, now);
        let second = plan_session(&candidates, 10, now);
        assert_eq!(
            first
                .selections
                .iter()
                .map(|selection| selection.question_id.clone())
                .collect::<Vec<_>>(),
            second
                .selections
                .iter()
                .map(|selection| selection.question_id.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(first.selections[0].question_id.as_str(), "q-a");
    }
}
