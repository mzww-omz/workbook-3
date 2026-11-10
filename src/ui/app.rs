//! GPUI application shell and Phase 1 learning flow.

use std::time::Instant;

use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use gpui::{
    AnyElement, App, Application, Bounds, Context, FocusHandle, Focusable, KeyBinding, Window,
    WindowBounds, WindowOptions, actions, div, prelude::*, px, rgb, size,
};

use crate::domain::{
    AnswerForm, AnswerSpec, AttemptId, LearningCard, Question, QuestionId, Response,
    SelfAssessment, SessionView, WorkbookId,
};
use crate::storage::{
    AttemptDraft, Dashboard, QuestionSummary, Repository, SubmitOutcome, WorkbookDetail,
    WorkbookSummary,
};
use crate::study::{GradeResult, Grading, grade};

use super::components::*;
use super::text_input::{self, TextInput};

actions!(
    workbook3_ui,
    [
        SubmitAnswer,
        NextItem,
        ShowHint,
        RevealFlashcard,
        InterruptStudy,
        Choose1,
        Choose2,
        Choose3,
        Choose4,
        Choose5,
        Choose6,
    ]
);

pub fn app_key_bindings() -> Vec<KeyBinding> {
    let mut bindings = text_input::key_bindings();
    bindings.extend([
        KeyBinding::new("enter", SubmitAnswer, Some("Study && StudyQuestion")),
        KeyBinding::new("enter", NextItem, Some("Study && StudyFeedback")),
        KeyBinding::new("enter", NextItem, Some("Study && StudyCard")),
        KeyBinding::new("enter", NextItem, Some("Study && StudyFinished")),
        KeyBinding::new("f1", ShowHint, Some("Study && StudyQuestion")),
        KeyBinding::new("space", RevealFlashcard, Some("Study && Flashcard")),
        KeyBinding::new("escape", InterruptStudy, Some("Study")),
        KeyBinding::new("1", Choose1, Some("Study && ChoiceInput")),
        KeyBinding::new("2", Choose2, Some("Study && ChoiceInput")),
        KeyBinding::new("3", Choose3, Some("Study && ChoiceInput")),
        KeyBinding::new("4", Choose4, Some("Study && ChoiceInput")),
        KeyBinding::new("5", Choose5, Some("Study && ChoiceInput")),
        KeyBinding::new("6", Choose6, Some("Study && ChoiceInput")),
    ]);
    bindings
}

#[derive(Clone, Debug, PartialEq)]
enum Screen {
    Home,
    Library,
    Plans,
    Workbook(WorkbookId),
    Study,
}

struct FeedbackState {
    result: GradeResult,
    due_at: Option<DateTime<Utc>>,
    skill_mastery: Option<f32>,
    first_exposure: bool,
    note: Option<String>,
}

enum StudyPhase {
    Card {
        card: LearningCard,
        question_id: QuestionId,
    },
    Question {
        question: Question,
    },
    Feedback {
        question: Question,
        feedback: FeedbackState,
    },
    Finished,
}

struct StudyState {
    session: SessionView,
    phase: StudyPhase,
    hint_used: bool,
    hint_visible: bool,
    selected_choice: Option<String>,
    near_miss: Option<String>,
    retry_after_near_miss: bool,
    flashcard_revealed: bool,
    attempt_id: AttemptId,
    question_started: Instant,
}

pub struct AppView {
    repo: Option<Repository>,
    fatal: Option<String>,
    screen: Screen,
    dashboard: Option<Dashboard>,
    workbooks: Vec<WorkbookSummary>,
    plans: Vec<crate::domain::Plan>,
    active_session: Option<SessionView>,
    workbook_detail: Option<WorkbookDetail>,
    study: Option<StudyState>,
    input: gpui::Entity<TextInput>,
    root_focus: FocusHandle,
    notice: Option<String>,
}

impl AppView {
    pub fn new(cx: &mut Context<Self>, repo: Result<Repository>) -> Self {
        let input = cx.new(|cx| TextInput::new(cx, "答えを入力"));
        let root_focus = cx.focus_handle();
        match repo {
            Ok(mut repo) => {
                let seed_error = repo
                    .seed_sample_content()
                    .err()
                    .map(|error| format!("サンプル教材の投入に失敗しました: {error:#}"));
                let mut view = Self {
                    repo: Some(repo),
                    fatal: None,
                    screen: Screen::Home,
                    dashboard: None,
                    workbooks: Vec::new(),
                    plans: Vec::new(),
                    active_session: None,
                    workbook_detail: None,
                    study: None,
                    input,
                    root_focus,
                    notice: seed_error,
                };
                view.refresh();
                view
            }
            Err(error) => Self {
                repo: None,
                fatal: Some(format!("{error:#}")),
                screen: Screen::Home,
                dashboard: None,
                workbooks: Vec::new(),
                plans: Vec::new(),
                active_session: None,
                workbook_detail: None,
                study: None,
                input,
                root_focus,
                notice: None,
            },
        }
    }

    fn refresh(&mut self) {
        let Some(repo) = self.repo.as_ref() else {
            return;
        };
        match repo.dashboard() {
            Ok(dashboard) => self.dashboard = Some(dashboard),
            Err(error) => self.notice = Some(format!("読み込みに失敗しました: {error:#}")),
        }
        match repo.list_workbooks() {
            Ok(workbooks) => self.workbooks = workbooks,
            Err(error) => {
                self.notice = Some(format!("Workbookの読み込みに失敗しました: {error:#}"))
            }
        }
        match repo.list_plans() {
            Ok(plans) => self.plans = plans,
            Err(error) => self.notice = Some(format!("Planの読み込みに失敗しました: {error:#}")),
        }
        match repo.active_session() {
            Ok(session) => self.active_session = session,
            Err(error) => {
                self.notice = Some(format!("セッションの読み込みに失敗しました: {error:#}"))
            }
        }
    }

    fn go_home(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.screen = Screen::Home;
        self.study = None;
        self.workbook_detail = None;
        self.refresh();
        window.focus(&self.root_focus);
        cx.notify();
    }

    fn go_library(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.screen = Screen::Library;
        self.workbook_detail = None;
        self.refresh();
        window.focus(&self.root_focus);
        cx.notify();
    }

