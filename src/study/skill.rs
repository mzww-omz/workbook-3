//! Provisional, explainable Skill estimator.
//!
//! A Skill is learned when the learner succeeds on *unfamiliar* problems, so
//! only first exposures count toward the first-exposure success ratio. Repeating
//! the same question cannot fully raise mastery. The `SkillEstimator` boundary
//! leaves room for a validated BKT/PFA replacement later.

use chrono::{DateTime, Utc};

use crate::domain::{Outcome, SkillId, SkillState};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkillObservation {
    pub outcome: Outcome,
    pub score: f32,
    pub assisted: bool,
    pub hint_used: bool,
    pub first_exposure: bool,
}

pub trait SkillEstimator {
    fn update(
        &self,
        prior: SkillState,
        observation: SkillObservation,
        at: DateTime<Utc>,
    ) -> SkillState;
}

#[derive(Default)]
pub struct SimpleSkillEstimator;

impl SkillEstimator for SimpleSkillEstimator {
    fn update(
        &self,
        mut state: SkillState,
        observation: SkillObservation,
        at: DateTime<Utc>,
    ) -> SkillState {
        state.attempts += 1;
        state.hint_uses += u32::from(observation.hint_used);
        if observation.first_exposure {
            state.distinct_questions += 1;
            state.first_exposure_attempts += 1;
            if observation.outcome == Outcome::Correct && !observation.assisted {
                state.first_exposure_successes += 1;
            }
        }

        let weight = match observation.outcome {
            Outcome::Correct if !observation.assisted => 1.0,
            Outcome::Correct => 0.5,
            Outcome::Partial => 0.25 * observation.score.clamp(0.0, 1.0),
            Outcome::Incorrect | Outcome::Ungradable => 0.0,
        };
        state.recent_successes.push(weight);
        if state.recent_successes.len() > 8 {
            state.recent_successes.remove(0);
        }
        state.mastery = mastery(&state);
        state.updated_at = at;
        state
    }
}

/// 60% first-exposure success, 40% recency-weighted performance. Both terms are
/// visible to learners in the UI as "初見正解" and "直近の正答".
pub fn mastery(state: &SkillState) -> f32 {
    let first_exposure = if state.first_exposure_attempts == 0 {
        0.0
    } else {
        state.first_exposure_successes as f32 / state.first_exposure_attempts as f32
    };
    let recent = if state.recent_successes.is_empty() {
        0.0
    } else {
        state.recent_successes.iter().sum::<f32>() / state.recent_successes.len() as f32
    };
    (first_exposure * 0.6 + recent * 0.4).clamp(0.0, 1.0)
}

pub fn initial_skill_state(skill_id: SkillId, at: DateTime<Utc>) -> SkillState {
    SkillState {
        skill_id,
        updated_at: at,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Outcome, SkillId};
    use chrono::{Duration, Utc};

    fn update(state: SkillState, outcome: Outcome, assisted: bool, first: bool) -> SkillState {
        SimpleSkillEstimator.update(
            state,
            SkillObservation {
                outcome,
                score: if outcome == Outcome::Correct {
                    1.0
                } else {
                    0.0
                },
                assisted,
                hint_used: assisted,
                first_exposure: first,
            },
            Utc::now(),
        )
    }

    #[test]
    fn repeated_question_cannot_fake_first_exposure_success() {
        let now = Utc::now();
        let id = SkillId::new("skill:test");
        let state = initial_skill_state(id.clone(), now);
        let state = update(state, Outcome::Incorrect, false, true);
        assert_eq!(state.first_exposure_attempts, 1);
        assert_eq!(state.first_exposure_successes, 0);
        assert_eq!(state.distinct_questions, 1);

        let state = update(state, Outcome::Correct, false, false);
        assert_eq!(state.first_exposure_attempts, 1);
        assert_eq!(state.first_exposure_successes, 0);
        assert_eq!(state.distinct_questions, 1);
        assert!(state.mastery < 0.5);

        let second_question = update(state, Outcome::Correct, false, true);
        assert_eq!(second_question.first_exposure_attempts, 2);
        assert_eq!(second_question.first_exposure_successes, 1);
        assert!(second_question.mastery > 0.5);
    }

    #[test]
    fn assisted_success_is_weaker_than_unaided_success() {
        let now = Utc::now() - Duration::seconds(1);
        let id = SkillId::new("skill:test");
        let unaided = update(
            initial_skill_state(id.clone(), now),
            Outcome::Correct,
            false,
            true,
        );
        let assisted = update(initial_skill_state(id, now), Outcome::Correct, true, true);
        assert!(unaided.mastery > assisted.mastery);
    }
}
