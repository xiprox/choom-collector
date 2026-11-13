//! Small pieces every screen draws with: text, buttons, checkboxes, and how numbers and times read.

use chrono::{DateTime, Local};
use gpui_kit::assets::IconName;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::theme::*;

pub fn icon(name: IconName, hex: u32) -> Icon {
    Icon::new(name).with_size(ICON).text_color(color(hex))
}

/// A section's or column's label: 12px, muted.
pub fn label(text: impl Into<SharedString>) -> Div {
    div().text_size(SMALL_SIZE).font_weight(FontWeight::MEDIUM).text_color(color(MUTED)).child(text.into())
}

/// Codes, sizes, and paths.
pub fn mono(cx: &App, text: impl Into<SharedString>) -> Div {
    div().font_family(cx.theme().mono_font_family.clone()).text_size(SMALL_SIZE).child(text.into())
}

/// A button with no box until it's hovered, as most buttons are.
pub fn plain_button(id: impl Into<ElementId>, text: impl Into<SharedString>, leading: Option<IconName>) -> Stateful<Div> {
    h_flex()
        .id(id.into())
        // Blocks the hit to what's behind, so in the title bar a real click reaches the button
        // instead of Windows taking it as a grab of the bar to move the window.
        .occlude()
        .h(px(24.))
        .px(px(8.))
        .gap(px(8.))
        .rounded(RADIUS)
        .text_color(color(TEXT))
        .cursor_pointer()
        .hover(|s| s.bg(color(RAISED)))
        .active(|s| s.bg(color(CONTROL)))
        .when_some(leading, |b, name| b.child(icon(name, TEXT)))
        .child(text.into())
}

/// A square button with only an icon, which says what it does in its tooltip.
pub fn icon_button(id: impl Into<ElementId>, name: IconName, hex: u32) -> Stateful<Div> {
    div()
        .id(id.into())
        // Blocks the hit to what's behind, so in the title bar a real click reaches the button
        // instead of Windows taking it as a grab of the bar to move the window.
        .occlude()
        .flex()
        .items_center()
        .justify_center()
        .size(px(24.))
        .rounded(RADIUS)
        .cursor_pointer()
        .hover(|s| s.bg(color(RAISED)))
        .active(|s| s.bg(color(CONTROL)))
        .child(icon(name, hex))
}

/// The red stop square that ends the session, as an IDE's stop button ends a run.
pub fn stop_button(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id.into())
        // Blocks the hit to what's behind, so in the title bar a real click reaches the button
        // instead of Windows taking it as a grab of the bar to move the window.
        .occlude()
        .flex()
        .items_center()
        .justify_center()
        .size(px(20.))
        .rounded(RADIUS)
        .cursor_pointer()
        .hover(|s| s.bg(color(RAISED)))
        .active(|s| s.bg(color(CONTROL)))
        // The square is 11px, the code's height, drawn in a 16px box centered in the 20px button.
        .child(svg().path("brand/stop.svg").size(px(16.)).text_color(color(TROUBLE)))
}

/// What a button is for, which only its colors show: every button has the same shape.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// What's yours or waits on you, such as Follow, Save, or Join: the accent's yellow.
    Primary,
    /// Any other choice, such as Cancel, Keep my copy, or Change: dark gray.
    Secondary,
    /// A choice that removes something, such as Delete it: dark gray with the trouble color's text.
    Danger,
}

/// The one button shape (the user, 2026-11-13): 24px tall, 12px at the sides, medium weight, 4px
/// corners, and no border, so buttons side by side match whatever their kind.
pub fn button(id: impl Into<ElementId>, text: impl Into<SharedString>, kind: Kind) -> Stateful<Div> {
    let (bg, hover, fg) = match kind {
        Kind::Primary => (ACCENT, 0xe8d56a, ON_ACCENT),
        Kind::Secondary => (CONTROL, 0x3c3c43, STRONG),
        Kind::Danger => (CONTROL, 0x3c3c43, TROUBLE),
    };
    h_flex()
        .id(id.into())
        // Blocks the hit to what's behind, so in the title bar a real click reaches the button
        // instead of Windows taking it as a grab of the bar to move the window.
        .occlude()
        .flex_shrink_0()
        .h(px(24.))
        .px(px(12.))
        .rounded(RADIUS)
        .bg(color(bg))
        .text_color(color(fg))
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .hover(move |s| s.bg(color(hover)))
        .active(|s| s.opacity(0.85))
        .child(text.into())
}