    fn go_plans(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.screen = Screen::Plans;
        self.workbook_detail = None;
        self.refresh();
        window.focus(&self.root_focus);
        cx.notify();
    }

    fn open_workbook(&mut self, id: WorkbookId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.as_ref() else {
            return;
        };
        match repo.workbook_detail(&id) {
            Ok(Some(detail)) => {
                self.workbook_detail = Some(detail);
                self.screen = Screen::Workbook(id);
                self.notice = None;
            }
            Ok(None) => self.notice = Some("Workbookが見つかりません。".to_owned()),
            Err(error) => self.notice = Some(format!("Workbookを開けません: {error:#}")),
        }
        window.focus(&self.root_focus);
        cx.notify();
    }

    fn start_session(
        &mut self,
        workbook_id: Option<WorkbookId>,
        minutes: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(repo) = self.repo.as_mut() else {
            return;
        };
        match repo.start_session(workbook_id.as_ref(), minutes, Utc::now()) {
            Ok(session) => {
                self.notice = None;
                self.active_session = Some(session.clone());
                self.begin_study(session, window, cx);
            }
            Err(error) => {
                self.notice = Some(format!("学習を開始できませんでした: {error:#}"));
                cx.notify();
            }
        }
    }

    fn resume_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(repo) = self.repo.as_ref() else {
            return;
        };
        match repo.active_session() {
            Ok(Some(session)) => {
                self.notice = None;
                self.begin_study(session, window, cx);
            }
            Ok(None) => {
                self.notice = Some("再開できるセッションがありません。".to_owned());
                cx.notify();
            }
            Err(error) => {
                self.notice = Some(format!("セッションを再開できません: {error:#}"));
                cx.notify();
            }
        }
    }

    fn begin_study(&mut self, session: SessionView, window: &mut Window, cx: &mut Context<Self>) {
        self.study = Some(StudyState {
            session,
            phase: StudyPhase::Finished,
            hint_used: false,
            hint_visible: false,
            selected_choice: None,
            near_miss: None,
            retry_after_near_miss: false,
            flashcard_revealed: false,
            attempt_id: AttemptId::new(uuid::Uuid::new_v4().to_string()),
            question_started: Instant::now(),
        });
        self.screen = Screen::Study;
        self.advance_study(window, cx);
    }

    fn advance_study(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = self.study.as_ref().map(|study| study.session.clone()) else {
            return;
        };
        if session.session.current_index >= session.items.len() {
            if let Some(repo) = self.repo.as_mut() {
                let _ = repo.finish_session(&session.session.id, Utc::now());
            }
            self.refresh();
            if let Some(study) = self.study.as_mut() {
                study.phase = StudyPhase::Finished;
            }
            window.focus(&self.root_focus);
            cx.notify();
            return;
        }

        let item = session.items[session.session.current_index].clone();
        let loaded = self.repo.as_ref().and_then(|repo| {
            let question = repo.get_question(&item.question_id).ok().flatten()?;
            let card = if item.show_card && !item.card_seen {
                question
                    .card_id
                    .as_ref()
                    .and_then(|card_id| repo.get_card(card_id).ok().flatten())
            } else {
                None
            };
            Some((question, card))
        });
        let Some((question, card)) = loaded else {
            self.notice = Some("問題を読み込めませんでした。".to_owned());
            cx.notify();
            return;
        };

        if let Some(card) = card {
            if let Some(study) = self.study.as_mut() {
                study.phase = StudyPhase::Card {
                    card,
                    question_id: question.id.clone(),
                };
            }
            window.focus(&self.root_focus);
            cx.notify();
            return;
        }

        let is_text = matches!(question.form, AnswerForm::ShortAnswer | AnswerForm::Numeric);
        self.input.update(cx, |input, cx| input.clear(cx));
        if let Some(study) = self.study.as_mut() {
            study.phase = StudyPhase::Question { question };
            study.hint_used = false;
            study.hint_visible = false;
            study.selected_choice = None;
            study.near_miss = None;
            study.retry_after_near_miss = false;
            study.flashcard_revealed = false;
            study.attempt_id = AttemptId::new(uuid::Uuid::new_v4().to_string());
            study.question_started = Instant::now();
        }
        if is_text {
            let handle = self.input.read(cx).focus_handle(cx);
            window.focus(&handle);
        } else {
            window.focus(&self.root_focus);
        }
        cx.notify();
    }

    fn card_next(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((session_id, question_id, index)) =
            self.study.as_ref().and_then(|study| match &study.phase {
                StudyPhase::Card { question_id, .. } => Some((
                    study.session.session.id.clone(),
                    question_id.clone(),
                    study.session.session.current_index,
                )),
                _ => None,
            })
        else {
            return;
        };
        if let Some(repo) = self.repo.as_mut()
            && let Err(error) = repo.mark_card_seen(&session_id, &question_id)
        {
            self.notice = Some(format!("Cardの既読を保存できません: {error:#}"));
        }
        if let Some(study) = self.study.as_mut()
            && let Some(item) = study.session.items.get_mut(index)
        {
            item.card_seen = true;
        }
        self.advance_study(window, cx);
    }

    fn toggle_hint(&mut self, cx: &mut Context<Self>) {
        if let Some(study) = self.study.as_mut() {
            study.hint_visible = !study.hint_visible;
            if study.hint_visible {
                study.hint_used = true;
            }
        }
        cx.notify();
    }

    fn choose(&mut self, index: usize, cx: &mut Context<Self>) {
        let choice_id = self.study.as_ref().and_then(|study| match &study.phase {
            StudyPhase::Question { question } => question
                .answer
                .choices()
                .get(index)
                .map(|choice| choice.id.clone()),
            _ => None,
        });
        if let Some(choice_id) = choice_id
            && let Some(study) = self.study.as_mut()
        {
            study.selected_choice = Some(choice_id);
        }
        cx.notify();
    }

    fn reveal_flashcard(&mut self, cx: &mut Context<Self>) {
        if let Some(study) = self.study.as_mut()
            && matches!(study.phase, StudyPhase::Question { .. })
        {
            study.flashcard_revealed = true;
        }
        cx.notify();
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((question, selected_choice, hint_used, retry, flashcard_revealed)) =
            self.study.as_ref().and_then(|study| match &study.phase {
                StudyPhase::Question { question } => Some((
                    question.clone(),
                    study.selected_choice.clone(),
                    study.hint_used,
                    study.retry_after_near_miss,
                    study.flashcard_revealed,
                )),
                _ => None,
            })
        else {
            return;
        };

        if question.form == AnswerForm::Flashcard {
            if !flashcard_revealed {
                self.reveal_flashcard(cx);
            }
            return;
        }

        let response = match question.form {
            AnswerForm::SingleChoice => match selected_choice {
                Some(selected) => Response::Choice {
                    selected_ids: vec![selected],
                },
                None => {
                    self.notice =
                        Some("選択肢を選んでください（1〜6キーでも選べます）。".to_owned());
                    cx.notify();
                    return;
                }
            },
            AnswerForm::ShortAnswer | AnswerForm::Numeric => {
                let text = self.input.read(cx).text();
                if text.trim().is_empty() {
                    self.notice = Some("回答を入力してください。".to_owned());
                    cx.notify();
                    return;
                }
                Response::Text { text }
            }
            _ => {
                self.notice = Some("この回答形式はPhase 2で対応します。".to_owned());
                cx.notify();
                return;
            }
        };

        let grading = grade(&question, &response, hint_used, retry);
        match grading {
            Grading::NearMiss { message } => {
                if let Some(study) = self.study.as_mut() {
                    study.near_miss = Some(message.clone());
                    study.retry_after_near_miss = true;
                }
                if matches!(question.form, AnswerForm::Numeric | AnswerForm::ShortAnswer) {
                    let handle = self.input.read(cx).focus_handle(cx);
                    window.focus(&handle);
                }
                cx.notify();
            }
            Grading::Final(result) => {
                self.record_final(response, result, window, cx);
            }
        }
    }

    fn submit_flashcard(
        &mut self,
        assessment: SelfAssessment,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(study) = self.study.as_ref() else {
            return;
        };
        let StudyPhase::Question { question } = &study.phase else {
            return;
        };
        let response = Response::Flashcard { assessment };
        let grading = grade(
            question,
            &response,
            study.hint_used,
            study.retry_after_near_miss,
        );
        let Grading::Final(result) = grading else {
            return;
        };
        self.record_final(response, result, window, cx);
    }

    fn record_final(
        &mut self,
        response: Response,
        result: GradeResult,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((
            session_id,
            question_id,
            question,
            attempt_id,
            hint_used,
            retry,
            elapsed_ms,
            position,
        )) = self.study.as_ref().and_then(|study| match &study.phase {
            StudyPhase::Question { question } => Some((
                study.session.session.id.clone(),
                question.id.clone(),
                question.clone(),
                study.attempt_id.clone(),
                study.hint_used,
                study.retry_after_near_miss,
                study.question_started.elapsed().as_millis() as u64,
                study.session.session.current_index,
            )),
            _ => None,
        })
        else {
            return;
        };

        let draft = AttemptDraft {
            attempt_id,
            session_id,
            question_id,
            response,
            outcome: result.outcome,
            rating: result.rating,
            score: result.score,
            hint_used,
            near_miss_retry: retry,
            elapsed_ms,
        };
        let Some(repo) = self.repo.as_mut() else {
            return;
        };
        match repo.submit_attempt(draft, Utc::now()) {
            Ok(SubmitOutcome::Recorded {
                first_exposure,
                due_at,
                skill_mastery,
                ..
            }) => {
                self.apply_feedback(
                    question,
                    result,
                    due_at,
                    skill_mastery,
                    first_exposure,
                    None,
                    position,
                    window,
                    cx,
                );
            }
            Ok(SubmitOutcome::Duplicate { .. }) => {
                self.apply_feedback(
                    question,
                    result,
                    None,
                    None,
                    false,
                    Some("この回答は保存済みです（二重計上していません）。".to_owned()),
                    position,
                    window,
                    cx,
                );
            }
            Ok(SubmitOutcome::AlreadyAnswered { .. }) => {
                self.notice = Some("この問題は回答済みです。二重計上していません。".to_owned());
                self.advance_study(window, cx);
            }
            Err(error) => {
                self.notice = Some(format!("回答を保存できませんでした: {error:#}"));
                cx.notify();
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_feedback(
        &mut self,
        question: Question,
        result: GradeResult,
        due_at: Option<DateTime<Utc>>,
        skill_mastery: Option<f32>,
        first_exposure: bool,
        note: Option<String>,
        position: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(study) = self.study.as_mut() {
            if let Some(item) = study.session.items.get_mut(position) {
                item.answered = true;
            }
            study.near_miss = None;
            study.phase = StudyPhase::Feedback {
                question,
                feedback: FeedbackState {
                    result,
                    due_at,
                    skill_mastery,
                    first_exposure,
                    note,
                },
            };
        }
        self.notice = None;
        self.refresh();
        window.focus(&self.root_focus);
        cx.notify();
    }

    fn next(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(study) = self.study.as_ref() else {
            return;
        };
        match &study.phase {
            StudyPhase::Feedback { .. } => {
                let next_index = study.session.session.current_index + 1;
                if let Some(study) = self.study.as_mut() {
                    study.session.session.current_index = next_index;
                }
                self.advance_study(window, cx);
            }
            StudyPhase::Card { .. } => self.card_next(window, cx),
            StudyPhase::Finished => self.go_home(window, cx),
            StudyPhase::Question { .. } => self.submit(window, cx),
        }
    }

    fn interrupt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.screen = Screen::Home;
        self.study = None;
        self.notice =
            Some("学習を中断しました。回答済みの内容は保存され、Homeから再開できます。".to_owned());
        self.refresh();
        window.focus(&self.root_focus);
        cx.notify();
    }

    fn key_context(&self) -> String {
        if self.fatal.is_some() {
            return "App".to_owned();
        }
        match self.screen {
            Screen::Study => {
                let Some(study) = self.study.as_ref() else {
                    return "Study".to_owned();
                };
                match &study.phase {
                    StudyPhase::Card { .. } => "Study StudyCard".to_owned(),
                    StudyPhase::Question { question } => {
                        let mut context = "Study StudyQuestion".to_owned();
                        if question.form == AnswerForm::SingleChoice {
                            context.push_str(" ChoiceInput");
                        }
                        if question.form == AnswerForm::Flashcard {
                            context.push_str(" Flashcard");
                        }
                        if matches!(question.form, AnswerForm::ShortAnswer | AnswerForm::Numeric) {
                            context.push_str(" TextQuestion");
                        }
                        context
                    }
                    StudyPhase::Feedback { .. } => "Study StudyFeedback".to_owned(),
                    StudyPhase::Finished => "Study StudyFinished".to_owned(),
                }
            }
            _ => "App".to_owned(),
        }
    }
}

impl Render for AppView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let context = self.key_context();
        let content = if let Some(fatal) = &self.fatal {
            self.render_fatal(fatal)
        } else if self.screen == Screen::Study {
            self.render_study(cx)
        } else {
            self.render_shell(cx)
        };
        div()
            .key_context(context.as_str())
            .track_focus(&self.root_focus)
            .size_full()
            .bg(rgb(BG))
            .text_color(rgb(INK))
            .child(content)
            .on_action(
                cx.listener(|this: &mut Self, _: &SubmitAnswer, window, cx| {
                    this.submit(window, cx)
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &NextItem, window, cx| this.next(window, cx)),
            )
            .on_action(cx.listener(|this: &mut Self, _: &ShowHint, _, cx| this.toggle_hint(cx)))
            .on_action(
                cx.listener(|this: &mut Self, _: &RevealFlashcard, _, cx| {
                    this.reveal_flashcard(cx)
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &InterruptStudy, window, cx| {
                    this.interrupt(window, cx)
                }),
            )
            .on_action(cx.listener(|this: &mut Self, _: &Choose1, _, cx| this.choose(0, cx)))
            .on_action(cx.listener(|this: &mut Self, _: &Choose2, _, cx| this.choose(1, cx)))
            .on_action(cx.listener(|this: &mut Self, _: &Choose3, _, cx| this.choose(2, cx)))
            .on_action(cx.listener(|this: &mut Self, _: &Choose4, _, cx| this.choose(3, cx)))
            .on_action(cx.listener(|this: &mut Self, _: &Choose5, _, cx| this.choose(4, cx)))
            .on_action(cx.listener(|this: &mut Self, _: &Choose6, _, cx| this.choose(5, cx)))
    }
}

impl AppView {
    fn render_fatal(&self, message: &str) -> AnyElement {
        div()
            .size_full()
            .p_10()
            .child(
                div()
                    .max_w(px(760.))
                    .child(
                        div()
                            .text_size(px(28.))
                            .font_weight(gpui::FontWeight::BOLD)
                            .child("Workbook 3 を起動できません"),
                    )
                    .child(div().mt_4().text_size(px(16.)).child(message.to_owned()))
                    .child(
                        div()
                            .mt_4()
                            .text_color(rgb(MUTED))
                            .child("データ保存先の権限を確認してください。"),
                    ),
            )
            .into_any_element()
    }

    fn render_shell(&self, cx: &mut Context<Self>) -> AnyElement {
        let nav = div()
            .flex()
            .items_center()
            .h(px(64.))
            .px_8()
            .border_b_1()
            .border_color(rgb(BORDER))
            .bg(rgb(BG))
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(gpui::FontWeight::BOLD)
                    .child("Workbook 3"),
            )
            .child(div().w(px(40.)).flex_none())
            .child(self.nav_button(
                "nav-home",
                "Home",
                self.screen == Screen::Home,
                cx,
                |this, window, cx| this.go_home(window, cx),
            ))
            .child(self.nav_button(
                "nav-library",
                "Library",
                matches!(self.screen, Screen::Library | Screen::Workbook(_)),
                cx,
                |this, window, cx| this.go_library(window, cx),
            ))
            .child(self.nav_button(
                "nav-plans",
                "Plans",
                self.screen == Screen::Plans,
                cx,
                |this, window, cx| this.go_plans(window, cx),
            ))
            .child(div().flex_1())
            .child(muted("オフライン・ローカル"));

        let body = match &self.screen {
            Screen::Home => self.render_home(cx),
            Screen::Library => self.render_library(cx),
            Screen::Plans => self.render_plans(),
            Screen::Workbook(id) => self.render_workbook(id, cx),
            Screen::Study => self.render_study(cx),
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .child(nav)
            .child(
                div()
                    .id("app-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(body),
            )
            .into_any_element()
    }

    fn nav_button(
        &self,
        id: &'static str,
        label: &'static str,
        active: bool,
        cx: &mut Context<Self>,
        handler: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> AnyElement {
        div()
            .id(id)
            .h_full()
            .px_4()
            .flex()
            .items_center()
            .text_size(px(15.))
            .text_color(if active { rgb(ACCENT) } else { rgb(MUTED) })
            .when(active, |style| style.border_b_2().border_color(rgb(ACCENT)))
            .cursor_pointer()
            .hover(|style| style.text_color(rgb(INK)))
            .on_click(cx.listener(move |this, _, window, cx| handler(this, window, cx)))
            .child(label)
            .into_any_element()
    }

    fn notice_banner(&self) -> Option<AnyElement> {
        self.notice.as_ref().map(|notice| {
            div()
                .my_4()
                .w_full()
                .px_4()
                .py_3()
                .bg(rgb(ACCENT_SOFT))
                .border_l_4()
                .border_color(rgb(ACCENT))
                .text_size(px(14.))
                .child(notice.clone())
                .into_any_element()
        })
    }

    fn render_home(&self, cx: &mut Context<Self>) -> AnyElement {
        let dashboard = self.dashboard.as_ref();
        let mut content = div().py_10().child(
            div().flex().items_end().justify_between().child(
                div()
                    .child(caption("TODAY"))
                    .child(
                        div()
                            .mt_1()
                            .text_size(px(34.))
                            .font_weight(gpui::FontWeight::BOLD)
                            .child("今日の学習"),
                    )
                    .child(
                        div()
                            .mt_2()
                            .text_size(px(15.))
                            .text_color(rgb(MUTED))
                            .child(Local::now().format("%Y年%m月%d日").to_string()),
                    ),
            ),
        );
        if let Some(notice) = self.notice_banner() {
            content = content.child(notice);
        }

        if let Some(session) = &self.active_session {
            content = content.child(
                div()
                    .mt_6()
                    .p_5()
                    .border_1()
                    .border_color(rgb(ACCENT))
                    .bg(rgb(ACCENT_SOFT))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .child(caption("進行中"))
                            .child(
                                div()
                                    .mt_1()
                                    .text_size(px(19.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(format!(
                                        "{}・{} / {}問",
                                        session.workbook_title,
                                        session.answered_count(),
                                        session.items.len()
                                    )),
                            )
                            .child(
                                div()
                                    .mt_1()
                                    .text_size(px(13.))
                                    .text_color(rgb(MUTED))
                                    .child("回答は自動保存されています。"),
                            ),
                    )
                    .child(primary_button("resume-session", "再開する").on_click(
                        cx.listener(|this, _, window, cx| this.resume_session(window, cx)),
                    )),
            );
        }

        let stats = div()
            .mt_6()
            .flex()
            .gap_4()
            .child(self.stat(
                "復習待ち",
                dashboard.map(|d| d.due_recall).unwrap_or(0).to_string(),
                "今日が期限のRecall",
            ))
            .child(
                self.stat(
                    "問題数",
                    dashboard
                        .map(|d| d.total_questions)
                        .unwrap_or(0)
                        .to_string(),
                    "保存済みの教材",
                ),
            )
            .child(
                self.stat(
                    "Skill",
                    dashboard
                        .map(|d| format!("{} / {}", d.mastered_skills, d.skill_count))
                        .unwrap_or_else(|| "0 / 0".to_owned()),
                    "習得度60%以上",
                ),
            );
        content = content.child(stats);

        content =
            content.child(
                div()
                    .mt_8()
                    .child(
                        div()
                            .text_size(px(21.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("時間を決めて始める"),
                    )
                    .child(
                        div()
                            .mt_1()
                            .text_size(px(14.))
                            .text_color(rgb(MUTED))
                            .child("復習・弱点Skill・未学習を1つのPlannerで選びます。"),
                    )
                    .child(div().mt_4().flex().gap_3().children(
                        [3u32, 5, 10, 20].into_iter().map(|minutes| {
                            secondary_button(format!("start-all-{minutes}"), format!("{minutes}分"))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.start_session(None, minutes, window, cx)
                                }))
                                .into_any_element()
                        }),
                    )),
            );

        let mut workbook_rows = div().mt_3().flex().flex_col();
        for workbook in &self.workbooks {
            let id = workbook.id.clone();
            workbook_rows = workbook_rows.child(
                div()
                    .py_4()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .child(
                                div()
                                    .text_size(px(17.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(workbook.title.clone()),
                            )
                            .child(
                                div()
                                    .mt_1()
                                    .text_size(px(13.))
                                    .text_color(rgb(MUTED))
                                    .child(format!(
                                        "{}問・Card {}枚・復習待ち {}",
                                        workbook.question_count,
                                        workbook.card_count,
                                        workbook.due_count
                                    )),
                            ),
                    )
                    .child(
                        secondary_button(format!("open-{}", workbook.id), "開く").on_click(
                            cx.listener(move |this, _, window, cx| {
                                this.open_workbook(id.clone(), window, cx)
                            }),
                        ),
                    ),
            );
        }
        content =
            content
                .child(
                    div()
                        .mt_8()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(21.))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child("Workbook"),
                        )
                        .child(quiet_button("home-library", "Libraryで見る").on_click(
                            cx.listener(|this, _, window, cx| this.go_library(window, cx)),
                        )),
                )
                .child(workbook_rows);

        content = content.child(
            div()
                .mt_10()
                .pb_10()
                .text_size(px(12.))
                .text_color(rgb(MUTED))
                .child(
                    "データはこのアプリ専用のSQLiteに保存されます。AI接続がなくても学習できます。",
                ),
        );

        div()
            .w_full()
            .px_8()
            .child(page().child(content))
            .into_any_element()
    }

    fn stat(&self, label: &str, value: String, note: &str) -> AnyElement {
        div()
            .flex_1()
            .p_5()
            .bg(rgb(PANEL))
            .border_1()
            .border_color(rgb(BORDER))
            .child(caption(label.to_owned()))
            .child(
                div()
                    .mt_2()
                    .text_size(px(30.))
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(value),
            )
            .child(
                div()
                    .mt_1()
                    .text_size(px(12.))
                    .text_color(rgb(MUTED))
                    .child(note.to_owned()),
            )
            .into_any_element()
    }

    fn render_library(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut content = div().py_10().child(
            div()
                .child(caption("LIBRARY"))
                .child(
                    div()
                        .mt_1()
                        .text_size(px(34.))
                        .font_weight(gpui::FontWeight::BOLD)
                        .child("教材"),
                )
                .child(
                    div()
                        .mt_2()
                        .text_size(px(15.))
                        .text_color(rgb(MUTED))
                        .child("Workbookを開き、Cardで確認してから問題を解きます。"),
                ),
        );
        if let Some(notice) = self.notice_banner() {
            content = content.child(notice);
        }
        let mut rows = div().mt_6().flex().flex_col();
        for workbook in &self.workbooks {
            let id = workbook.id.clone();
            rows = rows.child(
                div()
                    .p_5()
                    .mb_3()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .child(
                                div()
                                    .text_size(px(19.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(workbook.title.clone()),
                            )
                            .child(
                                div()
                                    .mt_1()
                                    .text_size(px(14.))
                                    .text_color(rgb(MUTED))
                                    .child(workbook.description.clone()),
                            )
                            .child(
                                div()
                                    .mt_3()
                                    .flex()
                                    .gap_2()
                                    .child(chip(format!("{}問", workbook.question_count)))
                                    .child(chip(format!("Card {}", workbook.card_count)))
                                    .child(chip(format!("復習待ち {}", workbook.due_count))),
                            ),
                    )
                    .child(
                        secondary_button(format!("library-open-{}", workbook.id), "開く").on_click(
                            cx.listener(move |this, _, window, cx| {
                                this.open_workbook(id.clone(), window, cx)
                            }),
                        ),
                    ),
            );
        }
        content = content.child(rows);
        div()
            .w_full()
            .px_8()
            .child(page().child(content.pb_10()))
            .into_any_element()
    }

    fn render_plans(&self) -> AnyElement {
        let mut content = div().py_10().child(
            div()
                .child(caption("PLANS"))
                .child(div().mt_1().text_size(px(34.)).font_weight(gpui::FontWeight::BOLD).child("学習計画"))
                .child(div().mt_2().max_w(px(680.)).text_size(px(15.)).text_color(rgb(MUTED)).child("Examは特別な問題コピーを持たず、対象範囲・期限・1日の時間予算を持つPlanとして実装します。")),
        );
        content = content.child(
            div()
                .mt_8()
                .p_6()
                .border_1()
                .border_color(rgb(BORDER))
                .bg(rgb(PANEL))
                .child(div().text_size(px(19.)).font_weight(gpui::FontWeight::SEMIBOLD).child("Phase 3 で実装"))
                .child(div().mt_2().text_size(px(15.)).line_height(px(26.)).child("期限までの日数、試験範囲の自動反映、時間不足の警告、Skill習得追跡の改善をこの画面に追加します。"))
                .child(div().mt_2().text_size(px(15.)).line_height(px(26.)).child("FSRSのdue日時をExamの都合で書き換えません。Exam中の回答も通常の履歴に記録します。")),
        );
        if self.plans.is_empty() {
            content = content.child(
                div()
                    .mt_6()
                    .text_size(px(14.))
                    .text_color(rgb(MUTED))
                    .child("保存済みのPlanはまだありません。"),
            );
        } else {
            for plan in &self.plans {
                content = content.child(
                    div()
                        .mt_3()
                        .p_4()
                        .border_1()
                        .border_color(rgb(BORDER))
                        .child(plan.name.clone())
                        .child(muted(format!("1日 {} 分", plan.daily_minutes))),
                );
            }
        }
        content = content.child(
            div()
                .mt_8()
                .text_size(px(13.))
                .text_color(rgb(MUTED))
                .child("グループ・タグ範囲・目標はDB schemaに保存済みで、Phase 3で画面とPlannerに接続します。"),
        );
        div()
            .w_full()
            .px_8()
            .child(page().child(content.pb_10()))
            .into_any_element()
    }

    fn render_workbook(&self, id: &WorkbookId, cx: &mut Context<Self>) -> AnyElement {
        let Some(detail) = &self.workbook_detail else {
            return div()
                .p_10()
                .child("Workbookが見つかりません。")
                .into_any_element();
        };
        let mut content = div().py_8().child(
            quiet_button("back-library", "← Library")
                .on_click(cx.listener(|this, _, window, cx| this.go_library(window, cx))),
        );
        if let Some(notice) = self.notice_banner() {
            content = content.child(notice);
        }
        content = content.child(
            div()
                .mt_2()
                .child(
                    div()
                        .text_size(px(32.))
                        .font_weight(gpui::FontWeight::BOLD)
                        .child(detail.workbook.title.clone()),
                )
                .child(
                    div()
                        .mt_2()
                        .max_w(px(760.))
                        .text_size(px(15.))
                        .text_color(rgb(MUTED))
                        .child(detail.workbook.description.clone()),
                )
                .child(
                    div()
                        .mt_3()
                        .flex()
                        .gap_2()
                        .children(detail.tags.iter().map(|tag| chip(tag.name.clone()))),
                ),
        );

        let workbook_id = id.clone();
        content =
            content.child(
                div()
                    .mt_6()
                    .p_5()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(PANEL))
                    .child(
                        div()
                            .text_size(px(17.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("このWorkbookを学習する"),
                    )
                    .child(
                        div()
                            .mt_1()
                            .text_size(px(13.))
                            .text_color(rgb(MUTED))
                            .child(
                                "初回はLearning Cardから始まります。復習では問題から始まります。",
                            ),
                    )
                    .child(div().mt_3().flex().gap_3().children(
                        [3u32, 5, 10, 20].into_iter().map(|minutes| {
                            let workbook_id = workbook_id.clone();
                            secondary_button(
                                format!("start-workbook-{minutes}"),
                                format!("{minutes}分"),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.start_session(Some(workbook_id.clone()), minutes, window, cx)
                            }))
                            .into_any_element()
                        }),
                    )),
            );

        let mut cards = div().mt_8().flex().flex_col();
        for card in &detail.cards {
            cards = cards.child(
                div()
                    .py_3()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(caption("LEARNING CARD"))
                    .child(div().mt_1().text_size(px(16.)).child(card.title.clone())),
            );
        }
        content = content.child(
            div()
                .mt_8()
                .child(
                    div()
                        .text_size(px(20.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(format!("Learning Cards（{}）", detail.cards.len())),
                )
                .child(cards),
        );

        let mut questions = div().mt_3().flex().flex_col();
        for (index, question) in detail.questions.iter().enumerate() {
            questions = questions.child(self.question_row(index, question));
        }
        content = content.child(
            div()
                .mt_8()
                .pb_10()
                .child(
                    div()
                        .text_size(px(20.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(format!("Questions（{}）", detail.questions.len())),
                )
                .child(questions),
        );

        div()
            .w_full()
            .px_8()
            .child(page().child(content))
            .into_any_element()
    }

    fn question_row(&self, index: usize, question: &QuestionSummary) -> AnyElement {
        div()
            .py_4()
            .border_b_1()
            .border_color(rgb(BORDER))
            .flex()
            .items_start()
            .gap_4()
            .child(
                div()
                    .w(px(30.))
                    .text_size(px(13.))
                    .text_color(rgb(MUTED))
                    .child(format!("{:02}", index + 1)),
            )
            .child(
                div()
                    .flex_1()
                    .child(div().text_size(px(16.)).child(question.preview.clone()))
                    .child(
                        div()
                            .mt_2()
                            .flex()
                            .gap_2()
                            .child(chip(question.form.label()))
                            .child(chip(question.kind.label()))
                            .child(chip(question.difficulty.label()))
                            .child(chip(format!("約{}秒", question.estimated_seconds))),
                    ),
            )
            .into_any_element()
    }

    fn render_study(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(study) = &self.study else {
            return div()
                .p_10()
                .child("セッションがありません。")
                .into_any_element();
        };
        let top = div()
            .flex()
            .items_center()
            .h(px(60.))
            .px_8()
            .border_b_1()
            .border_color(rgb(BORDER))
            .child(
                quiet_button("interrupt", "終了")
                    .on_click(cx.listener(|this, _, window, cx| this.interrupt(window, cx))),
            )
            .child(div().w(px(24.)).flex_none())
            .child(
                div()
                    .text_size(px(16.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(study.session.workbook_title.clone()),
            )
            .child(div().flex_1())
            .child(muted(format!(
                "予算 {}分",
                study.session.session.budget_minutes
            )))
            .child(div().w(px(20.)).flex_none())
            .child(caption(study.session.progress_label()));

        let mut content = div().py_8();
        if let Some(notice) = self.notice_banner() {
            content = content.child(notice);
        }
        let body = match &study.phase {
            StudyPhase::Card { card, .. } => self.render_card(card, cx),
            StudyPhase::Question { question } => self.render_question(study, question, cx),
            StudyPhase::Feedback { question, feedback } => {
                self.render_feedback(study, question, feedback, cx)
            }
            StudyPhase::Finished => self.render_finished(study, cx),
        };
        content = content.child(body);
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(top)
            .child(
                div()
                    .id("study-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .w_full()
                    .px_8()
                    .child(div().max_w(px(860.)).mx_auto().child(content)),
            )
            .into_any_element()
    }

    fn render_card(&self, card: &LearningCard, cx: &mut Context<Self>) -> AnyElement {
        div()
            .max_w(px(760.))
            .child(caption("LEARNING CARD"))
            .child(
                div()
                    .mt_2()
                    .text_size(px(30.))
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(card.title.clone()),
            )
            .children(render_blocks(&card.blocks))
            .child(
                div().mt_8().child(
                    primary_button("card-next", "問題へ")
                        .on_click(cx.listener(|this, _, window, cx| this.card_next(window, cx))),
                ),
            )
            .into_any_element()
    }

    fn render_question(
        &self,
        study: &StudyState,
        question: &Question,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut container = div();
        container = container.child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(caption("QUESTION"))
                .child(chip(question.kind.label()))
                .child(chip(question.difficulty.label()))
                .child(chip(question.form.label())),
        );
        container = container.child(
            div()
                .mt_4()
                .text_size(px(23.))
                .line_height(px(36.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .children(render_blocks(&question.prompt)),
        );

        if let Some(message) = &study.near_miss {
            container = container.child(
                div()
                    .mt_4()
                    .px_4()
                    .py_3()
                    .bg(rgb(WARNING_SOFT))
                    .border_l_4()
                    .border_color(rgb(0xb45309))
                    .text_size(px(14.))
                    .child(message.clone()),
            );
        }

        let answer_area = match question.form {
            AnswerForm::SingleChoice => {
                let mut choices = div().mt_6().flex().flex_col().gap_3();
                for (index, choice) in question.answer.choices().iter().enumerate() {
                    let selected = study.selected_choice.as_deref() == Some(choice.id.as_str());
                    let id = choice.id.clone();
                    choices = choices.child(
                        div()
                            .id(gpui::SharedString::from(format!("choice-{}", choice.id)))
                            .w_full()
                            .px_4()
                            .py_3()
                            .border_1()
                            .border_color(if selected { rgb(ACCENT) } else { rgb(0xc7ccd4) })
                            .bg(if selected { rgb(ACCENT_SOFT) } else { rgb(BG) })
                            .cursor_pointer()
                            .hover(|style| style.bg(rgb(PANEL)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(study) = this.study.as_mut() {
                                    study.selected_choice = Some(id.clone());
                                }
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .w(px(24.))
                                            .text_size(px(13.))
                                            .text_color(rgb(MUTED))
                                            .child(format!("{}", index + 1)),
                                    )
                                    .child(div().text_size(px(17.)).child(choice.text.clone())),
                            ),
                    );
                }
                choices.into_any_element()
            }
            AnswerForm::ShortAnswer | AnswerForm::Numeric => {
                let hint = if question.form == AnswerForm::Numeric {
                    "数値と単位（例：2.7 g/cm3）"
                } else {
                    "答えを入力"
                };
                let mut field = div()
                    .mt_6()
                    .max_w(px(620.))
                    .child(caption(if question.form == AnswerForm::Numeric {
                        "NUMERIC"
                    } else {
                        "SHORT ANSWER"
                    }))
                    .child(div().mt_2().child(self.input.clone()));
                if question.form == AnswerForm::Numeric {
                    field = field.child(
                        div()
                            .mt_2()
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .child(hint),
                    );
                }
                field.into_any_element()
            }
            AnswerForm::Flashcard => {
                if study.flashcard_revealed {
                    let back = match &question.answer {
                        AnswerSpec::Flashcard { back, .. } => back.clone(),
                        _ => String::new(),
                    };
                    div()
                        .mt_6()
                        .p_5()
                        .bg(rgb(PANEL))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .child(caption("BACK"))
                        .child(
                            div()
                                .mt_2()
                                .text_size(px(20.))
                                .line_height(px(30.))
                                .child(back),
                        )
                        .child(
                            div()
                                .mt_6()
                                .flex()
                                .gap_3()
                                .child(secondary_button("flash-again", "もう一度").on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.submit_flashcard(SelfAssessment::Again, window, cx)
                                    }),
                                ))
                                .child(primary_button("flash-good", "思い出せた").on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.submit_flashcard(SelfAssessment::Good, window, cx)
                                    }),
                                ))
                                .child(secondary_button("flash-easy", "簡単").on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.submit_flashcard(SelfAssessment::Easy, window, cx)
                                    }),
                                )),
                        )
                        .into_any_element()
                } else {
                    div()
                        .mt_6()
                        .child(
                            primary_button("flash-reveal", "答えを見る")
                                .on_click(cx.listener(|this, _, _, cx| this.reveal_flashcard(cx))),
                        )
                        .child(
                            div()
                                .mt_2()
                                .text_size(px(12.))
                                .text_color(rgb(MUTED))
                                .child("Spaceキーでも表示できます。"),
                        )
                        .into_any_element()
                }
            }
            _ => div()
                .mt_6()
                .text_size(px(14.))
                .text_color(rgb(MUTED))
                .child("この回答形式はPhase 2で実装します。")
                .into_any_element(),
        };
        container = container.child(answer_area);

        if question.form != AnswerForm::Flashcard
            && let Some(hint) = &question.hint
        {
            container = container.child(
                div()
                    .mt_6()
                    .child(
                        quiet_button(
                            "toggle-hint",
                            if study.hint_visible {
                                "ヒントを閉じる"
                            } else {
                                "ヒントを見る（F1）"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_hint(cx))),
                    )
                    .when(study.hint_visible, |this| {
                        this.child(
                            div()
                                .mt_2()
                                .p_4()
                                .bg(rgb(PANEL))
                                .border_l_4()
                                .border_color(rgb(MUTED))
                                .text_size(px(15.))
                                .child(hint.clone()),
                        )
                    }),
            );
        }

        if question.form != AnswerForm::Flashcard {
            container = container.child(
                div()
                    .mt_8()
                    .child(
                        primary_button("submit-answer", "回答する")
                            .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx))),
                    )
                    .child(
                        div()
                            .mt_2()
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .child("Enterで回答。Escで中断（回答は保存されます）。"),
                    ),
            );
        }

        container.into_any_element()
    }

    fn render_feedback(
        &self,
        _study: &StudyState,
        question: &Question,
        feedback: &FeedbackState,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let correct = feedback.result.outcome == crate::domain::Outcome::Correct;
        let partial = feedback.result.outcome == crate::domain::Outcome::Partial;
        let label = if correct {
            "正解"
        } else if partial {
            "一部正解"
        } else if feedback.result.outcome == crate::domain::Outcome::Ungradable {
            "自動判定不可"
        } else {
            "不正解"
        };
        let mut container = div();
        container = container.child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(caption("FEEDBACK"))
                .child(chip(question.kind.label())),
        );
        container = container.child(
            div()
                .mt_4()
                .text_size(px(23.))
                .line_height(px(36.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .children(render_blocks(&question.prompt)),
        );
        container = container.child(
            div()
                .mt_6()
                .p_5()
                .bg(if correct {
                    rgb(ACCENT_SOFT)
                } else {
                    rgb(PANEL)
                })
                .border_l_4()
                .border_color(if correct { rgb(ACCENT) } else { rgb(MUTED) })
                .child(
                    div()
                        .text_size(px(22.))
                        .font_weight(gpui::FontWeight::BOLD)
                        .child(label),
                )
                .child(
                    div()
                        .mt_2()
                        .text_size(px(15.))
                        .line_height(px(24.))
                        .child(feedback.result.feedback.clone()),
                )
                .when_some(feedback.result.expected.clone(), |this, expected| {
                    this.child(
                        div()
                            .mt_2()
                            .text_size(px(15.))
                            .child(format!("正答: {expected}")),
                    )
                })
                .when_some(feedback.note.clone(), |this, note| {
                    this.child(
                        div()
                            .mt_2()
                            .text_size(px(13.))
                            .text_color(rgb(MUTED))
                            .child(note),
                    )
                }),
        );

        let mut meta = div().mt_3().flex().gap_2();
        if feedback.first_exposure {
            meta = meta.child(chip("初見"));
        }
        if feedback.result.assisted {
            meta = meta.child(chip("ヒント・再入力あり（自力正解ではない）"));
        }
        if let Some(due) = feedback.due_at {
            meta = meta.child(chip(format!("次回: {}", format_due(due))));
        }
        if let Some(mastery) = feedback.skill_mastery {
            meta = meta.child(chip(format!(
                "Skill習得度: {}%",
                (mastery * 100.0).round() as u32
            )));
        }
        container = container.child(meta);

        container = container.child(
            div()
                .mt_8()
                .child(caption("EXPLANATION"))
                .children(render_blocks(&question.explanation)),
        );

        container = container.child(
            div()
                .mt_8()
                .pb_10()
                .child(
                    primary_button("next-item", "次へ")
                        .on_click(cx.listener(|this, _, window, cx| this.next(window, cx))),
                )
                .child(
                    div()
                        .mt_2()
                        .text_size(px(12.))
                        .text_color(rgb(MUTED))
                        .child("Enterで次へ。"),
                ),
        );
        container.into_any_element()
    }

    fn render_finished(&self, study: &StudyState, cx: &mut Context<Self>) -> AnyElement {
        div()
            .max_w(px(680.))
            .child(caption("SESSION COMPLETE"))
            .child(div().mt_2().text_size(px(32.)).font_weight(gpui::FontWeight::BOLD).child("このセッションは完了しました"))
            .child(div().mt_3().text_size(px(16.)).text_color(rgb(MUTED)).child(format!("{}問中{}問に回答しました。未回答の問題は正解・不正解のどちらにも数えていません。", study.session.items.len(), study.session.answered_count())))
            .child(
                div()
                    .mt_8()
                    .flex()
                    .gap_3()
                    .child(primary_button("finish-home", "Homeへ").on_click(cx.listener(|this, _, window, cx| this.go_home(window, cx))))
                    .child(secondary_button("finish-library", "Libraryへ").on_click(cx.listener(|this, _, window, cx| this.go_library(window, cx)))),
            )
            .into_any_element()
    }
}

fn format_due(due: DateTime<Utc>) -> String {
    let local = due.with_timezone(&Local);
    let now = Local::now();
    if local.date_naive() == now.date_naive() {
        local.format("%H:%M").to_string()
    } else {
        local.format("%m/%d %H:%M").to_string()
    }
}

pub fn run() {
    Application::new().run(|cx: &mut App| {
        cx.bind_keys(app_key_bindings());
        let bounds = Bounds::centered(None, size(px(1120.), px(780.)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| {
                    let repo = open_repository();
                    cx.new(|cx| AppView::new(cx, repo))
                },
            )
            .expect("open Workbook 3 window");
        window
            .update(cx, |view, window, cx| {
                window.focus(&view.root_focus);
                cx.activate(true);
            })
            .ok();
    });
}

fn open_repository() -> Result<Repository> {
    let path = data_dir().join("workbook3.sqlite3");
    eprintln!("Workbook 3 data: {}", path.display());
    Repository::open(path)
}

pub fn data_dir() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("WORKBOOK3_DATA_DIR") {
        return std::path::PathBuf::from(path);
    }
    dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("Workbook3")
}
