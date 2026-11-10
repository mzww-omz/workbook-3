//! Core, UI-independent domain types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::content::{ContentBlock, NormalizedRect};

macro_rules! id_type {
    ($name:ident) => {
        #[derive(
            Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

id_type!(WorkbookId);
id_type!(CardId);
id_type!(QuestionId);
id_type!(SkillId);
id_type!(TagId);
id_type!(SessionId);
id_type!(AttemptId);
id_type!(GroupId);
id_type!(PlanId);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearningKind {
    /// Remember a stable item (term, formula, vocabulary, place name).
    Recall,
    /// Apply a skill to unfamiliar problems. Mastery is tracked per skill,
    /// not per question.
    Skill,
}

impl LearningKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Recall => "Recall",
            Self::Skill => "Skill",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Recall => "recall",
            Self::Skill => "skill",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "recall" => Some(Self::Recall),
            "skill" => Some(Self::Skill),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Difficulty {
    Beginner,
    Intermediate,
    Advanced,
}

impl Difficulty {
    pub fn label(self) -> &'static str {
        match self {
            Self::Beginner => "初級",
            Self::Intermediate => "中級",
            Self::Advanced => "上級",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Beginner => "beginner",
            Self::Intermediate => "intermediate",
            Self::Advanced => "advanced",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "beginner" => Some(Self::Beginner),
            "intermediate" => Some(Self::Intermediate),
            "advanced" => Some(Self::Advanced),
            _ => None,
        }
    }

    /// Conservative default time cost used by the deterministic planner.
    pub fn default_seconds(self) -> u32 {
        match self {
            Self::Beginner => 45,
            Self::Intermediate => 60,
            Self::Advanced => 90,
        }
    }
}

/// The eleven answer forms are fixed product vocabulary. `LearningKind` is
/// deliberately independent: a flashcard can be Recall, a numeric question can
/// be Skill.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnswerForm {
    SingleChoice,
    MultipleChoice,
    TrueFalse,
    ShortAnswer,
    FillBlank,
    Flashcard,
    Numeric,
    Ordering,
    Matching,
    Hotspot,
    Labeling,
}

