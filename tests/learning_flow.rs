//! End-to-end core flow tests. These exercise the same API the GPUI layer uses,
//! without requiring a window server.

use anyhow::Result;
use chrono::Utc;

use workbook3::domain::{
    AnswerSpec, AttemptId, Question, Rating, Response, SelfAssessment, SessionStatus, SessionView,
    WorkbookId,
};
use workbook3::storage::{AttemptDraft, Repository, SubmitOutcome};
use workbook3::study::{Grading, grade};

fn correct_response(question: &Question) -> Response {
    match &question.answer {
        AnswerSpec::SingleChoice { correct_ids, .. } => Response::Choice {
            selected_ids: correct_ids.clone(),
        },
        AnswerSpec::MultipleChoice { correct_ids, .. } => Response::Choice {
            selected_ids: correct_ids.clone(),
        },
        AnswerSpec::TrueFalse { correct } => Response::Choice {
            selected_ids: vec![correct.to_string()],
        },
        AnswerSpec::ShortAnswer { accepted, .. } => Response::Text {
            text: accepted
                .first()
                .cloned()
                .expect("sample short answer has an accepted value"),
        },
        AnswerSpec::Numeric(spec) => Response::Text {
            text: match &spec.unit {
                Some(unit) => format!("{} {}", spec.value, unit.canonical),
                None => spec.value.to_string(),
            },
        },
        AnswerSpec::Flashcard { .. } => Response::Flashcard {
            assessment: SelfAssessment::Good,
        },
        _ => Response::Text {
            text: "unsupported in Phase 1".to_owned(),
        },
    }
}

fn answer_item(
    repo: &mut Repository,
    session: &SessionView,
    item_index: usize,
    attempt_suffix: &str,
) -> Result<()> {
    let item = &session.items[item_index];
    let question = repo
        .get_question(&item.question_id)?
        .expect("question exists");
    let response = correct_response(&question);
    let grading = grade(&question, &response, false, false);
    let Grading::Final(result) = grading else {
        panic!("correct response must not be a near miss");
    };
    assert_eq!(result.outcome, workbook3::domain::Outcome::Correct);
    let outcome = repo.submit_attempt(
        AttemptDraft {
            attempt_id: AttemptId::new(format!("attempt-{attempt_suffix}")),
            session_id: session.session.id.clone(),
            question_id: item.question_id.clone(),
            response,
            outcome: result.outcome,
            rating: result.rating,
            score: result.score,
            hint_used: false,
            near_miss_retry: false,
            elapsed_ms: 1200,
        },
        Utc::now(),
    )?;
    assert!(matches!(outcome, SubmitOutcome::Recorded { .. }));
    Ok(())
}

#[test]
fn full_recall_skill_session_flow() -> Result<()> {
    let mut repo = Repository::open_in_memory()?;
    repo.seed_sample_content()?;
    let session = repo.start_session(Some(&WorkbookId::new("wb-01-math")), 10, Utc::now())?;
    assert_eq!(session.items.len(), 5);
    assert!(session.items[0].show_card);
    assert_eq!(session.session.current_index, 0);

    for index in 0..session.items.len() {
        answer_item(&mut repo, &session, index, &format!("flow-{index}"))?;
    }

    let loaded = repo.load_session(&session.session.id)?.expect("session");
    assert_eq!(loaded.session.current_index, 5);
    assert_eq!(loaded.answered_count(), 5);
    assert_eq!(repo.attempt_count_total()?, 5);

    // 3 Recall questions get FSRS state; 2 Skill questions are tracked per
    // Skill, not per question.
    let recall_questions = [
        "q-math-flashcard",
        "q-math-slope-choice",
        "q-math-value-numeric",
    ];
    for id in recall_questions {
        let state = repo
            .review_state(&workbook3::domain::QuestionId::new(id))?
            .expect("recall review state");
        assert_eq!(state.reps, 1);
        assert_eq!(state.last_rating, Rating::Good);
        assert!(state.due_at > Utc::now());
    }
    let skill_state = repo
        .skill_state(&workbook3::domain::SkillId::new("skill:math-linear-slope"))?
        .expect("skill state");
    assert_eq!(skill_state.attempts, 2);
    assert_eq!(skill_state.first_exposure_attempts, 2);
    assert_eq!(skill_state.first_exposure_successes, 2);
    assert!(skill_state.mastery > 0.9);
    assert!(
        repo.review_state(&workbook3::domain::QuestionId::new(
            "q-math-slope-from-points"
        ))?
        .is_none()
    );

    repo.finish_session(&session.session.id, Utc::now())?;
    let completed = repo.load_session(&session.session.id)?.expect("session");
    assert_eq!(completed.session.status, SessionStatus::Completed);
    assert!(repo.active_session()?.is_none());
    Ok(())
}

#[test]
fn duplicate_submission_does_not_double_count_skill_or_recall() -> Result<()> {
    let mut repo = Repository::open_in_memory()?;
    repo.seed_sample_content()?;
    let session = repo.start_session(Some(&WorkbookId::new("wb-01-math")), 10, Utc::now())?;
    let item = &session.items[0];
    let question = repo.get_question(&item.question_id)?.expect("question");
    let response = correct_response(&question);
    let Grading::Final(result) = grade(&question, &response, false, false) else {
        panic!("expected final grade");
    };
    let draft = AttemptDraft {
        attempt_id: AttemptId::new("same-attempt"),
        session_id: session.session.id.clone(),
        question_id: item.question_id.clone(),
        response: response.clone(),
        outcome: result.outcome,
        rating: result.rating,
        score: result.score,
        hint_used: false,
        near_miss_retry: false,
        elapsed_ms: 500,
    };
    assert!(matches!(
        repo.submit_attempt(draft.clone(), Utc::now())?,
        SubmitOutcome::Recorded { .. }
    ));
    assert!(matches!(
        repo.submit_attempt(draft, Utc::now())?,
        SubmitOutcome::Duplicate { .. }
    ));
    assert_eq!(repo.attempt_count(&item.question_id)?, 1);
    let state = repo.review_state(&item.question_id)?.expect("state");
    assert_eq!(state.reps, 1);
    Ok(())
}
