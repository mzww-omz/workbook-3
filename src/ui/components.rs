use gpui::{Div, SharedString, Stateful, div, prelude::*, px, rgb};

pub const BG: u32 = 0xffffff;
pub const PANEL: u32 = 0xf6f7f8;
pub const INK: u32 = 0x111827;
pub const MUTED: u32 = 0x6b7280;
pub const BORDER: u32 = 0xdfe3e8;
pub const ACCENT: u32 = 0x1d4ed8;
pub const ACCENT_SOFT: u32 = 0xeff4ff;
pub const WARNING_SOFT: u32 = 0xfffbeb;

pub fn app_shell() -> Div {
    div()
        .flex()
        .flex_col()
        .size_full()
        .bg(rgb(BG))
        .text_color(rgb(INK))
}

pub fn page() -> Div {
    div().w_full().max_w(px(1040.)).mx_auto().px_8()
}

pub fn panel() -> Div {
    div()
        .w_full()
        .border_1()
        .border_color(rgb(BORDER))
        .bg(rgb(BG))
}

pub fn muted(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(14.))
        .text_color(rgb(MUTED))
        .child(text.into())
}

pub fn caption(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(12.))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(rgb(MUTED))
        .child(text.into())
}

pub fn primary_button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
) -> Stateful<Div> {
    div()
        .id(id.into())
        .px_4()
        .py_2()
        .bg(rgb(ACCENT))
        .text_color(rgb(0xffffff))
        .text_size(px(15.))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .cursor_pointer()
        .hover(|style| style.bg(rgb(0x1e40af)))
        .active(|style| style.opacity(0.85))
        .child(label.into())
}

pub fn secondary_button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
) -> Stateful<Div> {
    div()
        .id(id.into())
        .px_4()
        .py_2()
        .bg(rgb(BG))
        .border_1()
        .border_color(rgb(0xc7ccd4))
        .text_color(rgb(INK))
        .text_size(px(15.))
        .cursor_pointer()
        .hover(|style| style.bg(rgb(0xf3f4f6)))
        .active(|style| style.opacity(0.85))
        .child(label.into())
}

pub fn quiet_button(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id.into())
        .px_2()
        .py_1()
        .text_color(rgb(MUTED))
        .text_size(px(14.))
        .cursor_pointer()
        .hover(|style| style.text_color(rgb(INK)))
        .child(label.into())
}

pub fn chip(text: impl Into<SharedString>) -> Div {
    div()
        .px_2()
        .py_1()
        .bg(rgb(PANEL))
        .border_1()
        .border_color(rgb(BORDER))
        .text_size(px(12.))
        .text_color(rgb(MUTED))
        .child(text.into())
}

pub fn divider() -> Div {
    div().h(px(1.)).w_full().bg(rgb(BORDER))
}

pub fn render_blocks(blocks: &[crate::content::ContentBlock]) -> Vec<gpui::AnyElement> {
    use crate::content::ContentBlock;
    blocks
        .iter()
        .map(|block| match block {
            ContentBlock::Heading { level, text } => div()
                .mt_4()
                .mb_1()
                .text_size(if *level <= 2 { px(24.) } else { px(19.) })
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(text.clone())
                .into_any_element(),
            ContentBlock::Text { text } => div()
                .my_2()
                .text_size(px(17.))
                .line_height(px(29.))
                .child(text.clone())
                .into_any_element(),
            ContentBlock::Markdown { markdown } => div()
                .my_2()
                .text_size(px(17.))
                .line_height(px(29.))
                .child(markdown.clone())
                .into_any_element(),
            ContentBlock::Math { latex, .. } => div()
                .my_2()
                .px_3()
                .py_2()
                .bg(rgb(PANEL))
                .font_family("Menlo")
                .text_size(px(16.))
                .child(latex.clone())
                .into_any_element(),
            ContentBlock::Table {
                caption,
                headers,
                rows,
            } => {
                let mut container = div().my_3().w_full();
                if let Some(caption) = caption {
                    container = container.child(muted(caption.clone()));
                }
                container = container.child(
                    div()
                        .text_size(px(15.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(headers.join("  |  ")),
                );
                for row in rows {
                    container = container.child(
                        div()
                            .py_1()
                            .border_b_1()
                            .border_color(rgb(BORDER))
                            .text_size(px(15.))
                            .child(row.join("  |  ")),
                    );
                }
                container.into_any_element()
            }
            other => div()
                .my_2()
                .px_3()
                .py_2()
                .bg(rgb(PANEL))
                .text_size(px(14.))
                .text_color(rgb(MUTED))
                .child(other.plain_text())
                .into_any_element(),
        })
        .collect()
}
