//! Rendering for the terminal-style search window.
use super::{FocusSearch, SearchApp, failed_batch};
use crate::style::*;
use gpui_kit::component::{
    Disableable, Sizable, Theme,
    button::{Button, ButtonCustomVariant, ButtonVariants},
    input::Input,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use nswpedia_search::resolver::BATCH_SIZE;
use std::time::Duration;

impl SearchApp {
    fn ready(&self) -> usize {
        self.files.iter().filter(|r| r.url.is_some()).count()
    }
    fn status(&self) -> (&'static str, u32) {
        if self.search_busy {
            ("SCANNING", CYAN)
        } else if self.resolve_busy {
            ("RESOLVING", MAGENTA)
        } else if self.search_error.is_some() || self.resolve_error.is_some() {
            ("FAULT", AMBER)
        } else {
            ("ONLINE", LIME)
        }
    }
    fn cursor(&self, hex: u32) -> Div {
        div()
            .text_color(if self.blink { c(hex) } else { ca(hex, 0.) })
            .child("█")
    }

    fn render_header(&self) -> impl IntoElement {
        let (label, hex) = self.status();
        let busy = self.search_busy || self.resolve_busy;
        let indicator = if busy {
            div()
                .with_animation(
                    ("status-spin", label.len()),
                    Animation::new(Duration::from_millis(720)).repeat(),
                    |el, t| el.child(spinner(t)),
                )
                .into_any_element()
        } else {
            div()
                .text_color(if self.blink || label == "FAULT" {
                    c(hex)
                } else {
                    ca(hex, 0.35)
                })
                .child("●")
                .into_any_element()
        };
        div()
            .flex()
            .items_center()
            .gap_3()
            .px_4()
            .pt_3()
            .pb_2()
            .child(logo(self.glitch))
            .child(
                div()
                    .text_xs()
                    .text_color(c(FAINT))
                    .child("// 1FICHIER LINK RESOLVER"),
            )
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_0p5()
                    .text_xs()
                    .text_color(c(hex))
                    .border_1()
                    .border_color(ca(hex, 0.45))
                    .bg(ca(hex, 0.06))
                    .shadow(glow(hex, 0.25, 10.))
                    .child(indicator)
                    .child(format!("SYS::{label}")),
            )
            .relative()
            .with_animation("header-in", entrance(120, 520), |el, t| {
                el.opacity(t).top(px(-14. * (1. - t)))
            })
    }

    fn render_command(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self.search_busy;
        let frame = div()
            .mx_4()
            .mt_1()
            .relative()
            .flex()
            .items_center()
            .gap_3()
            .pl_3()
            .pr_1p5()
            .py_1p5()
            .bg(c(PANEL))
            .border_1()
            .border_color(ca(CYAN, if busy { 0.75 } else { 0.38 }))
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .child(div().text_color(c(MAGENTA)).child("nsw"))
                    .child(div().text_color(c(DIM)).child("@"))
                    .child(div().text_color(c(CYAN)).child("pedia"))
                    .child(div().text_color(c(DIM)).child(":~$")),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(Input::new(&self.input).appearance(false)),
            )
            .child(
                Button::new("search")
                    .custom(term_button(cx, CYAN))
                    .border_1()
                    .border_color(ca(CYAN, 0.6))
                    .px_4()
                    .accessibility_label("Search")
                    .label(if busy { "SCANNING" } else { "EXEC ▸" })
                    .on_click(cx.listener(|this, _, _, cx| this.search(false, cx))),
            )
            .children(corners(CYAN));
        let frame = if busy {
            frame
                .with_animation(
                    "command-pulse",
                    Animation::new(Duration::from_millis(1400))
                        .repeat()
                        .with_easing(pulsating_between(0.15, 0.55)),
                    |el, t| el.shadow(glow(CYAN, t, 22.)),
                )
                .into_any_element()
        } else {
            frame.shadow(glow(CYAN, 0.14, 18.)).into_any_element()
        };
        div()
            .relative()
            .child(frame)
            .with_animation("command-in", entrance(260, 520), |el, t| {
                el.opacity(t).left(px(-24. * (1. - t)))
            })
    }

    fn render_activity(&self) -> impl IntoElement {
        let track = div()
            .mx_4()
            .mt_2()
            .mb_3()
            .h(px(2.))
            .relative()
            .overflow_hidden()
            .bg(c(LINE));
        if self.search_busy || self.resolve_busy {
            let head = if self.search_busy { CYAN } else { MAGENTA };
            track.child(
                div()
                    .absolute()
                    .top_0()
                    .h_full()
                    .w(relative(0.35))
                    .bg(linear_gradient(
                        90.,
                        linear_color_stop(ca(head, 0.), 0.),
                        linear_color_stop(c(head), 1.),
                    ))
                    .with_animation(
                        "activity",
                        Animation::new(Duration::from_millis(1100))
                            .repeat()
                            .with_easing(ease_in_out),
                        |el, t| el.left(relative(-0.35 + 1.35 * t)),
                    ),
            )
        } else if !self.files.is_empty() {
            let ratio = self.ready() as f32 / self.files.len() as f32;
            track.child(
                div()
                    .h_full()
                    .w(relative(ratio))
                    .bg(ca(LIME, 0.7))
                    .shadow(glow(LIME, 0.5, 6.)),
            )
        } else {
            track
        }
    }

    fn render_results(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = div()
            .id("results")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_1p5();
        if !self.searched {
            list = list.child(
                div()
                    .flex()
                    .gap_1()
                    .p_2()
                    .text_color(c(DIM))
                    .child("> AWAITING QUERY")
                    .child(self.cursor(CYAN)),
            );
        }
        if let Some(error) = &self.search_error {
            list = list.child(fault(error));
        }
        if self.search_busy && self.games.is_empty() {
            for index in 0..7 {
                list = list.child(skeleton(index));
            }
        }
        if self.searched
            && !self.search_busy
            && self.games.is_empty()
            && self.search_error.is_none()
        {
            list = list.child(
                div()
                    .p_2()
                    .text_color(c(DIM))
                    .child("NO RESULTS // TRY ANOTHER TITLE"),
            );
        }
        for (index, game) in self.games.iter().enumerate() {
            let url = game.source_url.clone();
            let selected = self.selected == Some(index);
            let variant = if selected {
                ButtonCustomVariant::new(cx)
                    .color(ca(MAGENTA, 0.12))
                    .foreground(c(0xffffff))
                    .hover(ca(MAGENTA, 0.18))
                    .active(ca(MAGENTA, 0.26))
            } else {
                ButtonCustomVariant::new(cx)
                    .foreground(c(TEXT))
                    .hover(ca(CYAN, 0.08))
                    .active(ca(CYAN, 0.16))
            };
            let row = div()
                .relative()
                .flex()
                .items_center()
                .gap_1()
                .mb_0p5()
                .child(
                    Button::new(("game", index))
                        .custom(variant)
                        .flex_1()
                        .min_w_0()
                        .h_auto()
                        .py_2()
                        .pl_2()
                        .border_l_2()
                        .border_color(if selected {
                            c(MAGENTA)
                        } else {
                            ca(MAGENTA, 0.)
                        })
                        .accessibility_label(game.title.clone())
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .gap_2()
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .text_xs()
                                        .text_color(if selected { c(MAGENTA) } else { c(FAINT) })
                                        .child(format!("{:02}", index + 1)),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .child(game.title.clone()),
                                )
                                .when(selected, |el| {
                                    el.child(div().text_color(c(MAGENTA)).child("◂"))
                                }),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| this.select(index, cx))),
                )
                .child(
                    Button::new(("source", index))
                        .custom(term_button(cx, DIM))
                        .small()
                        .label("↗")
                        .tooltip("Open source page")
                        .on_click(move |_, _, cx| cx.open_url(&url)),
                );
            list = list.child(row.with_animation(
                format!("game-in-{}-{}", self.search_id, game.id),
                entrance((index % 24) as u64 * 30, 320),
                |el, t| el.opacity(t).left(px(-18. * (1. - t))),
            ));
        }
        if self.page < self.total_pages && self.page < 100 {
            list = list.child(
                Button::new("more-results")
                    .custom(term_button(cx, CYAN))
                    .w_full()
                    .mt_2()
                    .border_1()
                    .border_color(ca(CYAN, 0.25))
                    .accessibility_label("More results")
                    .label(if self.search_busy {
                        "SCANNING…"
                    } else {
                        "[+] MORE RESULTS"
                    })
                    .disabled(self.search_busy)
                    .on_click(cx.listener(|this, _, _, cx| this.search(true, cx))),
            );
        }
        let meta = if self.total_pages > 0 {
            format!(
                "{} HITS · P{}/{}",
                self.games.len(),
                self.page,
                self.total_pages
            )
        } else {
            String::new()
        };
        panel("RESULTS", meta, CYAN)
            .w(px(300.))
            .flex_shrink_0()
            .child(list)
            .with_animation("results-in", entrance(380, 480), |el, t| {
                el.opacity(t).top(px(16. * (1. - t)))
            })
    }

    fn render_detail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(game) = self.selected.and_then(|i| self.games.get(i)) else {
            let body = if !self.searched {
                splash().into_any_element()
            } else if self.search_busy {
                centered(
                    div()
                        .flex()
                        .gap_2()
                        .text_color(c(CYAN))
                        .child(div().with_animation(
                            "index-spin",
                            Animation::new(Duration::from_millis(720)).repeat(),
                            |el, t| el.child(spinner(t)),
                        ))
                        .child("SCANNING INDEX"),
                )
            } else {
                centered(
                    div()
                        .flex()
                        .gap_1()
                        .text_color(c(DIM))
                        .child("NO SIGNAL")
                        .child(self.cursor(DIM)),
                )
            };
            return panel("FILES", String::new(), MAGENTA)
                .flex_1()
                .min_w_0()
                .child(body)
                .with_animation("detail-in", entrance(460, 480), |el, t| {
                    el.opacity(t).top(px(16. * (1. - t)))
                });
        };
        let ready = self.ready();
        let total = self.files.len();
        let source = game.source_url.clone();
        let title = game.title.clone();
        let seed = game.id;
        let heading = div()
            .px_5()
            .pt_4()
            .pb_3()
            .border_b_1()
            .border_color(c(LINE))
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .text_color(c(CYAN))
                            .with_animation(
                                ("title", game.id),
                                Animation::new(Duration::from_millis(650)),
                                move |el, t| el.child(scramble(&title, t, seed)),
                            ),
                    )
                    .child(
                        Button::new("source-page")
                            .custom(term_button(cx, DIM))
                            .small()
                            .border_1()
                            .border_color(c(LINE_HI))
                            .accessibility_label("Source page")
                            .label("SOURCE ↗")
                            .on_click(move |_, _, cx| cx.open_url(&source)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .mt_2()
                    .text_xs()
                    .text_color(c(DIM))
                    .child(format!("ID:{:07}", game.id))
                    .child(div().text_color(c(FAINT)).child("│"))
                    .child(format!("{total} FILES"))
                    .child(div().text_color(c(FAINT)).child("│"))
                    .child(
                        div()
                            .text_color(if total > 0 && ready == total {
                                c(LIME)
                            } else {
                                c(DIM)
                            })
                            .child(format!("{ready}/{total} READY")),
                    )
                    .child(meter(ready, total))
                    .child(div().flex_1())
                    .when(ready > 1, |el| {
                        el.child(self.copy_button("copy-all", None, "COPY ALL", cx))
                    }),
            );
        let mut list = div()
            .id("files")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px_5()
            .pb_4();
        if self.files.is_empty() {
            if self.resolve_busy {
                list = list.child(
                    div()
                        .flex()
                        .gap_2()
                        .py_4()
                        .text_color(c(MAGENTA))
                        .child(div().with_animation(
                            "manifest-spin",
                            Animation::new(Duration::from_millis(720)).repeat(),
                            |el, t| el.child(spinner(t)),
                        ))
                        .child("FETCHING FILE MANIFEST"),
                );
            } else if self.resolve_error.is_none() {
                list = list.child(
                    div()
                        .py_4()
                        .text_color(c(DIM))
                        .child("NO 1FICHIER LINKS LISTED"),
                );
            }
        }
        for (index, row) in self.files.iter().enumerate() {
            let key = row.file.key.clone();
            let resolving = self.resolve_busy
                && (self.batch_offset..self.batch_offset + BATCH_SIZE).contains(&index);
            let tag = if row.url.is_some() {
                div().text_color(c(LIME)).child("[ OK ]").into_any_element()
            } else if row.error.is_some() {
                div()
                    .text_color(c(AMBER))
                    .child("[FAIL]")
                    .into_any_element()
            } else if resolving {
                div()
                    .text_color(c(MAGENTA))
                    .with_animation(
                        ("row-spin", index),
                        Animation::new(Duration::from_millis(720)).repeat_synced(),
                        |el, t| el.child(spinner(t)),
                    )
                    .into_any_element()
            } else {
                div()
                    .text_color(c(FAINT))
                    .child("[ -- ]")
                    .into_any_element()
            };
            let mut info = div().flex_1().min_w_0().child(row.file.name.clone()).child(
                div().text_xs().mt_0p5().text_color(c(DIM)).child(
                    [row.file.size.as_str(), row.file.format.as_str()]
                        .into_iter()
                        .filter(|part| !part.is_empty())
                        .collect::<Vec<_>>()
                        .join(" · "),
                ),
            );
            if let Some(url) = &row.url {
                let url = url.clone();
                info = info.child(div().text_xs().mt_1p5().text_color(c(LIME)).with_animation(
                    format!("url-{}-{}", game.id, key),
                    Animation::new(Duration::from_millis(420)),
                    move |el, t| el.child(typed(&url, t)),
                ));
            } else if let Some(error) = &row.error {
                info = info.child(
                    div()
                        .text_xs()
                        .mt_1p5()
                        .text_color(c(AMBER))
                        .child(format!("!! {error}")),
                );
            } else {
                info = info.child(
                    div()
                        .text_xs()
                        .mt_1p5()
                        .text_color(c(FAINT))
                        .child(if resolving { "resolving…" } else { "pending" }),
                );
            }
            let mut line = div()
                .flex()
                .items_start()
                .gap_3()
                .py_3()
                .px_2()
                .border_b_1()
                .border_color(c(LINE))
                .child(div().flex_shrink_0().text_xs().pt_0p5().child(tag))
                .child(info);
            if row.url.is_some() {
                line = line.child(self.copy_button(("copy", index), Some(key.clone()), "COPY", cx));
            }
            let line = if row.url.is_some() {
                line.with_animation(
                    format!("landed-{}-{}", game.id, key),
                    Animation::new(Duration::from_millis(900)),
                    |el, t| el.bg(ca(LIME, 0.10 * (1. - ease_out(t)))),
                )
                .into_any_element()
            } else {
                line.into_any_element()
            };
            list = list.child(div().relative().child(line).with_animation(
                format!("file-in-{}-{}", game.id, key),
                entrance((index % BATCH_SIZE) as u64 * 24, 300),
                |el, t| el.opacity(t).left(px(22. * (1. - t))),
            ));
        }
        if let Some(error) = &self.resolve_error {
            list = list.child(
                div()
                    .mt_4()
                    .flex()
                    .items_center()
                    .gap_3()
                    .p_3()
                    .border_l_2()
                    .border_color(c(AMBER))
                    .bg(ca(AMBER, 0.06))
                    .child(
                        div()
                            .flex_1()
                            .text_color(c(AMBER))
                            .child(format!("!! {error}")),
                    )
                    .child(
                        Button::new("retry")
                            .custom(term_button(cx, AMBER))
                            .border_1()
                            .border_color(ca(AMBER, 0.5))
                            .accessibility_label("Retry")
                            .label("RETRY ↻")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let offset = failed_batch(&this.files).unwrap_or(this.batch_offset);
                                this.resolve(offset, cx);
                            })),
                    )
                    .with_animation(("fault", self.resolve_id), entrance(0, 360), |el, t| {
                        el.opacity(t)
                    }),
            );
        }
        if let Some(offset) = self.next_offset {
            list = list.child(
                Button::new("more-files")
                    .custom(term_button(cx, CYAN))
                    .mt_4()
                    .border_1()
                    .border_color(ca(CYAN, 0.45))
                    .accessibility_label("Resolve more")
                    .label("RESOLVE NEXT BATCH ▸")
                    .on_click(cx.listener(move |this, _, _, cx| this.resolve(offset, cx))),
            );
        }
        panel("FILES", format!("{ready}/{total}"), MAGENTA)
            .flex_1()
            .min_w_0()
            .child(heading)
            .child(list)
            .with_animation("detail-in", entrance(460, 480), |el, t| {
                el.opacity(t).top(px(16. * (1. - t)))
            })
    }

    fn copy_button(
        &self,
        id: impl Into<ElementId>,
        key: Option<String>,
        label: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let copied = self.copied.as_deref() == Some(key.as_deref().unwrap_or("all"));
        let hex = if copied { LIME } else { CYAN };
        let button = Button::new(id)
            .custom(term_button(cx, hex))
            .small()
            .flex_shrink_0()
            .border_1()
            .border_color(ca(hex, 0.55))
            .accessibility_label(if key.is_some() { "Copy" } else { "Copy all" })
            .label(if copied { "COPIED ✓" } else { label })
            .on_click(cx.listener(move |this, _, _, cx| this.copy(key.as_deref(), cx)));
        if copied {
            button
                .with_animation(("copied", self.copy_seq), entrance(0, 700), |el, t| {
                    el.shadow(glow(LIME, 0.9 * (1. - t), 16.))
                })
                .into_any_element()
        } else {
            button.into_any_element()
        }
    }

    fn render_footer(&self) -> impl IntoElement {
        let modifier = if cfg!(target_os = "macos") {
            "⌘"
        } else {
            "^"
        };
        let hint = |keys: String, label: &'static str| {
            div()
                .flex()
                .items_center()
                .gap_1p5()
                .child(
                    div()
                        .px_1()
                        .bg(ca(CYAN, 0.12))
                        .text_color(c(CYAN))
                        .child(keys),
                )
                .child(label)
        };
        div()
            .flex()
            .items_center()
            .gap_4()
            .px_4()
            .py_1p5()
            .border_t_1()
            .border_color(c(LINE))
            .bg(c(PANEL))
            .text_xs()
            .text_color(c(DIM))
            .child(hint(format!("{modifier}L"), "FOCUS"))
            .child(hint("ENTER".into(), "SEARCH"))
            .child(hint(format!("{modifier}Q"), "QUIT"))
            .child(div().flex_1())
            .when(self.copied.is_some(), |el| {
                el.child(
                    div()
                        .text_color(c(LIME))
                        .child("» COPIED TO CLIPBOARD")
                        .with_animation(
                            ("copied-note", self.copy_seq),
                            entrance(0, 400),
                            |el, t| el.opacity(t),
                        ),
                )
            })
            .child(format!("{} TITLES", self.games.len()))
            .child(format!("LINKS {}/{}", self.ready(), self.files.len()))
    }
}