/// The primary button made small for a title bar, for one thing waiting on the player, such as an
/// update: 20px tall, so it keeps 6px clear of the bar's edges.
pub fn accent_pill(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Stateful<Div> {
    button(id, text, Kind::Primary).h(px(20.)).px(px(8.)).text_size(SMALL_SIZE)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Check {
    Off,
    On,
    /// On, with something waiting for the player's yes.
    Partly,
}

pub fn checkbox(id: impl Into<ElementId>, check: Check, focused: bool) -> Stateful<Div> {
    let mark = match check {
        Check::Off => None,
        Check::On => Some(IconName::Check),
        Check::Partly => Some(IconName::Minus),
    };
    div()
        .id(id.into())
        // Blocks the hit to what's behind, so in the title bar a real click reaches the button
        // instead of Windows taking it as a grab of the bar to move the window.
        .occlude()
        .flex()
        .items_center()
        .justify_center()
        .size(px(16.))
        .rounded(RADIUS)
        .cursor_pointer()
        .map(|b| match mark {
            Some(_) => b.bg(color(ACCENT)),
            None => b.border_1().border_color(color(if focused { ACCENT } else { CONTROL })),
        })
        .when_some(mark, |b, name| b.child(Icon::new(name).with_size(px(12.)).text_color(color(ON_ACCENT))))
}

/// A follower's tile in a shared mod's status: their initial, colored by their state.
pub fn tile(cx: &App, name: &str, background: u32, foreground: u32) -> Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .w(px(20.))
        .h(px(16.))
        .rounded(px(2.))
        .bg(color(background))
        .text_color(color(foreground))
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(SMALL_SIZE)
        .font_weight(FontWeight::SEMIBOLD)
        .child(name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default())
}

/// A tile for files on their way: it fills from the left as they arrive, cyan for what's here and
/// half-strength cyan for what's still to come, with the initial in dark over both.
pub fn progress_tile(cx: &App, name: &str, done: u32, total: u32) -> Div {
    let share = if total == 0 { 0.0 } else { (done as f32 / total as f32).clamp(0.0, 1.0) };
    div()
        .relative()
        .overflow_hidden()
        .w(px(20.))
        .h(px(16.))
        .rounded(px(2.))
        .bg(color(PROGRESS_DIM))
        .child(div().absolute().top_0().left_0().h_full().w(px(20. * share)).bg(color(PROGRESS)))
        .child(
            div()
                .absolute()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(color(ON_ACCENT))
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(SMALL_SIZE)
                .font_weight(FontWeight::SEMIBOLD)
                .child(name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default()),
        )
}

/// The mark: two slanted bars, two players side by side.
pub fn mark(size: Pixels) -> Svg {
    svg().path("brand/mark.svg").size(size).text_color(color(ACCENT))
}

/// A code as players read and type it: `K7RM2Q` reads `K7R-M2Q`.
pub fn code(raw: &str) -> String {
    if raw.len() == 6 { format!("{}-{}", &raw[..3], &raw[3..]) } else { raw.to_string() }
}

/// Sizes the way players read them: 96 KB, 1.2 MB, 212 MB.
pub fn bytes(n: u64) -> String {
    const KB: f64 = 1000.;
    let n = n as f64;
    if n < KB * KB {
        format!("{:.0} KB", (n / KB).max(1.))
    } else if n < 100. * KB * KB {
        format!("{:.1} MB", n / KB / KB)
    } else if n < KB * KB * KB {
        format!("{:.0} MB", n / KB / KB)
    } else {
        format!("{:.1} GB", n / KB / KB / KB)
    }
}

pub fn hhmm(t: &DateTime<Local>) -> String {
    t.format("%H:%M").to_string()
}
