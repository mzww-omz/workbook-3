//! Development/sample content. This is intentionally small, original, and
//! cross-subject. It is written only into the app's dedicated database after
//! checking that the database is empty.

use anyhow::Result;
use chrono::{Duration, Utc};
use rusqlite::{Transaction, params};

use crate::content::ContentBlock;
use crate::domain::{
    AnswerForm, AnswerSpec, Choice, Difficulty, LearningKind, NumericSpec, Question, Tag, TagKind,
    Tolerance, UnitSpec, Workbook,
};
use crate::storage::fmt_time;

pub const SEED_VERSION: &str = "1";

pub fn seed_sample_content(tx: &Transaction<'_>) -> Result<()> {
    let marker: Option<String> = tx
        .query_row(
            "SELECT value FROM app_meta WHERE key = 'sample_seed_version'",
            [],
            |row| row.get(0),
        )
        .ok();
    if marker.is_some() {
        return Ok(());
    }

    let existing: i64 = tx.query_row("SELECT count(*) FROM workbooks", [], |row| row.get(0))?;
    if existing > 0 {
        tx.execute(
            "INSERT INTO app_meta(key, value) VALUES ('sample_seed_version', 'skipped-existing')",
            [],
        )?;
        return Ok(());
    }

    let now = Utc::now();
    let tags = tags();
    for tag in &tags {
        tx.execute(
            "INSERT INTO tags(id, kind, name) VALUES (?1, ?2, ?3)",
            params![tag.id.as_str(), tag.kind.as_str(), tag.name.as_str()],
        )?;
    }

    let workbooks = [
        Workbook {
            id: "wb-01-math".into(),
            title: "数学：一次関数".to_owned(),
            description: "傾きと切片、変化の割合を、式・数値・文章題で確認します。".to_owned(),
            created_at: now,
        },
        Workbook {
            id: "wb-02-english".into(),
            title: "英語：現在完了".to_owned(),
            description: "have/has + 過去分詞。経験・継続・完了の基本を学びます。".to_owned(),
            created_at: now + Duration::seconds(1),
        },
        Workbook {
            id: "wb-03-japanese".into(),
            title: "国語：敬語".to_owned(),
            description: "尊敬語・謙譲語・丁寧語を、場面に応じて使い分けます。".to_owned(),
            created_at: now + Duration::seconds(2),
        },
        Workbook {
            id: "wb-04-science".into(),
            title: "理科：密度".to_owned(),
            description: "密度＝質量÷体積。単位と有効数字にも注意します。".to_owned(),
            created_at: now + Duration::seconds(3),
        },
        Workbook {
            id: "wb-05-social".into(),
            title: "社会：三権分立".to_owned(),
            description: "国会・内閣・裁判所の役割と、抑制均衡のしくみを確認します。".to_owned(),
            created_at: now + Duration::seconds(4),
        },
    ];
    for workbook in &workbooks {
        tx.execute(
            "INSERT INTO workbooks(id, title, description, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![
                workbook.id.as_str(),
                workbook.title,
                workbook.description,
                fmt_time(workbook.created_at)
            ],
        )?;
    }

    let workbook_tags = [
        (
            "wb-01-math",
            vec!["subject:math", "grade:jhs2", "unit:linear-function"],
        ),
        (
            "wb-02-english",
            vec!["subject:english", "grade:jhs3", "unit:present-perfect"],
        ),
        (
            "wb-03-japanese",
            vec!["subject:japanese", "grade:jhs2", "unit:keigo"],
        ),
        (
            "wb-04-science",
            vec!["subject:science", "grade:jhs1", "unit:density"],
        ),
        (
            "wb-05-social",
            vec!["subject:social", "grade:jhs3", "unit:separation-of-powers"],
        ),
    ];
    for (workbook, tag_ids) in workbook_tags {
        for tag_id in tag_ids {
            tx.execute(
                "INSERT INTO workbook_tags(workbook_id, tag_id) VALUES (?1, ?2)",
                params![workbook, tag_id],
            )?;
        }
    }

    insert_card(
        tx,
        "card-math-linear",
        "wb-01-math",
        "一次関数の読み方",
        vec![
            ContentBlock::heading(2, "y = ax + b"),
            ContentBlock::text(
                "一次関数 y = ax + b では、a を傾き、b を切片といいます。x が1増えるとき、y は a だけ増えます。",
            ),
            ContentBlock::markdown(
                "変化の割合 = Δy / Δx = a\n\n傾きが正なら右上がり、負なら右下がりです。",
            ),
        ],
        now,
    )?;
    insert_card(
        tx,
        "card-english-perfect",
        "wb-02-english",
        "現在完了の3つの使い方",
        vec![
            ContentBlock::heading(2, "have / has + 過去分詞"),
            ContentBlock::text(
                "経験「〜したことがある」、継続「ずっと〜している」、完了「ちょうど〜した」を表します。",
            ),
            ContentBlock::text("主語が he / she / it のときは has を使います。"),
        ],
        now,
    )?;
    insert_card(
        tx,
        "card-japanese-keigo",
        "wb-03-japanese",
        "敬語の3分類",
        vec![
            ContentBlock::heading(2, "尊敬語・謙譲語・丁寧語"),
            ContentBlock::text(
                "尊敬語は相手の動作を高め、謙譲語は自分の動作を低めて相手を立てます。",
            ),
            ContentBlock::text(
                "丁寧語は「です・ます・ございます」など、聞き手に丁寧に伝える言葉です。",
            ),
        ],
        now,
    )?;
    insert_card(
        tx,
        "card-science-density",
        "wb-04-science",
        "密度の求め方",
        vec![
            ContentBlock::heading(2, "密度 = 質量 ÷ 体積"),
            ContentBlock::text(
                "単位は g/cm³ や kg/m³ などで表します。同じ体積なら、質量が大きいほど密度が大きい物質です。",
            ),
            ContentBlock::markdown("例：質量 27 g、体積 10 cm³ → 27 ÷ 10 = 2.7 g/cm³"),
        ],
        now,
    )?;
    insert_card(
        tx,
        "card-social-powers",
        "wb-05-social",
        "三権分立と抑制均衡",
        vec![
            ContentBlock::heading(2, "国会・内閣・裁判所"),
            ContentBlock::text("国会は立法、内閣は行政、裁判所は司法を担当します。"),
            ContentBlock::text("それぞれの権力が互いにチェックし合うしくみを抑制均衡といいます。"),
        ],
        now,
    )?;

    let questions = sample_questions(now);
    for question in &questions {
        insert_question(tx, question)?;
    }

    tx.execute(
        "INSERT INTO app_meta(key, value) VALUES ('sample_seed_version', ?1)",
        params![SEED_VERSION],
    )?;
    Ok(())
}

