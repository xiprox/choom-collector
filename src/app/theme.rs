//! The values the window draws with, from record/areas/interface/decisions.md, which is where to
//! change them first.

use gpui_kit::component::Theme;
use gpui_kit::*;

// Near-black grays with a touch of blue, since exact neutral grays read yellowish beside the accent:
// "clean, neutral, professional black" (the user, 2026-11-13).
pub const BASE: u32 = 0x08080a;
pub const PANEL: u32 = 0x0d0d10;
pub const RAISED: u32 = 0x18181c;
/// A row under the pointer, between the panel and a selected row.
pub const HOVER: u32 = 0x121216;
pub const LINE: u32 = 0x1e1e23;
pub const LINE_SOFT: u32 = 0x141418;
pub const CONTROL: u32 = 0x303036;
pub const TEXT: u32 = 0xe3e3e8;
pub const STRONG: u32 = 0xf4f4f7;
pub const MUTED: u32 = 0x96969f;
/// Placeholders in text boxes, dimmer than muted text, so they read as not filled in.
pub const PLACEHOLDER: u32 = 0x58585f;
/// What's the player's own, or happening now.
pub const ACCENT: u32 = 0xddc94f;
pub const ON_ACCENT: u32 = 0x08080a;
/// Something in progress, such as files on their way: Dual Input's primary cyan, so it can't be
/// mistaken for the accent's yellow (the user, 2026-11-13).
pub const PROGRESS: u32 = 0x74d4ff;
/// The part of a filling tile still to come: the cyan at about half strength, which dark text
/// still reads on.
pub const PROGRESS_DIM: u32 = 0x3f7489;
pub const DONE: u32 = 0x8c8c95;
/// Something's off.
pub const WARNING: u32 = 0xe89b5e;
/// Something broke.
pub const TROUBLE: u32 = 0xea7977;

pub const FONT: &str = "IBM Plex Sans";
pub const MONO_FONT: &str = "IBM Plex Mono";
pub const TEXT_SIZE: Pixels = px(13.);
pub const SMALL_SIZE: Pixels = px(12.);
pub const TITLE_SIZE: Pixels = px(16.);
pub const LINE_HEIGHT: Pixels = px(16.);

pub const TITLE_BAR: Pixels = px(32.);
pub const SECTION: Pixels = px(32.);
pub const STATUS_BAR: Pixels = px(32.);
// Sizes and spacing are multiples of 4px; text sizes aren't on the grid, their line heights are.
/// The space around a row's or bar's content, the same on every side: 16px of content (a line of
/// text, a checkbox, an icon) with 8px around it makes every row and bar 32px (the user,
/// 2026-11-13).
pub const INSET: Pixels = px(8.);
pub const ROW: Pixels = px(32.);
pub const HEADER_ROW: Pixels = px(32.);
pub const PLAYERS_WIDTH: Pixels = px(280.);
pub const RADIUS: Pixels = px(4.);
/// Icons are a glyph size, like text sizes, so they sit off the grid in a 16px slot or a button.
pub const ICON: Pixels = px(14.);

pub fn color(hex: u32) -> Hsla {
    rgb(hex).into()
}

/// Gives gpui-component's own controls, such as the text inputs and the title bar's window
/// buttons, the same colors as everything drawn by hand.
pub fn apply(cx: &mut App) {
    Theme::update(cx, |t| {
        t.font_family = FONT.into();
        t.mono_font_family = MONO_FONT.into();
        t.font_size = TEXT_SIZE;
        t.mono_font_size = SMALL_SIZE;
        t.radius = RADIUS;
        t.radius_lg = RADIUS;
        t.shadow = false;
        t.background = color(BASE);
        t.foreground = color(TEXT);
        t.border = color(LINE);
        t.input = color(LINE);
        t.ring = color(ACCENT);
        t.caret = color(STRONG);
        t.selection = color(0x34343c);
        t.muted = color(RAISED);
        // gpui-component draws a text box's placeholder in its muted foreground.
        t.muted_foreground = color(PLACEHOLDER);
        t.popover = color(RAISED);
        t.popover_foreground = color(TEXT);
        t.primary = color(ACCENT);
        t.primary_foreground = color(ON_ACCENT);
        t.title_bar = color(PANEL);
        t.title_bar_border = color(LINE);
        t.secondary = color(PANEL);
        t.secondary_foreground = color(TEXT);
        t.secondary_hover = color(RAISED);
        t.secondary_active = color(CONTROL);
        t.danger = color(0xc42b1c);
        t.danger_foreground = color(STRONG);
        t.danger_active = color(0x9e2216);
        t.scrollbar_thumb = color(CONTROL);
        t.scrollbar_thumb_hover = color(0x44444b);
    });
}
