//! Deterministic grading. Grading is intentionally separate from FSRS:
//! correctness decides *what happened*, the scheduler decides *when to review*.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use crate::domain::{
    AnswerForm, AnswerSpec, NumericSpec, Outcome, Question, Rating, Response, SelfAssessment,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GradeResult {
    pub outcome: Outcome,
    pub score: f32,
    pub rating: Rating,
    /// True when a hint or a near-miss retry helped produce the answer.
    pub assisted: bool,
    pub feedback: String,
    pub expected: Option<String>,
}

impl GradeResult {
    pub fn is_correct(&self) -> bool {
        self.outcome == Outcome::Correct
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Grading {
    /// Do not persist this answer yet. Show the message and let the learner
    /// re-enter once. The same Attempt ID is reused for the final submission.
    NearMiss {
        message: String,
    },
    Final(GradeResult),
}

pub fn grade(
    question: &Question,
    response: &Response,
    hint_used: bool,
    near_miss_retry: bool,
) -> Grading {
    if question.form == AnswerForm::Flashcard {
        return match response {
            Response::Flashcard { assessment } => Grading::Final(flashcard_grade(*assessment)),
            _ => Grading::Final(ungradable(
                "フラッシュカードの自己評価が必要です。",
                question,
            )),
        };
    }

    let assisted = hint_used || near_miss_retry;
    match (&question.answer, response) {
        (
            AnswerSpec::SingleChoice {
                correct_ids,
                choices: _,
            },
            Response::Choice { selected_ids },
        ) => {
            let correct = set_equal(correct_ids, selected_ids);
            Grading::Final(final_result(
                if correct {
                    Outcome::Correct
                } else {
                    Outcome::Incorrect
                },
                if correct { 1.0 } else { 0.0 },
                assisted,
                correct_feedback(correct, assisted),
                (!correct).then(|| question.answer.display_answer()),
            ))
        }
        (
            AnswerSpec::MultipleChoice {
                correct_ids,
                choices: _,
            },
            Response::Choice { selected_ids },
        ) => {
            let correct_set: std::collections::BTreeSet<&String> = correct_ids.iter().collect();
            let selected_set: std::collections::BTreeSet<&String> = selected_ids.iter().collect();
            let (outcome, score) = if correct_set == selected_set {
                (Outcome::Correct, 1.0)
            } else if selected_set.is_empty() {
                (Outcome::Incorrect, 0.0)
            } else {
                let tp = correct_set.intersection(&selected_set).count() as f32;
                let union = correct_set.union(&selected_set).count() as f32;
                let score = if union == 0.0 { 0.0 } else { tp / union };
                if score > 0.0 {
                    (Outcome::Partial, score)
                } else {
                    (Outcome::Incorrect, 0.0)
                }
            };
            Grading::Final(final_result(
                outcome,
                score,
                assisted,
                correct_feedback(outcome == Outcome::Correct, assisted),
                (outcome != Outcome::Correct).then(|| question.answer.display_answer()),
            ))
        }
        (AnswerSpec::TrueFalse { correct }, Response::Choice { selected_ids }) => {
            let selected = selected_ids.first().map(String::as_str);
            let is_correct = matches!(
                (correct, selected),
                (true, Some("true")) | (false, Some("false"))
            );
            Grading::Final(final_result(
                if is_correct {
                    Outcome::Correct
                } else {
                    Outcome::Incorrect
                },
                if is_correct { 1.0 } else { 0.0 },
                assisted,
                correct_feedback(is_correct, assisted),
                (!is_correct).then(|| question.answer.display_answer()),
            ))
        }
        (
            AnswerSpec::ShortAnswer {
                accepted,
                near_miss_aliases,
            },
            Response::Text { text },
        ) => {
            let normalized = normalize_answer(text);
            let all_accepted = accepted
                .iter()
                .chain(near_miss_aliases.iter())
                .map(|value| normalize_answer(value))
                .collect::<Vec<_>>();
            if all_accepted
                .iter()
                .any(|candidate| candidate == &normalized)
            {
                Grading::Final(final_result(
                    Outcome::Correct,
                    1.0,
                    assisted,
                    correct_feedback(true, assisted),
                    None,
                ))
            } else if !near_miss_retry
                && all_accepted
                    .iter()
                    .any(|candidate| is_near_text_match(candidate, &normalized))
            {
                Grading::NearMiss {
                    message: "表記が近いようです。もう一度入力してください。".to_owned(),
                }
            } else {
                Grading::Final(final_result(
                    Outcome::Incorrect,
                    0.0,
                    assisted,
                    correct_feedback(false, assisted),
                    Some(question.answer.display_answer()),
                ))
            }
        }
        (AnswerSpec::Numeric(spec), Response::Text { text }) => {
            grade_numeric(spec, text, assisted, near_miss_retry, question)
        }
        (AnswerSpec::FillBlank { .. }, _)
        | (AnswerSpec::Ordering(_), _)
        | (AnswerSpec::Matching(_), _)
        | (AnswerSpec::Hotspot(_), _)
        | (AnswerSpec::Labeling(_), _) => Grading::Final(ungradable(
            "この回答形式の自動採点は Phase 2 で実装します。",
            question,
        )),
        _ => Grading::Final(ungradable("回答形式と回答データが一致しません。", question)),
    }
}

fn grade_numeric(
    spec: &NumericSpec,
    raw: &str,
    assisted: bool,
    near_miss_retry: bool,
    question: &Question,
) -> Grading {
    let Some((value, unit)) = parse_numeric_input(raw) else {
        return Grading::Final(final_result(
            Outcome::Incorrect,
            0.0,
            assisted,
            "数値として読み取れませんでした。".to_owned(),
            Some(question.answer.display_answer()),
        ));
    };

    let mut canonical = value;
    let mut unit_problem = None;
    if let Some(expected_unit) = &spec.unit {
        match unit.as_deref().and_then(normalize_unit).as_deref() {
            Some(unit) => match expected_unit
                .accepted
                .iter()
                .find(|conversion| normalize_unit(&conversion.unit).as_deref() == Some(unit))
            {
                Some(conversion) => canonical = value * conversion.factor_to_canonical,
                None => unit_problem = Some(format!("単位は {} です。", expected_unit.canonical)),
            },
            None if expected_unit.required => {
                unit_problem = Some(format!(
                    "単位を付けてください（{}）。",
                    expected_unit.canonical
                ))
            }
            None => {}
        }
    }

    if let Some(message) = unit_problem {
        if !near_miss_retry {
            return Grading::NearMiss { message };
        }
        return Grading::Final(final_result(
            Outcome::Incorrect,
            0.0,
            true,
            message,
            Some(question.answer.display_answer()),
        ));
    }

    let correct = within_tolerance(canonical, spec);
    if correct {
        if !near_miss_retry
            && let Some(target) = spec.significant_figures
            && let Some(actual) = significant_figures(raw)
            && actual != target
        {
            return Grading::NearMiss {
                message: format!("有効数字を確認してください（{}桁で答えます）。", target),
            };
        }
        return Grading::Final(final_result(
            Outcome::Correct,
            1.0,
            assisted,
            correct_feedback(true, assisted),
            None,
        ));
    }

    if !near_miss_retry && is_rounding_near_miss(canonical, spec) {
        return Grading::NearMiss {
            message: "丸めや単位換算を確認してください。".to_owned(),
        };
    }

    Grading::Final(final_result(
        Outcome::Incorrect,
        0.0,
        assisted,
        correct_feedback(false, assisted),
        Some(question.answer.display_answer()),
    ))
}

pub fn flashcard_grade(assessment: SelfAssessment) -> GradeResult {
    let rating = assessment.rating();
    let correct = assessment != SelfAssessment::Again;
    GradeResult {
        outcome: if correct {
            Outcome::Correct
        } else {
            Outcome::Incorrect
        },
        score: if correct { 1.0 } else { 0.0 },
        rating,
        assisted: false,
        feedback: match assessment {
            SelfAssessment::Again => "もう一度：次回は短い間隔で出題します。".to_owned(),
            SelfAssessment::Good => "できた：自力で思い出せたとして記録しました。".to_owned(),
            SelfAssessment::Easy => "簡単：長めの間隔で出題します。".to_owned(),
        },
        expected: None,
    }
}

fn final_result(
    outcome: Outcome,
    score: f32,
    assisted: bool,
    feedback: String,
    expected: Option<String>,
) -> GradeResult {
    let rating = if outcome == Outcome::Correct && !assisted {
        Rating::Good
    } else {
        Rating::Again
    };
    GradeResult {
        outcome,
        score,
        rating,
        assisted,
        feedback,
        expected,
    }
}

fn correct_feedback(correct: bool, assisted: bool) -> String {
    match (correct, assisted) {
        (true, false) => "正解です。自力で答えられました。".to_owned(),
        (true, true) => {
            "正解です。ただしヒントまたは再入力の補助があります。自力正解としては扱いません。"
                .to_owned()
        }
        (false, true) => "不正解です。ヒントまたは再入力の補助がありました。".to_owned(),
        (false, false) => "不正解です。解説を確認しましょう。".to_owned(),
    }
}

fn ungradable(message: &str, question: &Question) -> GradeResult {
    GradeResult {
        outcome: Outcome::Ungradable,
        score: 0.0,
        rating: Rating::Again,
        assisted: false,
        feedback: message.to_owned(),
        expected: Some(question.answer.display_answer()),
    }
}

fn set_equal(expected: &[String], actual: &[String]) -> bool {
    if expected.len() != actual.len() {
        return false;
    }
    let mut expected = expected.to_vec();
    let mut actual = actual.to_vec();
    expected.sort();
    actual.sort();
    expected == actual
}

fn within_tolerance(value: f64, spec: &NumericSpec) -> bool {
    let difference = (value - spec.value).abs();
    let absolute = spec.tolerance.absolute.unwrap_or(0.0);
    let relative = spec
        .tolerance
        .relative
        .map(|relative| relative * spec.value.abs())
        .unwrap_or(0.0);
    difference <= absolute.max(relative)
}

fn is_rounding_near_miss(value: f64, spec: &NumericSpec) -> bool {
    if spec.value == 0.0 {
        return false;
    }
    let relative_error = (value - spec.value).abs() / spec.value.abs();
    relative_error <= 0.05
}

/// NFKC, lowercase, remove all whitespace, and drop trailing sentence
/// punctuation. This is intentionally conservative so that near-miss handling
/// remains explicit.
pub fn normalize_answer(value: &str) -> String {
    let normalized = value
        .nfkc()
        .collect::<String>()
        .to_lowercase()
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    normalized
        .trim_end_matches(['。', '.', '、', ',', '!', '?', '！', '？', '．', '，'])
        .to_owned()
}

fn is_near_text_match(expected: &str, actual: &str) -> bool {
    if expected.is_empty() || actual.is_empty() {
        return false;
    }
    let expected_len = expected.chars().count();
    let actual_len = actual.chars().count();
    if expected_len.max(actual_len) < 4 {
        return false;
    }
    let distance = levenshtein(expected, actual);
    if distance <= 1 {
        return true;
    }
    // A short extra suffix such as "作用" should prompt one retry, not count as
    // correct. Limit the allowance to two characters.
    if (expected.contains(actual) || actual.contains(expected))
        && expected_len.abs_diff(actual_len) <= 2
    {
        return true;
    }
    false
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a = a.chars().collect::<Vec<_>>();
    let b = b.chars().collect::<Vec<_>>();
    let mut previous = (0..=b.len()).collect::<Vec<_>>();
    for (i, a_char) in a.iter().enumerate() {
        let mut current = vec![i + 1; b.len() + 1];
        for (j, b_char) in b.iter().enumerate() {
            let substitution = previous[j] + usize::from(a_char != b_char);
            current[j + 1] = (current[j] + 1).min(previous[j + 1] + 1).min(substitution);
        }
        previous = current;
    }
    previous[b.len()]
}

/// Parse a leading number and the remaining unit text.
pub fn parse_numeric_input(raw: &str) -> Option<(f64, Option<String>)> {
    let text = raw.trim().replace(',', "");
    if text.is_empty() {
        return None;
    }
    let mut end = 0;
    let mut seen_digit = false;
    for (index, character) in text.char_indices() {
        let is_number_char = character.is_ascii_digit()
            || character == '.'
            || character == '+'
            || character == '-'
            || character == 'e'
            || character == 'E';
        if is_number_char {
            if character.is_ascii_digit() {
                seen_digit = true;
            }
            end = index + character.len_utf8();
        } else {
            break;
        }
    }
    if !seen_digit {
        return None;
    }
    let number = text[..end].parse::<f64>().ok()?;
    let unit = text[end..].trim();
    Some((number, (!unit.is_empty()).then(|| unit.to_owned())))
}

pub fn normalize_unit(unit: &str) -> Option<String> {
    let normalized = unit
        .nfkc()
        .collect::<String>()
        .to_lowercase()
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>()
        .replace('^', "")
        .replace(['·', '⋅'], "・");
    (!normalized.is_empty()).then_some(normalized)
}

pub fn significant_figures(raw: &str) -> Option<u8> {
    let (value, _) = parse_numeric_input(raw)?;
    if value == 0.0 {
        return Some(1);
    }
    let text = raw.trim().replace(',', "");
    let mut number = String::new();
    for character in text.chars() {
        if character.is_ascii_digit() || character == '.' {
            number.push(character);
        } else {
            break;
        }
    }
    let mut parts = number.split('.');
    let whole = parts.next().unwrap_or("").trim_start_matches('0');
    let fraction = parts.next().unwrap_or("");
    let digits = if number.contains('.') {
        let mut combined = String::new();
        if !whole.is_empty() {
            combined.push_str(whole);
        }
        combined.push_str(fraction);
        combined.trim_start_matches('0').to_owned()
    } else {
        let trimmed = whole.trim_end_matches('0');
        if trimmed.is_empty() {
            "0".to_owned()
        } else {
            trimmed.to_owned()
        }
    };
    Some(digits.chars().count().max(1) as u8)
}

pub fn now() -> DateTime<Utc> {
    Utc::now()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::ContentBlock;
    use crate::domain::{Difficulty, LearningKind, QuestionId, WorkbookId};
    use crate::domain::{Tolerance, UnitSpec};
    use chrono::Utc;

    fn question(form: AnswerForm, answer: AnswerSpec) -> Question {
        Question {
            id: QuestionId::new("q"),
            workbook_id: WorkbookId::new("w"),
            form,
            kind: LearningKind::Recall,
            difficulty: Difficulty::Beginner,
            prompt: vec![ContentBlock::text("prompt")],
            answer,
            explanation: vec![ContentBlock::text("explanation")],
            hint: Some("hint".to_owned()),
            card_id: None,
            skill_id: None,
            tags: vec![],
            estimated_seconds: 60,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn short_answer_normalization_and_near_miss() {
        let q = question(
            AnswerForm::ShortAnswer,
            AnswerSpec::ShortAnswer {
                accepted: vec!["光合成".to_owned()],
                near_miss_aliases: vec![],
            },
        );
        let correct = grade(
            &q,
            &Response::Text {
                text: " 光合成。 ".to_owned(),
            },
            false,
            false,
        );
        assert!(matches!(
            correct,
            Grading::Final(GradeResult {
                outcome: Outcome::Correct,
                assisted: false,
                ..
            })
        ));

        let near = grade(
            &q,
            &Response::Text {
                text: "光合成作用".to_owned(),
            },
            false,
            false,
        );
        assert!(matches!(near, Grading::NearMiss { .. }));

        let assisted = grade(
            &q,
            &Response::Text {
                text: "光合成".to_owned(),
            },
            false,
            true,
        );
        assert!(matches!(
            assisted,
            Grading::Final(GradeResult {
                outcome: Outcome::Correct,
                assisted: true,
                rating: Rating::Again,
                ..
            })
        ));
    }

    #[test]
    fn numeric_tolerance_units_and_significant_figures() {
        let q = question(
            AnswerForm::Numeric,
            AnswerSpec::Numeric(NumericSpec {
                value: 2.7,
                tolerance: Tolerance::absolute(0.01),
                unit: Some(
                    UnitSpec::new("g/cm3")
                        .with("kg/m3", 0.001)
                        .with("g/ml", 1.0),
                ),
                significant_figures: Some(2),
            }),
        );

        assert!(matches!(
            grade(
                &q,
                &Response::Text {
                    text: "2.7 g/cm3".to_owned()
                },
                false,
                false
            ),
            Grading::Final(GradeResult {
                outcome: Outcome::Correct,
                ..
            })
        ));
        assert!(matches!(
            grade(
                &q,
                &Response::Text {
                    text: "2.7".to_owned()
                },
                false,
                false
            ),
            Grading::NearMiss { .. }
        ));
        assert!(matches!(
            grade(
                &q,
                &Response::Text {
                    text: "2700 kg/m3".to_owned()
                },
                false,
                false
            ),
            Grading::Final(GradeResult {
                outcome: Outcome::Correct,
                ..
            })
        ));
        assert!(matches!(
            grade(
                &q,
                &Response::Text {
                    text: "2.8 g/cm3".to_owned()
                },
                false,
                false
            ),
            Grading::NearMiss { .. }
        ));
    }

    #[test]
    fn hint_correct_is_not_self_correct() {
        let q = question(
            AnswerForm::SingleChoice,
            AnswerSpec::SingleChoice {
                choices: vec![],
                correct_ids: vec!["a".to_owned()],
            },
        );
        let result = grade(
            &q,
            &Response::Choice {
                selected_ids: vec!["a".to_owned()],
            },
            true,
            false,
        );
        let Grading::Final(result) = result else {
            panic!("expected final grade");
        };
        assert!(result.is_correct());
        assert!(result.assisted);
        assert_eq!(result.rating, Rating::Again);
    }

    #[test]
    fn flashcard_self_assessment_maps_rating() {
        let result = flashcard_grade(SelfAssessment::Easy);
        assert_eq!(result.rating, Rating::Easy);
        assert!(result.is_correct());
    }

    #[test]
    fn significant_figure_counting() {
        assert_eq!(significant_figures("0.00270"), Some(3));
        assert_eq!(significant_figures("2.70"), Some(3));
        assert_eq!(significant_figures("2700"), Some(2));
    }
}
