//! Recall scheduler adapter. The product targets FSRS-7, but `fsrs-rs` 6.6.2
//! ships the FSRS-6 model (21 parameters, FSRS6 decay). We adopt FSRS-6
//! explicitly and keep the adapter boundary so a future FSRS-7 implementation
//! can replace it without touching the UI, storage, or attempt model.

use chrono::{DateTime, Duration, Utc};
use fsrs::{FSRS, MemoryState};

use crate::domain::Rating;

pub const FSRS_MODEL_VERSION: &str = "FSRS-6 (fsrs-rs 6.6.2 defaults)";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReviewMemory {
    pub stability: f32,
    pub difficulty: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReviewUpdate {
    pub memory: ReviewMemory,
    pub due_at: DateTime<Utc>,
    pub interval_days: f32,
    pub reps_delta: u32,
    pub lapses_delta: u32,
}

/// Pure scheduling boundary. Implementations must not read the clock; callers
/// pass the review timestamp.
pub trait RecallScheduler {
    fn model_version(&self) -> &'static str;
    fn review(
        &self,
        previous: Option<ReviewMemory>,
        last_reviewed_at: Option<DateTime<Utc>>,
        rating: Rating,
        reviewed_at: DateTime<Utc>,
    ) -> ReviewUpdate;
}

pub struct Fsrs6Scheduler {
    fsrs: FSRS,
    desired_retention: f32,
}

impl Fsrs6Scheduler {
    pub fn new() -> Self {
        Self {
            fsrs: FSRS::new(&[]).expect("built-in FSRS parameters are valid"),
            desired_retention: 0.9,
        }
    }

    pub fn with_desired_retention(desired_retention: f32) -> Self {
        Self {
            fsrs: FSRS::new(&[]).expect("built-in FSRS parameters are valid"),
            desired_retention: desired_retention.clamp(0.7, 0.99),
        }
    }
}

impl Default for Fsrs6Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl RecallScheduler for Fsrs6Scheduler {
    fn model_version(&self) -> &'static str {
        FSRS_MODEL_VERSION
    }

    fn review(
        &self,
        previous: Option<ReviewMemory>,
        last_reviewed_at: Option<DateTime<Utc>>,
        rating: Rating,
        reviewed_at: DateTime<Utc>,
    ) -> ReviewUpdate {
        let elapsed_days = last_reviewed_at
            .map(|last| (reviewed_at - last).num_seconds().max(0) / 86_400)
            .unwrap_or(0)
            .min(u32::MAX as i64) as u32;

        let memory = previous.map(|memory| MemoryState {
            stability: memory.stability,
            difficulty: memory.difficulty,
        });

        let states = self
            .fsrs
            .next_states(memory, self.desired_retention, elapsed_days)
            .expect("valid FSRS memory state");
        let item = match rating {
            Rating::Again => &states.again,
            Rating::Hard => &states.hard,
            Rating::Good => &states.good,
            Rating::Easy => &states.easy,
        };

        // Again is a same-day relearning step. Larger ratings use the FSRS
        // interval directly. We never mutate due dates for Exam plans.
        let interval_days = item.interval.max(0.0);
        let seconds = (interval_days as f64 * 86_400.0).round() as i64;
        let seconds = if rating == Rating::Again {
            seconds.clamp(600, 86_400)
        } else {
            seconds.max(3_600)
        };

        ReviewUpdate {
            memory: ReviewMemory {
                stability: item.memory.stability,
                difficulty: item.memory.difficulty,
            },
            due_at: reviewed_at + Duration::seconds(seconds),
            interval_days: seconds as f32 / 86_400.0,
            reps_delta: 1,
            lapses_delta: u32::from(rating == Rating::Again),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_review_schedules_future_and_lapse_is_shorter() {
        let scheduler = Fsrs6Scheduler::new();
        let now = DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let good = scheduler.review(None, None, Rating::Good, now);
        let again = scheduler.review(None, None, Rating::Again, now);
        assert!(good.due_at > now);
        assert!(again.due_at > now);
        assert!(good.due_at > again.due_at);
        assert!(good.memory.stability > 0.0);
        assert!(good.memory.difficulty > 0.0);
        assert_eq!(good.reps_delta, 1);
        assert_eq!(again.lapses_delta, 1);
    }

    #[test]
    fn later_review_uses_elapsed_time() {
        let scheduler = Fsrs6Scheduler::new();
        let first = DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let second = first + Duration::days(3);
        let initial = scheduler.review(None, None, Rating::Good, first);
        let reviewed = scheduler.review(Some(initial.memory), Some(first), Rating::Good, second);
        assert!(reviewed.memory.stability >= initial.memory.stability);
        assert!(reviewed.due_at > second);
    }
}