fn tags() -> Vec<Tag> {
    let entries: &[(&str, TagKind, &str)] = &[
        ("subject:math", TagKind::Subject, "数学"),
        ("subject:english", TagKind::Subject, "英語"),
        ("subject:japanese", TagKind::Subject, "国語"),
        ("subject:science", TagKind::Subject, "理科"),
        ("subject:social", TagKind::Subject, "社会"),
        ("grade:jhs1", TagKind::Grade, "中学1年"),
        ("grade:jhs2", TagKind::Grade, "中学2年"),
        ("grade:jhs3", TagKind::Grade, "中学3年"),
        ("unit:linear-function", TagKind::Unit, "一次関数"),
        ("unit:present-perfect", TagKind::Unit, "現在完了"),
        ("unit:keigo", TagKind::Unit, "敬語"),
        ("unit:density", TagKind::Unit, "密度"),
        ("unit:separation-of-powers", TagKind::Unit, "三権分立"),
        (
            "skill:math-linear-slope",
            TagKind::Skill,
            "一次関数の傾きを求める",
        ),
        (
            "skill:english-present-perfect",
            TagKind::Skill,
            "現在完了を文脈で選ぶ",
        ),
        (
            "skill:japanese-keigo",
            TagKind::Skill,
            "場面に合う敬語を選ぶ",
        ),
        (
            "skill:science-density",
            TagKind::Skill,
            "密度を使って量を求める",
        ),
        (
            "skill:social-powers",
            TagKind::Skill,
            "三権の役割を説明する",
        ),
    ];
    entries
        .iter()
        .map(|(id, kind, name)| Tag {
            id: (*id).into(),
            kind: *kind,
            name: (*name).to_owned(),
        })
        .collect()
}

