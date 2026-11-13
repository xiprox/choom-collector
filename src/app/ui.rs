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

/// A button with a border, for the second choice beside a filled one.
pub fn outline_button(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Stateful<Div> {
    h_flex()
        .id(id.into())
        // Blocks the hit to what's behind, so in the title bar a real click reaches the button
        // instead of Windows taking it as a grab of the bar to move the window.
        .occlude()
        .h(px(24.))
        .px(px(12.))
        .rounded(RADIUS)
        .border_1()
        .border_color(color(CONTROL))
        .text_color(color(TEXT))
        .cursor_pointer()
        .hover(|s| s.bg(color(RAISED)))
        .active(|s| s.bg(color(CONTROL)))
        .child(text.into())
}

/// The one filled button a screen may have.
pub fn accent_button(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Stateful<Div> {
    h_flex()
        .id(id.into())
        // Blocks the hit to what's behind, so in the title bar a real click reaches the button
        // instead of Windows taking it as a grab of the bar to move the window.
        .occlude()
        .h(px(28.))
        .px(px(20.))
        .rounded(RADIUS)
        .bg(color(ACCENT))
        .text_color(color(ON_ACCENT))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .hover(|s| s.opacity(0.9))
        .active(|s| s.opacity(0.8))
        .child(text.into())
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