impl Render for SearchApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self.search_busy || self.resolve_busy;
        let font = Theme::global(cx).mono_font_family.clone();
        div()
            .key_context("SearchApp")
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .font_family(font)
            .bg(c(BG))
            .text_color(c(TEXT))
            .text_sm()
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                this.input.update(cx, |input, cx| input.focus(window, cx))
            }))
            .child(self.render_header())
            .child(self.render_command(cx))
            .child(self.render_activity())
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .gap_3()
                    .px_4()
                    .pb_3()
                    .child(self.render_results(cx))
                    .child(self.render_detail(cx)),
            )
            .child(self.render_footer())
            .when(busy, |el| el.child(sweep()))
            .child(scanlines())
            .child(boot_flash())
    }
}

fn logo(glitch: u64) -> impl IntoElement {
    const WORD: &str = "NSWPEDIA//SEARCH";
    div()
        .relative()
        .font_weight(FontWeight::BOLD)
        .child(
            div()
                .flex()
                .child(div().text_color(c(CYAN)).child("NSWPEDIA"))
                .child(div().text_color(c(MAGENTA)).child("//"))
                .child(div().text_color(c(TEXT)).child("SEARCH")),
        )
        .with_animation(
            ("glitch", glitch),
            Animation::new(Duration::from_millis(460)),
            move |el, t| {
                if t >= 1. {
                    return el;
                }
                let n = noise(glitch + 1, (t * 22.) as u64);
                let dx = (n % 9) as f32 - 4.;
                let dy = ((n >> 8) % 3) as f32 - 1.;
                let text = if n % 3 == 0 {
                    scramble(WORD, 0.4 + t * 0.6, n)
                } else {
                    WORD.to_string()
                };
                el.child(
                    div()
                        .absolute()
                        .top(px(dy))
                        .left(px(dx))
                        .text_color(ca(MAGENTA, 0.7))
                        .child(text.clone()),
                )
                .child(
                    div()
                        .absolute()
                        .top(px(-dy))
                        .left(px(-dx))
                        .text_color(ca(CYAN, 0.7))
                        .child(text),
                )
            },
        )
}

