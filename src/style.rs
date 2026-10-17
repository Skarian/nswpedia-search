//! Terminal/cyberpunk palette, text effects and decorative layers for the GUI.
use gpui_kit::component::button::ButtonCustomVariant;
use gpui_kit::*;
use std::time::Duration;

pub const BG: u32 = 0x05070b;
pub const PANEL: u32 = 0x080d13;
pub const LINE: u32 = 0x15262f;
pub const LINE_HI: u32 = 0x23414f;
pub const CYAN: u32 = 0x00e5ff;
pub const MAGENTA: u32 = 0xff2e88;
pub const LIME: u32 = 0x8cff7a;
pub const AMBER: u32 = 0xffb02e;
pub const TEXT: u32 = 0xd2f3fa;
pub const DIM: u32 = 0x5f7f8c;
pub const FAINT: u32 = 0x2c4652;

pub fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}
pub fn ca(hex: u32, alpha: f32) -> Hsla {
    c(hex).opacity(alpha)
}

pub fn ease_out(t: f32) -> f32 {
    1. - (1. - t.clamp(0., 1.)).powi(3)
}

/// One-shot animation that waits `delay_ms`, then eases out over `duration_ms`.
pub fn entrance(delay_ms: u64, duration_ms: u64) -> Animation {
    let total = (delay_ms + duration_ms).max(1);
    let start = delay_ms as f32 / total as f32;
    let span = duration_ms as f32 / total as f32;
    Animation::new(Duration::from_millis(total)).with_easing(move |t| ease_out((t - start) / span))
}

pub fn noise(a: u64, b: u64) -> u64 {
    let mut x = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^ (x >> 32)
}

const GLYPHS: &[char] = &[
    '#', '%', '&', '@', '$', '/', '\\', '<', '>', '*', '+', '=', '?', '0', '1', '▓', '▒', '░', '_',
    '|',
];

/// Decodes `text` left to right out of random glyphs as `t` goes 0 → 1.
pub fn scramble(text: &str, t: f32, seed: u64) -> String {
    let count = text.chars().count();
    let revealed = (t * 1.2 * count as f32) as usize;
    let frame = (t * 36.) as u64;
    text.chars()
        .enumerate()
        .map(|(i, ch)| {
            if i < revealed || ch.is_whitespace() {
                ch
            } else {
                GLYPHS[(noise(seed ^ i as u64, frame) % GLYPHS.len() as u64) as usize]
            }
        })
        .collect()
}

/// Types `text` out with a block cursor as `t` goes 0 → 1.
pub fn typed(text: &str, t: f32) -> String {
    let count = text.chars().count();
    let shown = (t * count as f32).round() as usize;
    let mut out: String = text.chars().take(shown).collect();
    if shown < count {
        out.push('█');
    }
    out
}

pub const SPINNER: [&str; 6] = ["[=   ]", "[ =  ]", "[  = ]", "[   =]", "[  = ]", "[ =  ]"];

pub fn spinner(t: f32) -> &'static str {
    SPINNER[((t * SPINNER.len() as f32) as usize).min(SPINNER.len() - 1)]
}

pub fn glow(hex: u32, alpha: f32, blur: f32) -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: ca(hex, alpha),
        offset: point(px(0.), px(0.)),
        blur_radius: px(blur),
        spread_radius: px(0.),
        inset: false,
    }]
}

/// Squared, outlined terminal button colors.
pub fn term_button(cx: &App, hex: u32) -> ButtonCustomVariant {
    ButtonCustomVariant::new(cx)
        .color(ca(hex, 0.06))
        .foreground(c(hex))
        .hover(ca(hex, 0.18))
        .active(ca(hex, 0.32))
}

/// Static CRT scanlines over the whole window. No hitbox, so input passes through.
pub fn scanlines() -> impl IntoElement {
    canvas(
        |_, _, _| (),
        |bounds, _, window, _| {
            let color = ca(0x000000, 0.22);
            let mut y = bounds.origin.y;
            let bottom = bounds.origin.y + bounds.size.height;
            while y < bottom {
                window.paint_quad(fill(
                    Bounds::new(point(bounds.origin.x, y), size(bounds.size.width, px(1.))),
                    color,
                ));
                y += px(3.);
            }
        },
    )
    .absolute()
    .inset_0()
}