fn insert_card(
    tx: &Transaction<'_>,
    id: &str,
    workbook_id: &str,
    title: &str,
    blocks: Vec<ContentBlock>,
    created_at: chrono::DateTime<Utc>,
) -> Result<()> {
    tx.execute(
        "INSERT INTO learning_cards(id, workbook_id, title, blocks_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            id,
            workbook_id,
            title,
            serde_json::to_string(&blocks)?,
            fmt_time(created_at)
        ],
    )?;
    Ok(())
}

fn insert_question(tx: &Transaction<'_>, question: &Question) -> Result<()> {
    tx.execute(
        "INSERT INTO questions(
            id, workbook_id, card_id, skill_id, form, kind, difficulty,
            prompt_json, answer_json, explanation_json, hint, tags_json,
            estimated_seconds, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            question.id.as_str(),
            question.workbook_id.as_str(),
            question.card_id.as_ref().map(|id| id.as_str()),
            question.skill_id.as_ref().map(|id| id.as_str()),
            question.form.as_str(),
            question.kind.as_str(),
            question.difficulty.as_str(),
            serde_json::to_string(&question.prompt)?,
            serde_json::to_string(&question.answer)?,
            serde_json::to_string(&question.explanation)?,
            question.hint,
            serde_json::to_string(
                &question
                    .tags
                    .iter()
                    .map(|tag| tag.as_str())
                    .collect::<Vec<_>>()
            )?,
            question.estimated_seconds,
            fmt_time(question.created_at),
        ],
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn question(
    id: &str,
    workbook_id: &str,
    form: AnswerForm,
    kind: LearningKind,
    difficulty: Difficulty,
    prompt: &str,
    answer: AnswerSpec,
    explanation: &str,
    hint: &str,
    card_id: &str,
    skill_id: Option<&str>,
    tags: &[&str],
    seconds: u32,
    created_at: chrono::DateTime<Utc>,
) -> Question {
    Question {
        id: id.into(),
        workbook_id: workbook_id.into(),
        form,
        kind,
        difficulty,
        prompt: vec![ContentBlock::text(prompt)],
        answer,
        explanation: vec![ContentBlock::text(explanation)],
        hint: Some(hint.to_owned()),
        card_id: Some(card_id.into()),
        skill_id: skill_id.map(Into::into),
        tags: tags.iter().map(|tag| (*tag).into()).collect(),
        estimated_seconds: seconds,
        created_at,
    }
}

fn correct(id: &str) -> Vec<String> {
    vec![id.to_owned()]
}

fn sample_questions(now: chrono::DateTime<Utc>) -> Vec<Question> {
    let time = |offset: i64| now + Duration::seconds(offset);
    vec![
        // 数学
        question(
            "q-math-slope-choice",
            "wb-01-math",
            AnswerForm::SingleChoice,
            LearningKind::Recall,
            Difficulty::Beginner,
            "一次関数 y = 2x + 3 の傾きはどれですか。",
            AnswerSpec::SingleChoice {
                choices: vec![
                    Choice {
                        id: "a".into(),
                        text: "2".into(),
                    },
                    Choice {
                        id: "b".into(),
                        text: "3".into(),
                    },
                    Choice {
                        id: "c".into(),
                        text: "-2".into(),
                    },
                    Choice {
                        id: "d".into(),
                        text: "1/2".into(),
                    },
                ],
                correct_ids: correct("a"),
            },
            "y = ax + b の a が傾きです。この式では a = 2 です。",
            "x の係数を見ます。",
            "card-math-linear",
            None,
            &["subject:math", "grade:jhs2", "unit:linear-function"],
            45,
            time(10),
        ),
        question(
            "q-math-value-numeric",
            "wb-01-math",
            AnswerForm::Numeric,
            LearningKind::Recall,
            Difficulty::Beginner,
            "一次関数 y = 2x + 3 で、x = 4 のときの y の値を求めなさい。",
            AnswerSpec::Numeric(NumericSpec {
                value: 11.0,
                tolerance: Tolerance::exact(),
                unit: None,
                significant_figures: None,
            }),
            "y = 2 × 4 + 3 = 8 + 3 = 11 です。",
            "x に 4 を代入します。",
            "card-math-linear",
            None,
            &["subject:math", "grade:jhs2", "unit:linear-function"],
            60,
            time(11),
        ),
        question(
            "q-math-slope-skill",
            "wb-01-math",
            AnswerForm::ShortAnswer,
            LearningKind::Skill,
            Difficulty::Intermediate,
            "一次関数 y = 3x - 2 で、x が1増えると y はどれだけ増えますか。数値で答えなさい。",
            AnswerSpec::ShortAnswer {
                accepted: vec!["3".into(), "3だけ".into(), "3増える".into()],
                near_miss_aliases: vec![],
            },
            "変化の割合は傾き a に等しいので、x が1増えると y は3増えます。",
            "変化の割合と傾きの関係を思い出します。",
            "card-math-linear",
            Some("skill:math-linear-slope"),
            &[
                "subject:math",
                "grade:jhs2",
                "unit:linear-function",
                "skill:math-linear-slope",
            ],
            60,
            time(12),
        ),
        question(
            "q-math-slope-from-points",
            "wb-01-math",
            AnswerForm::Numeric,
            LearningKind::Skill,
            Difficulty::Advanced,
            "2点 (2, 5) と (4, 1) を通る直線の傾きを求めなさい。",
            AnswerSpec::Numeric(NumericSpec {
                value: -2.0,
                tolerance: Tolerance::exact(),
                unit: None,
                significant_figures: None,
            }),
            "傾き = (1 - 5) / (4 - 2) = -4 / 2 = -2 です。",
            "(y の変化) ÷ (x の変化) を計算します。",
            "card-math-linear",
            Some("skill:math-linear-slope"),
            &[
                "subject:math",
                "grade:jhs2",
                "unit:linear-function",
                "skill:math-linear-slope",
            ],
            90,
            time(13),
        ),
        question(
            "q-math-flashcard",
            "wb-01-math",
            AnswerForm::Flashcard,
            LearningKind::Recall,
            Difficulty::Beginner,
            "一次関数 y = ax + b の「変化の割合」は何を表しますか。",
            AnswerSpec::Flashcard {
                front: "一次関数 y = ax + b の変化の割合は？".into(),
                back: "a（傾き）。x が1増えるときの y の増加量。".into(),
            },
            "変化の割合は Δy / Δx で、一次関数では常に傾き a に等しくなります。",
            "x が1増えたときの y の増え方を考えます。",
            "card-math-linear",
            None,
            &["subject:math", "grade:jhs2", "unit:linear-function"],
            45,
            time(14),
        ),
        // 英語
        question(
            "q-eng-have-choice",
            "wb-02-english",
            AnswerForm::SingleChoice,
            LearningKind::Recall,
            Difficulty::Beginner,
            "I ___ lived here for five years. に入る語はどれですか。",
            AnswerSpec::SingleChoice {
                choices: vec![
                    Choice {
                        id: "a".into(),
                        text: "have".into(),
                    },
                    Choice {
                        id: "b".into(),
                        text: "has".into(),
                    },
                    Choice {
                        id: "c".into(),
                        text: "am".into(),
                    },
                    Choice {
                        id: "d".into(),
                        text: "do".into(),
                    },
                ],
                correct_ids: correct("a"),
            },
            "主語 I には have を使います。現在完了の継続用法です。",
            "主語と have/has の対応を確認します。",
            "card-english-perfect",
            None,
            &["subject:english", "grade:jhs3", "unit:present-perfect"],
            45,
            time(20),
        ),
        question(
            "q-eng-finished-short",
            "wb-02-english",
            AnswerForm::ShortAnswer,
            LearningKind::Recall,
            Difficulty::Intermediate,
            "「彼はもう宿題を終えた」を、現在完了を使って英語で書きなさい。",
            AnswerSpec::ShortAnswer {
                accepted: vec![
                    "He has already finished his homework.".into(),
                    "He has already finished his homework".into(),
                    "He's already finished his homework.".into(),
                ],
                near_miss_aliases: vec![],
            },
            "完了用法では have/has + 過去分詞 + already を使います。He has already finished his homework.",
            "主語は He、already は have/has と過去分詞の間に置きます。",
            "card-english-perfect",
            None,
            &["subject:english", "grade:jhs3", "unit:present-perfect"],
            75,
            time(21),
        ),
        question(
            "q-eng-she-just",
            "wb-02-english",
            AnswerForm::ShortAnswer,
            LearningKind::Skill,
            Difficulty::Intermediate,
            "She ___ just arrived. の空所に have か has のどちらかを書きなさい。",
            AnswerSpec::ShortAnswer {
                accepted: vec!["has".into()],
                near_miss_aliases: vec![],
            },
            "主語 She は三人称単数なので has を使います。",
            "主語の人称と数を確認します。",
            "card-english-perfect",
            Some("skill:english-present-perfect"),
            &[
                "subject:english",
                "grade:jhs3",
                "unit:present-perfect",
                "skill:english-present-perfect",
            ],
            45,
            time(22),
        ),
        question(
            "q-eng-flashcard",
            "wb-02-english",
            AnswerForm::Flashcard,
            LearningKind::Recall,
            Difficulty::Beginner,
            "現在完了の「経験」の基本形は？",
            AnswerSpec::Flashcard {
                front: "「〜したことがある」を表す現在完了の形は？".into(),
                back: "have/has + 過去分詞（例：I have visited Kyoto.）".into(),
            },
            "経験は「have/has + 過去分詞」で表し、回数を表す語とよく使われます。",
            "have/has の後ろに何が来るか考えます。",
            "card-english-perfect",
            None,
            &["subject:english", "grade:jhs3", "unit:present-perfect"],
            45,
            time(23),
        ),
        // 国語
        question(
            "q-jpn-sonkeigo-choice",
            "wb-03-japanese",
            AnswerForm::SingleChoice,
            LearningKind::Recall,
            Difficulty::Beginner,
            "「先生が言う」の「言う」を尊敬語にしたものはどれですか。",
            AnswerSpec::SingleChoice {
                choices: vec![
                    Choice {
                        id: "a".into(),
                        text: "おっしゃる".into(),
                    },
                    Choice {
                        id: "b".into(),
                        text: "申す".into(),
                    },
                    Choice {
                        id: "c".into(),
                        text: "言います".into(),
                    },
                    Choice {
                        id: "d".into(),
                        text: "伺う".into(),
                    },
                ],
                correct_ids: correct("a"),
            },
            "「言う」の尊敬語は「おっしゃる」です。「申す」は謙譲語です。",
            "相手の動作を高める言い方を探します。",
            "card-japanese-keigo",
            None,
            &["subject:japanese", "grade:jhs2", "unit:keigo"],
            45,
            time(30),
        ),
        question(
            "q-jpn-kenjougo-short",
            "wb-03-japanese",
            AnswerForm::ShortAnswer,
            LearningKind::Recall,
            Difficulty::Intermediate,
            "「見る」の謙譲語を書きなさい。",
            AnswerSpec::ShortAnswer {
                accepted: vec!["拝見する".into(), "拝見します".into()],
                near_miss_aliases: vec![],
            },
            "「見る」の謙譲語は「拝見する」です。自分の動作を低めて相手を立てます。",
            "自分の動作をへりくだる言い方です。",
            "card-japanese-keigo",
            None,
            &["subject:japanese", "grade:jhs2", "unit:keigo"],
            60,
            time(31),
        ),
        question(
            "q-jpn-keigo-skill",
            "wb-03-japanese",
            AnswerForm::ShortAnswer,
            LearningKind::Skill,
            Difficulty::Advanced,
            "「先生に会う」を、先生を立てる言い方にしなさい。",
            AnswerSpec::ShortAnswer {
                accepted: vec!["お目にかかる".into(), "お目にかかります".into()],
                near_miss_aliases: vec![],
            },
            "「会う」の謙譲語は「お目にかかる」です。",
            "相手を立てるので、自分の動作を低める謙譲語を使います。",
            "card-japanese-keigo",
            Some("skill:japanese-keigo"),
            &[
                "subject:japanese",
                "grade:jhs2",
                "unit:keigo",
                "skill:japanese-keigo",
            ],
            75,
            time(32),
        ),
        question(
            "q-jpn-flashcard",
            "wb-03-japanese",
            AnswerForm::Flashcard,
            LearningKind::Recall,
            Difficulty::Beginner,
            "丁寧語の代表例は？",
            AnswerSpec::Flashcard {
                front: "丁寧語の代表例を3つ挙げると？".into(),
                back: "です・ます・ございます".into(),
            },
            "丁寧語は聞き手に対して丁寧に述べる言葉で、です・ます・ございますなどがあります。",
            "敬語の3分類のうち、聞き手への配慮を表すものです。",
            "card-japanese-keigo",
            None,
            &["subject:japanese", "grade:jhs2", "unit:keigo"],
            45,
            time(33),
        ),
        // 理科
        question(
            "q-sci-density-numeric",
            "wb-04-science",
            AnswerForm::Numeric,
            LearningKind::Recall,
            Difficulty::Beginner,
            "質量 27 g、体積 10 cm³ の物質の密度を、単位を付けて求めなさい。",
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
            "密度 = 27 ÷ 10 = 2.7 g/cm³ です。",
            "質量を体積で割ります。単位も忘れずに。",
            "card-science-density",
            None,
            &["subject:science", "grade:jhs1", "unit:density"],
            75,
            time(40),
        ),
        question(
            "q-sci-density-choice",
            "wb-04-science",
            AnswerForm::SingleChoice,
            LearningKind::Recall,
            Difficulty::Beginner,
            "密度が大きい物質について、正しい説明はどれですか。",
            AnswerSpec::SingleChoice {
                choices: vec![
                    Choice {
                        id: "a".into(),
                        text: "同じ体積なら質量が大きい".into(),
                    },
                    Choice {
                        id: "b".into(),
                        text: "同じ質量なら体積が大きい".into(),
                    },
                    Choice {
                        id: "c".into(),
                        text: "必ず水に浮く".into(),
                    },
                    Choice {
                        id: "d".into(),
                        text: "温度で質量が変わる".into(),
                    },
                ],
                correct_ids: correct("a"),
            },
            "密度 = 質量 ÷ 体積なので、同じ体積なら密度が大きいほど質量も大きくなります。",
            "密度の式を思い出します。",
            "card-science-density",
            None,
            &["subject:science", "grade:jhs1", "unit:density"],
            45,
            time(41),
        ),
        question(
            "q-sci-density-skill",
            "wb-04-science",
            AnswerForm::Numeric,
            LearningKind::Skill,
            Difficulty::Intermediate,
            "密度 2.5 g/cm³ の物質が 8 cm³ あります。質量を単位付きで求めなさい。",
            AnswerSpec::Numeric(NumericSpec {
                value: 20.0,
                tolerance: Tolerance::exact(),
                unit: Some(UnitSpec::new("g").with("kg", 1000.0)),
                significant_figures: None,
            }),
            "質量 = 密度 × 体積 = 2.5 × 8 = 20 g です。",
            "密度の式を質量について変形します。",
            "card-science-density",
            Some("skill:science-density"),
            &[
                "subject:science",
                "grade:jhs1",
                "unit:density",
                "skill:science-density",
            ],
            75,
            time(42),
        ),
        question(
            "q-sci-flashcard",
            "wb-04-science",
            AnswerForm::Flashcard,
            LearningKind::Recall,
            Difficulty::Beginner,
            "密度を求める式は？",
            AnswerSpec::Flashcard {
                front: "密度を求める式は？".into(),
                back: "密度 = 質量 ÷ 体積".into(),
            },
            "密度は単位体積あたりの質量です。",
            "「単位体積あたりの質量」と考えると式が出ます。",
            "card-science-density",
            None,
            &["subject:science", "grade:jhs1", "unit:density"],
            45,
            time(43),
        ),
        // 社会
        question(
            "q-soc-law-choice",
            "wb-05-social",
            AnswerForm::SingleChoice,
            LearningKind::Recall,
            Difficulty::Beginner,
            "法律を制定するのはどの機関ですか。",
            AnswerSpec::SingleChoice {
                choices: vec![
                    Choice {
                        id: "a".into(),
                        text: "国会".into(),
                    },
                    Choice {
                        id: "b".into(),
                        text: "内閣".into(),
                    },
                    Choice {
                        id: "c".into(),
                        text: "裁判所".into(),
                    },
                    Choice {
                        id: "d".into(),
                        text: "選挙管理委員会".into(),
                    },
                ],
                correct_ids: correct("a"),
            },
            "国会は立法権を持ち、法律を制定します。",
            "三権のうち立法を担当する機関です。",
            "card-social-powers",
            None,
            &["subject:social", "grade:jhs3", "unit:separation-of-powers"],
            45,
            time(50),
        ),
        question(
            "q-soc-confidence-short",
            "wb-05-social",
            AnswerForm::ShortAnswer,
            LearningKind::Recall,
            Difficulty::Intermediate,
            "内閣に対して内閣不信任決議を行うことができるのはどこですか。",
            AnswerSpec::ShortAnswer {
                accepted: vec!["衆議院".into(), "しゅうぎいん".into()],
                near_miss_aliases: vec![],
            },
            "内閣不信任決議は衆議院だけが行えます。",
            "二院制のうち、内閣信任に強く関わる院です。",
            "card-social-powers",
            None,
            &["subject:social", "grade:jhs3", "unit:separation-of-powers"],
            60,
            time(51),
        ),
        question(
            "q-soc-review-skill",
            "wb-05-social",
            AnswerForm::ShortAnswer,
            LearningKind::Skill,
            Difficulty::Advanced,
            "裁判所が法律や命令が憲法に違反していないかを審査する権限を何といいますか。",
            AnswerSpec::ShortAnswer {
                accepted: vec!["違憲審査権".into(), "違憲審査".into()],
                near_miss_aliases: vec![],
            },
            "裁判所の違憲審査権は、立法や行政をチェックする司法の役割です。",
            "「憲法に違反していないか」を審査する権限です。",
            "card-social-powers",
            Some("skill:social-powers"),
            &[
                "subject:social",
                "grade:jhs3",
                "unit:separation-of-powers",
                "skill:social-powers",
            ],
            75,
            time(52),
        ),
        question(
            "q-soc-flashcard",
            "wb-05-social",
            AnswerForm::Flashcard,
            LearningKind::Recall,
            Difficulty::Beginner,
            "三権のうち行政を担うのは？",
            AnswerSpec::Flashcard {
                front: "三権のうち行政を担う機関は？".into(),
                back: "内閣".into(),
            },
            "内閣は行政権を持ち、法律に基づいて政治を執行します。",
            "立法・行政・司法の分担を思い出します。",
            "card-social-powers",
            None,
            &["subject:social", "grade:jhs3", "unit:separation-of-powers"],
            45,
            time(53),
        ),
    ]
}
