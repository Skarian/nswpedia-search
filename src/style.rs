//! Quiet terminal palette and small motion helpers for the GUI.
use gpui_kit::component::button::ButtonCustomVariant;
use gpui_kit::*;
use std::time::Duration;

pub const BG: u32 = 0x0e0f11;
pub const PANEL: u32 = 0x131417;
pub const RAISED: u32 = 0x1a1c20;
pub const LINE: u32 = 0x22252a;
pub const LINE_HI: u32 = 0x30343a;
pub const ACCENT: u32 = 0x8fcf9f;
pub const WARN: u32 = 0xd9a066;
pub const TEXT: u32 = 0xd6d8dc;
pub const DIM: u32 = 0x80858e;
/// Lowest-emphasis text, kept near 3.7:1 against `PANEL` so it stays legible.
pub const FAINT: u32 = 0x6b7079;

pub fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}
pub fn ca(hex: u32, alpha: f32) -> Hsla {
    c(hex).opacity(alpha)
}

pub fn ease_out(t: f32) -> f32 {
    1. - (1. - t.clamp(0., 1.)).powi(3)
}

/// Short opacity fade used when content first appears.
pub fn fade() -> Animation {
    Animation::new(Duration::from_millis(180)).with_easing(ease_out)
}

pub const SPINNER: [&str; 4] = ["-", "\\", "|", "/"];

pub fn spinner(t: f32) -> &'static str {
    SPINNER[((t * SPINNER.len() as f32) as usize).min(SPINNER.len() - 1)]
}

// The kit keeps the arrow cursor on buttons, so every clickable button in the
// view also calls `.cursor_pointer()` to show that it can be pressed.

/// Flat, borderless button that tints on hover.
pub fn ghost_button(cx: &App, hex: u32) -> ButtonCustomVariant {
    ButtonCustomVariant::new(cx)
        .color(ca(hex, 0.))
        .foreground(c(hex))
        .hover(ca(TEXT, 0.07))
        .active(ca(TEXT, 0.16))
}

/// Outlined button for the primary actions.
pub fn outline_button(cx: &App, hex: u32) -> ButtonCustomVariant {
    ButtonCustomVariant::new(cx)
        .color(ca(hex, 0.04))
        .foreground(c(hex))
        .hover(ca(hex, 0.12))
        .active(ca(hex, 0.24))
}