fn corners(hex: u32) -> [Div; 4] {
    let arm = px(9.);
    let corner = |top: bool, left: bool| {
        let el = div().absolute().w(arm).h(arm).border_color(c(hex));
        let el = if top {
            el.top(px(-1.)).border_t_2()
        } else {
            el.bottom(px(-1.)).border_b_2()
        };
        if left {
            el.left(px(-1.)).border_l_2()
        } else {
            el.right(px(-1.)).border_r_2()
        }
    };
    [
        corner(true, true),
        corner(true, false),
        corner(false, true),
        corner(false, false),
    ]
}

fn panel(title: &'static str, meta: String, accent: u32) -> Div {
    div()
        .relative()
        .flex()
        .flex_col()
        .min_h_0()
        .bg(c(PANEL))
        .border_1()
        .border_color(c(LINE))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_1p5()
                .border_b_1()
                .border_color(c(LINE))
                .text_xs()
                .child(div().text_color(c(accent)).child("■"))
                .child(div().text_color(c(DIM)).child(title))
                .child(div().flex_1().h(px(1.)).bg(c(LINE)))
                .child(div().text_color(c(FAINT)).child(meta)),
        )
        .children(corners(accent).map(|el| el.border_color(ca(accent, 0.7))))
}

fn meter(ready: usize, total: usize) -> impl IntoElement {
    let ratio = if total == 0 {
        0.
    } else {
        ready as f32 / total as f32
    };
    div().w(px(120.)).h(px(5.)).bg(c(LINE)).child(
        div()
            .h_full()
            .w(relative(ratio))
            .bg(if ratio >= 1. { c(LIME) } else { c(MAGENTA) }),
    )
}