/// Soft cyan band that sweeps down the window while work is in flight.
pub fn sweep() -> impl IntoElement {
    div().absolute().inset_0().overflow_hidden().with_animation(
        "sweep",
        Animation::new(Duration::from_millis(2600)).repeat(),
        |el, t| {
            el.child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(relative(t * 1.3 - 0.2))
                    .h(px(140.))
                    .flex()
                    .flex_col()
                    .child(div().flex_1().bg(linear_gradient(
                        180.,
                        linear_color_stop(ca(CYAN, 0.), 0.),
                        linear_color_stop(ca(CYAN, 0.045), 1.),
                    )))
                    .child(div().h(px(1.)).bg(ca(CYAN, 0.12)))
                    .child(div().flex_1().bg(linear_gradient(
                        180.,
                        linear_color_stop(ca(CYAN, 0.045), 0.),
                        linear_color_stop(ca(CYAN, 0.), 1.),
                    ))),
            )
        },
    )
}

/// CRT power-on: a bright line snaps open into the picture, then fades away.
pub fn boot_flash() -> impl IntoElement {
    div().absolute().inset_0().with_animation(
        "boot",
        Animation::new(Duration::from_millis(750)),
        |el, t| {
            if t >= 1. {
                return el;
            }
            el.child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        let (w, h) = (bounds.size.width, bounds.size.height);
                        let open = ease_out((t / 0.55).min(1.));
                        let fade = 1. - ((t - 0.35) / 0.65).clamp(0., 1.);
                        let gap = h * open;
                        let edge = (h - gap) / 2.;
                        let black = ca(BG, fade.max(0.) * 0.98 + 0.02 * (1. - open));
                        window.paint_quad(fill(Bounds::new(bounds.origin, size(w, edge)), black));
                        window.paint_quad(fill(
                            Bounds::new(
                                point(bounds.origin.x, bounds.origin.y + edge + gap),
                                size(w, edge + px(1.)),
                            ),
                            black,
                        ));
                        let line_w = w * ease_out((t / 0.25).min(1.));
                        let line_h = px(2.) + px(10.) * open;
                        let line = Bounds::new(
                            point(
                                bounds.origin.x + (w - line_w) / 2.,
                                bounds.origin.y + h / 2. - line_h / 2.,
                            ),
                            size(line_w, line_h),
                        );
                        window.paint_quad(fill(line, ca(CYAN, 0.55 * (1. - open) + 0.08 * fade)));
                        if fade > 0. {
                            window.paint_quad(fill(bounds, ca(BG, 0.6 * fade * open)));
                        }
                    },
                )
                .size_full(),
            )
        },
    )
}

/// 5×5 pixel font for "NSWPEDIA"; packed two pixel rows per text line by `banner`.
const BANNER_PIXELS: [&str; 6] = [
    "#   #   ####  #   #  ####   #####  ####   ###   ### ",
    "##  #  #      #   #  #   #  #      #   #   #   #   #",
    "# # #   ###   # # #  ####   ####   #   #   #   #####",
    "#  ##      #  ## ##  #      #      #   #   #   #   #",
    "#   #  ####   #   #  #      #####  ####   ###  #   #",
    "                                                    ",
];

/// Half-block rendering of the pixel banner: square pixels in a monospace font.
pub fn banner() -> Vec<String> {
    BANNER_PIXELS
        .chunks(2)
        .map(|pair| {
            pair[0]
                .chars()
                .zip(pair[1].chars())
                .map(|(top, bottom)| match (top == '#', bottom == '#') {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    (false, false) => ' ',
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{scramble, typed};

    #[::core::prelude::v1::test]
    fn text_effects_settle_on_the_original_text() {
        assert_eq!(scramble("Hollow Knight", 1., 7), "Hollow Knight");
        assert_eq!(scramble("Hollow Knight", 0., 7).chars().count(), 13);
        assert_eq!(
            typed("https://1fichier.com/?a", 1.),
            "https://1fichier.com/?a"
        );
        assert_eq!(typed("abcd", 0.5), "ab█");
    }
}
