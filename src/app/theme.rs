//! The values the window draws with, from record/areas/interface/decisions.md, which is where to
//! change them first.

use gpui_kit::component::Theme;
use gpui_kit::*;

pub const BASE: u32 = 0x15171b;
pub const PANEL: u32 = 0x1a1d22;
pub const RAISED: u32 = 0x232830;
pub const LINE: u32 = 0x272b32;
pub const LINE_SOFT: u32 = 0x1e2126;
pub const CONTROL: u32 = 0x3a404a;
pub const TEXT: u32 = 0xdfe2e7;
pub const STRONG: u32 = 0xf4f5f7;
pub const MUTED: u32 = 0x959cab;
/// What's the player's own, or happening now.
pub const ACCENT: u32 = 0xddc94f;
pub const ON_ACCENT: u32 = 0x15171b;
pub const DONE: u32 = 0x8b94a3;
/// Something's off.
pub const WARNING: u32 = 0xe89b5e;
/// Something broke.
pub const TROUBLE: u32 = 0xe5736e;

pub const TEXT_SIZE: Pixels = px(13.);
pub const SMALL_SIZE: Pixels = px(12.);
pub const TITLE_SIZE: Pixels = px(16.);
pub const LINE_HEIGHT: Pixels = px(16.);

pub const TITLE_BAR: Pixels = px(32.);
pub const SECTION: Pixels = px(28.);
pub const STATUS_BAR: Pixels = px(24.);
pub const ROW: Pixels = px(26.);
pub const HEADER_ROW: Pixels = px(24.);
pub const PLAYERS_WIDTH: Pixels = px(280.);
pub const RADIUS: Pixels = px(3.);
pub const ICON: Pixels = px(14.);

pub fn color(hex: u32) -> Hsla {
    rgb(hex).into()
}

/// Gives gpui-component's own controls, such as the text inputs and the title bar's window
/// buttons, the same colors as everything drawn by hand.
pub fn apply(cx: &mut App) {
    Theme::update(cx, |t| {
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
        t.selection = color(0x3d4552);
        t.muted = color(RAISED);
        t.muted_foreground = color(MUTED);
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
        t.scrollbar_thumb_hover = color(0x4a515c);
    });
}