fn fault(message: &str) -> impl IntoElement {
    div()
        .m_1()
        .p_2()
        .border_l_2()
        .border_color(c(AMBER))
        .bg(ca(AMBER, 0.06))
        .text_color(c(AMBER))
        .child(format!("!! {message}"))
}

fn skeleton(index: usize) -> impl IntoElement {
    let width = 0.45 + (noise(index as u64, 3) % 45) as f32 / 100.;
    div()
        .flex()
        .gap_2()
        .items_center()
        .px_2()
        .py_2p5()
        .child(div().w(px(16.)).h(px(8.)).bg(c(FAINT)))
        .child(div().w(relative(width)).h(px(8.)).bg(c(LINE_HI)))
        .with_animation(
            ("skeleton", index),
            Animation::new(Duration::from_millis(1100))
                .repeat_synced()
                .with_easing(move |t| {
                    let phase = (t - index as f32 * 0.09).rem_euclid(1.);
                    0.25 + 0.75 * (phase * std::f32::consts::PI).sin()
                }),
            |el, t| el.opacity(t),
        )
}

fn centered(content: impl IntoElement) -> AnyElement {
    div()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .child(content)
        .into_any_element()
}

fn splash() -> impl IntoElement {
    const TINTS: [u32; 3] = [CYAN, 0x8f8cff, MAGENTA];
    let mut art = div()
        .flex()
        .flex_col()
        .text_size(px(18.))
        .line_height(px(20.));
    for (index, line) in banner().into_iter().enumerate() {
        art = art.child(
            div()
                .relative()
                .text_color(c(TINTS[index]))
                .child(line)
                .with_animation(
                    ("banner", index),
                    entrance(650 + index as u64 * 70, 300),
                    |el, t| el.opacity(t).left(px(-30. * (1. - t))),
                ),
        );
    }
    centered(
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_4()
            .child(div().shadow(glow(CYAN, 0.08, 40.)).child(art))
            .child(div().text_xs().text_color(c(DIM)).with_animation(
                "splash-tag",
                Animation::new(Duration::from_millis(1700)),
                |el, t| {
                    el.child(typed(
                        "> TYPE A GAME TITLE AND PRESS ENTER",
                        ((t - 0.6) / 0.4).clamp(0., 1.),
                    ))
                },
            )),
    )
}