impl AnswerForm {
    pub const ALL: [Self; 11] = [
        Self::SingleChoice,
        Self::MultipleChoice,
        Self::TrueFalse,
        Self::ShortAnswer,
        Self::FillBlank,
        Self::Flashcard,
        Self::Numeric,
        Self::Ordering,
        Self::Matching,
        Self::Hotspot,
        Self::Labeling,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::SingleChoice => "単一選択",
            Self::MultipleChoice => "複数選択",
            Self::TrueFalse => "○×",
            Self::ShortAnswer => "短答",
            Self::FillBlank => "穴埋め",
            Self::Flashcard => "フラッシュカード",
            Self::Numeric => "数値",
            Self::Ordering => "並べ替え",
            Self::Matching => "対応付け",
            Self::Hotspot => "領域選択",
            Self::Labeling => "ラベル配置",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::SingleChoice => "single_choice",
            Self::MultipleChoice => "multiple_choice",
            Self::TrueFalse => "true_false",
            Self::ShortAnswer => "short_answer",
            Self::FillBlank => "fill_blank",
            Self::Flashcard => "flashcard",
            Self::Numeric => "numeric",
            Self::Ordering => "ordering",
            Self::Matching => "matching",
            Self::Hotspot => "hotspot",
            Self::Labeling => "labeling",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|form| form.as_str() == value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TagKind {
    Subject,
    Grade,
    Unit,
    Skill,
    Other,
}

impl TagKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Subject => "subject",
            Self::Grade => "grade",
            Self::Unit => "unit",
            Self::Skill => "skill",
            Self::Other => "other",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "subject" => Some(Self::Subject),
            "grade" => Some(Self::Grade),
            "unit" => Some(Self::Unit),
            "skill" => Some(Self::Skill),
            "other" => Some(Self::Other),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tag {
    pub id: TagId,
    pub kind: TagKind,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workbook {
    pub id: WorkbookId,
    pub title: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LearningCard {
    pub id: CardId,
    pub workbook_id: WorkbookId,
    pub title: String,
    pub blocks: Vec<ContentBlock>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Question {
    pub id: QuestionId,
    pub workbook_id: WorkbookId,
    pub form: AnswerForm,
    pub kind: LearningKind,
    pub difficulty: Difficulty,
    pub prompt: Vec<ContentBlock>,
    pub answer: AnswerSpec,
    pub explanation: Vec<ContentBlock>,
    pub hint: Option<String>,
    pub card_id: Option<CardId>,
    pub skill_id: Option<SkillId>,
    pub tags: Vec<TagId>,
    pub estimated_seconds: u32,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Group {
    pub id: GroupId,
    pub name: String,
    pub description: String,
    pub workbook_ids: Vec<WorkbookId>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanTarget {
    Workbook(WorkbookId),
    Group(GroupId),
    All,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan {
    pub id: PlanId,
    pub name: String,
    pub target: PlanTarget,
    pub due_date: Option<DateTime<Utc>>,
    pub daily_minutes: u32,
    pub goal: String,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Answer specifications. Phase 1 renders four forms; all eleven are modeled.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Choice {
    pub id: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlankSpec {
    pub id: String,
    pub accepted: Vec<String>,
    pub hint: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tolerance {
    pub absolute: Option<f64>,
    pub relative: Option<f64>,
}

impl Tolerance {
    pub fn exact() -> Self {
        Self {
            absolute: Some(0.0),
            relative: None,
        }
    }

    pub fn absolute(value: f64) -> Self {
        Self {
            absolute: Some(value),
            relative: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UnitConversion {
    pub unit: String,
    /// Multiply a value in this unit by this factor to get the canonical unit.
    pub factor_to_canonical: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UnitSpec {
    pub canonical: String,
    pub accepted: Vec<UnitConversion>,
    #[serde(default)]
    pub required: bool,
}

impl UnitSpec {
    pub fn new(canonical: impl Into<String>) -> Self {
        let canonical = canonical.into();
        Self {
            accepted: vec![UnitConversion {
                unit: canonical.clone(),
                factor_to_canonical: 1.0,
            }],
            canonical,
            required: true,
        }
    }

    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }

    pub fn with(mut self, unit: impl Into<String>, factor_to_canonical: f64) -> Self {
        self.accepted.push(UnitConversion {
            unit: unit.into(),
            factor_to_canonical,
        });
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NumericSpec {
    pub value: f64,
    pub tolerance: Tolerance,
    pub unit: Option<UnitSpec>,
    pub significant_figures: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OrderingSpec {
    pub items: Vec<Choice>,
    /// Item ids in the correct order.
    pub correct_order: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MatchingSpec {
    pub left: Vec<Choice>,
    pub right: Vec<Choice>,
    /// (left id, right id)
    pub pairs: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HotspotSpec {
    pub image_asset: String,
    pub regions: Vec<HotspotRegion>,
    pub correct_region_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HotspotRegion {
    pub id: String,
    pub label: String,
    pub rect: NormalizedRect,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LabelingSpec {
    pub image_asset: String,
    pub labels: Vec<Choice>,
    pub targets: Vec<LabelTarget>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LabelTarget {
    pub id: String,
    pub correct_label_id: String,
    pub rect: NormalizedRect,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnswerSpec {
    SingleChoice {
        choices: Vec<Choice>,
        correct_ids: Vec<String>,
    },
    MultipleChoice {
        choices: Vec<Choice>,
        correct_ids: Vec<String>,
    },
    TrueFalse {
        correct: bool,
    },
    ShortAnswer {
        accepted: Vec<String>,
        #[serde(default)]
        near_miss_aliases: Vec<String>,
    },
    FillBlank {
        template: String,
        blanks: Vec<BlankSpec>,
    },
    Flashcard {
        front: String,
        back: String,
    },
    Numeric(NumericSpec),
    Ordering(OrderingSpec),
    Matching(MatchingSpec),
    Hotspot(HotspotSpec),
    Labeling(LabelingSpec),
}

impl AnswerSpec {
    pub fn form(&self) -> AnswerForm {
        match self {
            Self::SingleChoice { .. } => AnswerForm::SingleChoice,
            Self::MultipleChoice { .. } => AnswerForm::MultipleChoice,
            Self::TrueFalse { .. } => AnswerForm::TrueFalse,
            Self::ShortAnswer { .. } => AnswerForm::ShortAnswer,
            Self::FillBlank { .. } => AnswerForm::FillBlank,
            Self::Flashcard { .. } => AnswerForm::Flashcard,
            Self::Numeric(_) => AnswerForm::Numeric,
            Self::Ordering(_) => AnswerForm::Ordering,
            Self::Matching(_) => AnswerForm::Matching,
            Self::Hotspot(_) => AnswerForm::Hotspot,
            Self::Labeling(_) => AnswerForm::Labeling,
        }
    }

    pub fn choices(&self) -> &[Choice] {
        match self {
            Self::SingleChoice { choices, .. } | Self::MultipleChoice { choices, .. } => choices,
            _ => &[],
        }
    }

    pub fn display_answer(&self) -> String {
        match self {
            Self::SingleChoice {
                choices,
                correct_ids,
            } => choices
                .iter()
                .filter(|choice| correct_ids.contains(&choice.id))
                .map(|choice| choice.text.clone())
                .collect::<Vec<_>>()
                .join(", "),
            Self::MultipleChoice {
                choices,
                correct_ids,
            } => choices
                .iter()
                .filter(|choice| correct_ids.contains(&choice.id))
                .map(|choice| choice.text.clone())
                .collect::<Vec<_>>()
                .join(", "),
            Self::TrueFalse { correct } => {
                if *correct {
                    "○".to_owned()
                } else {
                    "×".to_owned()
                }
            }
            Self::ShortAnswer { accepted, .. } => accepted.first().cloned().unwrap_or_default(),
            Self::FillBlank { blanks, .. } => blanks
                .iter()
                .filter_map(|blank| blank.accepted.first().cloned())
                .collect::<Vec<_>>()
                .join(" / "),
            Self::Flashcard { back, .. } => back.clone(),
            Self::Numeric(spec) => match &spec.unit {
                Some(unit) => format!("{} {}", spec.value, unit.canonical),
                None => spec.value.to_string(),
            },
            Self::Ordering(spec) => spec
                .correct_order
                .iter()
                .filter_map(|id| spec.items.iter().find(|item| &item.id == id))
                .map(|item| item.text.clone())
                .collect::<Vec<_>>()
                .join(" → "),
            Self::Matching(spec) => spec
                .pairs
                .iter()
                .filter_map(|(left, right)| {
                    let left = spec.left.iter().find(|item| &item.id == left)?;
                    let right = spec.right.iter().find(|item| &item.id == right)?;
                    Some(format!("{} → {}", left.text, right.text))
                })
                .collect::<Vec<_>>()
                .join("、"),
            Self::Hotspot(spec) => spec
                .regions
                .iter()
                .filter(|region| spec.correct_region_ids.contains(&region.id))
                .map(|region| region.label.clone())
                .collect::<Vec<_>>()
                .join(", "),
            Self::Labeling(spec) => spec
                .targets
                .iter()
                .filter_map(|target| {
                    let label = spec
                        .labels
                        .iter()
                        .find(|label| label.id == target.correct_label_id)?;
                    Some(label.text.clone())
                })
                .collect::<Vec<_>>()
                .join(", "),
        }
    }
}

// ---------------------------------------------------------------------------
// Attempts and learning state
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rating {
    Again = 1,
    Hard = 2,
    Good = 3,
    Easy = 4,
}

impl Rating {
    pub fn as_i64(self) -> i64 {
        self as i64
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Again => "もう一度",
            Self::Hard => "難しい",
            Self::Good => "できた",
            Self::Easy => "簡単",
        }
    }

    pub fn parse(value: i64) -> Option<Self> {
        match value {
            1 => Some(Self::Again),
            2 => Some(Self::Hard),
            3 => Some(Self::Good),
            4 => Some(Self::Easy),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Correct,
    Partial,
    Incorrect,
    /// The form exists but automatic grading is intentionally deferred.
    Ungradable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelfAssessment {
    Again,
    Good,
    Easy,
}

impl SelfAssessment {
    pub fn rating(self) -> Rating {
        match self {
            Self::Again => Rating::Again,
            Self::Good => Rating::Good,
            Self::Easy => Rating::Easy,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Choice {
        selected_ids: Vec<String>,
    },
    Text {
        text: String,
    },
    Flashcard {
        assessment: SelfAssessment,
    },
    /// Reserved for Phase 2 renderers.
    Structured {
        payload: serde_json::Value,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Attempt {
    pub attempt_id: AttemptId,
    pub session_id: SessionId,
    pub question_id: QuestionId,
    pub submitted_at: DateTime<Utc>,
    pub response: Response,
    pub outcome: Outcome,
    pub rating: Rating,
    pub score: f32,
    pub hint_used: bool,
    pub near_miss_retry: bool,
    pub first_exposure: bool,
    pub elapsed_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReviewState {
    pub question_id: QuestionId,
    pub stability: f32,
    pub difficulty: f32,
    pub due_at: DateTime<Utc>,
    pub reps: u32,
    pub lapses: u32,
    pub last_rating: Rating,
    pub last_reviewed_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SkillState {
    pub skill_id: SkillId,
    pub attempts: u32,
    pub distinct_questions: u32,
    pub first_exposure_attempts: u32,
    pub first_exposure_successes: u32,
    pub hint_uses: u32,
    /// Recency-weighted success values in [0, 1], newest last. At most 8.
    pub recent_successes: Vec<f32>,
    pub mastery: f32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Active,
    Completed,
    Abandoned,
}

impl SessionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Completed => "completed",
            Self::Abandoned => "abandoned",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "active" => Some(Self::Active),
            "completed" => Some(Self::Completed),
            "abandoned" => Some(Self::Abandoned),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    /// `None` means the session spans all workbooks.
    pub workbook_id: Option<WorkbookId>,
    pub budget_minutes: u32,
    pub status: SessionStatus,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub current_index: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionItem {
    pub session_id: SessionId,
    pub question_id: QuestionId,
    pub position: usize,
    pub show_card: bool,
    pub card_seen: bool,
    pub answered: bool,
    pub attempt_id: Option<AttemptId>,
    /// Human-readable planner reason, kept for explainability.
    pub selection_reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionView {
    pub session: Session,
    pub workbook_title: String,
    pub items: Vec<SessionItem>,
}

impl SessionView {
    pub fn current_item(&self) -> Option<&SessionItem> {
        self.items.get(self.session.current_index)
    }

    pub fn answered_count(&self) -> usize {
        self.items.iter().filter(|item| item.answered).count()
    }

    pub fn progress_label(&self) -> String {
        if self.items.is_empty() {
            return "0 / 0".to_owned();
        }
        let current = self.session.current_index.min(self.items.len()) + 1;
        format!("{} / {}", current.min(self.items.len()), self.items.len())
    }
}
