//! Rendering for the terminal-style search window.
use super::{FocusSearch, SearchApp, failed_batch};
use crate::style::*;
use gpui_kit::component::{
    Disableable, Selectable, Sizable, Theme,
    button::{Button, ButtonCustomVariant, ButtonVariants},
    input::Input,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use nswpedia_search::{
    downloads::{Job, Status},
    resolver::BATCH_SIZE,
};
use std::{collections::HashMap, time::Duration};

impl SearchApp {
    fn ready(&self) -> usize {
        self.files.iter().filter(|r| r.url.is_some()).count()
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let jobs = self.downloads.jobs();
        let active = jobs.iter().filter(|j| j.status.is_active()).count();
        let failed = jobs
            .iter()
            .filter(|j| matches!(j.status, Status::Failed(_)))
            .count();
        let (queue_label, queue_hex) = if active > 0 {
            (format!("↓ {active}"), ACCENT)
        } else if failed > 0 {
            (format!("↓ {failed} !"), WARN)
        } else {
            ("↓".to_string(), DIM)
        };
        let queue_name = match (active, failed) {
            (0, 0) => "Downloads".to_string(),
            (0, failed) => format!("Downloads, {failed} failed"),
            (active, _) => format!("Downloads, {active} active"),
        };
        let busy = self.search_busy || self.resolve_busy;
        let status = if self.search_busy {
            working("searching")
        } else if self.resolve_busy {
            working("resolving")
        } else if self.search_error.is_some() || self.resolve_error.is_some() {
            div().text_color(c(WARN)).child("error").into_any_element()
        } else {
            div()
                .flex()
                .items_center()
                .gap_1p5()
                .child(div().text_color(ca(ACCENT, 0.8)).child("●"))
                .child("ready")
                .into_any_element()
        };
        div()
            .flex()
            .items_center()
            .px_5()
            .pt_4()
            .pb_3()
            .child(
                div()
                    .flex()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(div().text_color(c(TEXT)).child("nswpedia"))
                    .child(div().text_color(c(FAINT)).child("/"))
                    .child(div().text_color(c(DIM)).child("search")),
            )
            .child(div().flex_1())
            .child(
                div()
                    .text_xs()
                    .text_color(if busy { c(DIM) } else { c(FAINT) })
                    .child(status),
            )
            .child(
                Button::new("open-queue")
                    .custom(ghost_button(cx, queue_hex))
                    .small()
                    .ml_3()
                    .cursor_pointer()
                    .accessibility_label(queue_name)
                    .tooltip("Downloads")
                    .label(queue_label)
                    .on_click(cx.listener(|this, _, window, cx| this.open_queue(window, cx))),
            )
            .child(
                Button::new("open-settings")
                    .custom(ghost_button(cx, DIM))
                    .small()
                    .cursor_pointer()
                    .accessibility_label("Settings")
                    .label("settings")
                    .on_click(cx.listener(|this, _, window, cx| this.open_settings(window, cx))),
            )
    }

    fn render_command(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self.search_busy;
        div()
            .mx_5()
            .flex()
            .items_center()
            .gap_3()
            .pl_3()
            .pr_1()
            .py_1()
            .bg(c(PANEL))
            .border_1()
            .border_color(c(LINE_HI))
            .child(div().flex_shrink_0().text_color(c(ACCENT)).child("›"))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(Input::new(&self.input).appearance(false)),
            )
            .child(
                Button::new("search")
                    .custom(ghost_button(cx, if busy { DIM } else { TEXT }))
                    .px_3()
                    .cursor_pointer()
                    .accessibility_label(if busy { "Searching" } else { "Search" })
                    .label(if busy { "searching…" } else { "search ↵" })
                    .on_click(cx.listener(|this, _, _, cx| this.search(false, cx))),
            )
    }

    fn render_activity(&self) -> impl IntoElement {
        let track = div().mx_5().h(px(1.)).relative().overflow_hidden();
        if self.search_busy || self.resolve_busy {
            track.child(
                div()
                    .absolute()
                    .top_0()
                    .h_full()
                    .w(relative(0.25))
                    .bg(ca(ACCENT, 0.6))
                    .with_animation(
                        "activity",
                        Animation::new(Duration::from_millis(1400))
                            .repeat()
                            .with_easing(ease_in_out),
                        |el, t| el.left(relative(-0.25 + 1.25 * t)),
                    ),
            )
        } else if !self.files.is_empty() {
            let ratio = self.ready() as f32 / self.files.len() as f32;
            track.child(div().h_full().w(relative(ratio)).bg(ca(ACCENT, 0.45)))
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
            .p_1();
        if let Some(error) = &self.search_error {
            list = list.child(fault(error));
        }
        if self.search_busy && self.games.is_empty() {
            list = list.child(skeleton());
        }
        if self.searched
            && !self.search_busy
            && self.games.is_empty()
            && self.search_error.is_none()
        {
            list = list.child(note("no results — try another title"));
        }
        let mut games = div().flex().flex_col();
        for (index, game) in self.games.iter().enumerate() {
            let url = game.source_url.clone();
            let selected = self.selected == Some(index);
            let variant = if selected {
                ButtonCustomVariant::new(cx)
                    .color(c(RAISED))
                    .foreground(c(TEXT))
                    .hover(c(RAISED))
                    .active(c(RAISED))
            } else {
                ButtonCustomVariant::new(cx)
                    .foreground(ca(TEXT, 0.72))
                    .hover(ca(TEXT, 0.06))
                    .active(ca(TEXT, 0.14))
            };
            games = games.child(
                div()
                    .flex()
                    .items_center()
                    .child(
                        Button::new(("game", index))
                            .custom(variant)
                            .flex_1()
                            .min_w_0()
                            .h_auto()
                            .py_1p5()
                            .pl_2()
                            .border_l_2()
                            .border_color(if selected { c(ACCENT) } else { ca(ACCENT, 0.) })
                            .cursor_pointer()
                            .selected(selected)
                            .accessibility_label(game.title.clone())
                            .tooltip(game.title.clone())
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .gap_3()
                                    .child(
                                        div()
                                            .flex_shrink_0()
                                            .text_xs()
                                            .text_color(c(FAINT))
                                            .child(format!("{:02}", index + 1)),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .child(game.title.clone()),
                                    ),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| this.select(index, cx))),
                    )
                    .child(
                        Button::new(("source", index))
                            .custom(ghost_button(cx, FAINT))
                            .small()
                            .cursor_pointer()
                            .accessibility_label(format!("Open source page for {}", game.title))
                            .label("↗")
                            .tooltip("Open source page")
                            .on_click(move |_, _, cx| cx.open_url(&url)),
                    ),
            );
        }
        list = list
            .child(games.with_animation(("games", self.search_id), fade(), |el, t| el.opacity(t)));
        if self.page < self.total_pages && self.page < 100 {
            list = list.child(
                Button::new("more-results")
                    .custom(ghost_button(cx, DIM))
                    .w_full()
                    .mt_1()
                    .when(!self.search_busy, |el| el.cursor_pointer())
                    .accessibility_label("More results")
                    .label(if self.search_busy {
                        "loading…"
                    } else {
                        "more results"
                    })
                    .disabled(self.search_busy)
                    .on_click(cx.listener(|this, _, _, cx| this.search(true, cx))),
            );
        }
        let meta = if self.total_pages > 0 {
            format!(
                "{} · page {}/{}",
                self.games.len(),
                self.page,
                self.total_pages
            )
        } else {
            String::new()
        };
        panel("results", meta)
            .w(px(300.))
            .flex_shrink_0()
            .child(list)
    }

    fn render_detail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(game) = self.selected.and_then(|i| self.games.get(i)) else {
            let body = if !self.searched {
                centered(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .text_color(c(DIM))
                                .child(div().text_color(c(ACCENT)).child("›"))
                                .child("type a game title and press enter")
                                .child(div().text_color(ca(ACCENT, 0.7)).child("_")),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(c(FAINT))
                                .child("resolves 1fichier landing links"),
                        ),
                )
            } else if self.search_busy {
                centered(working("searching index"))
            } else {
                centered(note("nothing selected"))
            };
            return panel("files", String::new()).flex_1().min_w_0().child(body);
        };
        let ready = self.ready();
        let total = self.files.len();
        let source = game.source_url.clone();
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
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(c(TEXT))
                            .child(game.title.clone()),
                    )
                    .child(
                        Button::new("source-page")
                            .custom(ghost_button(cx, DIM))
                            .small()
                            .cursor_pointer()
                            .accessibility_label("Open source page")
                            .label("source ↗")
                            .on_click(move |_, _, cx| cx.open_url(&source)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .mt_1p5()
                    .h(px(24.))
                    .text_xs()
                    .text_color(c(DIM))
                    .child(format!("{total} file{}", if total == 1 { "" } else { "s" }))
                    .child(div().text_color(c(FAINT)).child("·"))
                    .child(
                        div()
                            .text_color(if total > 0 && ready == total {
                                c(ACCENT)
                            } else {
                                c(DIM)
                            })
                            .child(format!("{ready} ready")),
                    )
                    .child(div().flex_1())
                    .when(ready > 1, |el| {
                        el.child(
                            Button::new("download-all")
                                .custom(ghost_button(cx, DIM))
                                .small()
                                .flex_shrink_0()
                                .border_1()
                                .border_color(c(LINE_HI))
                                .cursor_pointer()
                                .accessibility_label(format!("Download all {ready} ready files"))
                                .label("download all")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.download(None, window, cx)
                                })),
                        )
                        .child(self.copy_button(
                            "copy-all",
                            None,
                            "copy all",
                            "Copy all links".into(),
                            cx,
                        ))
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
                list = list.child(div().py_4().child(working("fetching file list")));
            } else if self.resolve_error.is_none() {
                list = list.child(div().py_4().child(note("no 1fichier links listed")));
            }
        }
        let root = self.settings.download_dir();
        let jobs: HashMap<String, Job> = self
            .downloads
            .jobs()
            .into_iter()
            .filter(|job| job.root == root)
            .map(|job| (job.link.clone(), job))
            .collect();
        let mut rows = div().flex().flex_col();
        for (index, row) in self.files.iter().enumerate() {
            let key = row.file.key.clone();
            let resolving = self.resolve_busy
                && (self.batch_offset..self.batch_offset + BATCH_SIZE).contains(&index);
            let mark = if row.url.is_some() {
                div().text_color(c(ACCENT)).child("✓").into_any_element()
            } else if row.error.is_some() {
                div().text_color(c(WARN)).child("!").into_any_element()
            } else if resolving {
                div()
                    .text_color(c(DIM))
                    .with_animation(
                        ("row-spin", index),
                        Animation::new(Duration::from_millis(600)).repeat_synced(),
                        |el, t| el.child(spinner(t)),
                    )
                    .into_any_element()
            } else {
                div().text_color(c(FAINT)).child("·").into_any_element()
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
                info = info.child(
                    div()
                        .text_xs()
                        .mt_1()
                        .text_color(ca(ACCENT, 0.85))
                        .child(url.clone())
                        .with_animation(format!("url-{}-{}", game.id, key), fade(), |el, t| {
                            el.opacity(t)
                        }),
                );
            } else if let Some(error) = &row.error {
                info = info.child(
                    div()
                        .text_xs()
                        .mt_1()
                        .text_color(c(WARN))
                        .child(error.clone()),
                );
            } else {
                info = info.child(
                    div()
                        .text_xs()
                        .mt_1()
                        .text_color(c(FAINT))
                        .child(if resolving { "resolving…" } else { "pending" }),
                );
            }
            rows = rows.child(
                div()
                    .flex()
                    .items_start()
                    .gap_3()
                    .py_3()
                    .border_b_1()
                    .border_color(c(LINE))
                    .child(div().flex_shrink_0().w(px(12.)).child(mark))
                    .child(info)
                    .when_some(row.url.as_ref(), |el, url| {
                        el.child(self.download_button(
                            index,
                            key.clone(),
                            &row.file.name,
                            jobs.get(url),
                            cx,
                        ))
                    })
                    .when(row.url.is_some(), |el| {
                        el.child(self.copy_button(
                            ("copy", index),
                            Some(key.clone()),
                            "copy",
                            format!("Copy link for {}", row.file.name).into(),
                            cx,
                        ))
                    }),
            );
        }
        list = list.child(rows.with_animation(("files", game.id), fade(), |el, t| el.opacity(t)));
        if let Some(error) = &self.resolve_error {
            list = list.child(
                div()
                    .mt_4()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(div().flex_1().child(fault(error)))
                    .child(
                        Button::new("retry")
                            .custom(outline_button(cx, WARN))
                            .border_1()
                            .border_color(ca(WARN, 0.4))
                            .cursor_pointer()
                            .accessibility_label("Retry")
                            .label("retry")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let offset = failed_batch(&this.files).unwrap_or(this.batch_offset);
                                this.resolve(offset, cx);
                            })),
                    ),
            );
        }
        if let Some(offset) = self.next_offset {
            list = list.child(
                Button::new("more-files")
                    .custom(outline_button(cx, ACCENT))
                    .mt_4()
                    .border_1()
                    .border_color(ca(ACCENT, 0.35))
                    .cursor_pointer()
                    .accessibility_label("Resolve next batch")
                    .label("resolve next batch")
                    .on_click(cx.listener(move |this, _, _, cx| this.resolve(offset, cx))),
            );
        }
        panel("files", String::new())
            .flex_1()
            .min_w_0()
            .child(heading)
            .child(list)
    }

    fn copy_button(
        &self,
        id: impl Into<ElementId>,
        key: Option<String>,
        label: &'static str,
        name: SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let copied = self.copied.as_deref() == Some(key.as_deref().unwrap_or("all"));
        Button::new(id)
            .custom(ghost_button(cx, if copied { ACCENT } else { DIM }))
            .small()
            .flex_shrink_0()
            .border_1()
            .border_color(if copied { ca(ACCENT, 0.4) } else { c(LINE_HI) })
            .cursor_pointer()
            .accessibility_label(name)
            .label(if copied { "copied" } else { label })
            .on_click(cx.listener(move |this, _, _, cx| this.copy(key.as_deref(), cx)))
            .into_any_element()
    }

    /// Per-file download control. Its label follows the queued job: progress
    /// while active (click opens the queue), `saved` once done (click shows
    /// the file), `retry` after a failure.
    fn download_button(
        &self,
        index: usize,
        key: String,
        name: &str,
        job: Option<&Job>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        enum Click {
            Download,
            Queue,
            Reveal(std::path::PathBuf),
        }
        let (label, hex, a11y, click): (SharedString, u32, String, Click) = match job {
            Some(job) if job.status.is_active() => {
                let label = match (&job.status, job.total) {
                    (Status::Downloading, Some(total)) if total > 0 => {
                        format!("{:.0}%", job.received as f64 * 100. / total as f64).into()
                    }
                    (Status::Downloading, _) => "downloading".into(),
                    (Status::Resolving, _) => "starting".into(),
                    _ => "queued".into(),
                };
                let a11y = format!("{name}: {label}. Open downloads");
                (label, ACCENT, a11y, Click::Queue)
            }
            Some(Job {
                status: Status::Done,
                path: Some(path),
                ..
            }) if path.exists() => (
                "saved".into(),
                ACCENT,
                format!("Show {name} in folder"),
                Click::Reveal(path.clone()),
            ),
            Some(Job {
                status: Status::Failed(_),
                ..
            }) => (
                "retry".into(),
                WARN,
                format!("Download failed. Retry {name}"),
                Click::Download,
            ),
            _ => (
                "download".into(),
                DIM,
                format!("Download {name}"),
                Click::Download,
            ),
        };
        Button::new(("download", index))
            .custom(ghost_button(cx, hex))
            .small()
            .flex_shrink_0()
            .min_w(px(84.))
            .border_1()
            .border_color(if hex == DIM { c(LINE_HI) } else { ca(hex, 0.4) })
            .cursor_pointer()
            .accessibility_label(a11y)
            .label(label)
            .on_click(cx.listener(move |this, _, window, cx| match &click {
                Click::Download => this.download(Some(&key), window, cx),
                Click::Queue => this.open_queue(window, cx),
                Click::Reveal(path) => cx.reveal_path(path),
            }))
            .into_any_element()
    }

    fn render_footer(&self) -> impl IntoElement {
        let modifier = if cfg!(target_os = "macos") {
            "⌘"
        } else {
            "ctrl+"
        };
        let hint = |keys: String, label: &'static str| {
            div()
                .flex()
                .gap_1p5()
                .child(div().text_color(c(DIM)).child(keys))
                .child(label)
        };
        div()
            .flex()
            .items_center()
            .gap_4()
            .px_5()
            .py_2()
            .border_t_1()
            .border_color(c(LINE))
            .text_xs()
            .text_color(c(FAINT))
            .child(hint(format!("{modifier}l"), "focus"))
            .child(hint("enter".into(), "search"))
            .child(hint("tab".into(), "next"))
            .child(hint(format!("{modifier}q"), "quit"))
            .child(div().flex_1())
            .when(self.copied.is_some(), |el| {
                el.child(
                    div()
                        .text_color(c(ACCENT))
                        .child("copied to clipboard")
                        .with_animation(("copied-note", self.copy_seq), fade(), |el, t| {
                            el.opacity(t)
                        }),
                )
            })
            .when_some(self.note.clone(), |el, (text, error)| {
                el.child(
                    div()
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .text_color(if error { c(WARN) } else { c(ACCENT) })
                        .child(text)
                        .with_animation(("download-note", self.note_seq), fade(), |el, t| {
                            el.opacity(t)
                        }),
                )
            })
            .when(!self.files.is_empty(), |el| {
                el.child(format!("{}/{} links", self.ready(), self.files.len()))
            })
    }
}

impl Render for SearchApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let font = Theme::global(cx).mono_font_family.clone();
        div()
            .key_context("SearchApp")
            .size_full()
            .flex()
            .flex_col()
            .font_family(font)
            .bg(c(BG))
            .text_color(c(TEXT))
            .text_sm()
            .relative()
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                if this.modal.is_none() {
                    this.input.update(cx, |input, cx| input.focus(window, cx))
                }
            }))
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.modal.is_some() && event.keystroke.key == "escape" {
                    this.close_modal(window, cx);
                    cx.stop_propagation();
                }
            }))
            .child(self.render_header(cx))
            .child(self.render_command(cx))
            .child(self.render_activity())
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .gap_4()
                    .px_5()
                    .pt_4()
                    .pb_4()
                    .child(self.render_results(cx))
                    .child(self.render_detail(cx)),
            )
            .child(self.render_footer())
            .when_some(self.modal, |el, modal| {
                el.child(self.render_modal(modal, cx))
            })
    }
}

fn panel(title: &'static str, meta: String) -> Div {
    div()
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
                .px_3()
                .py_1p5()
                .border_b_1()
                .border_color(c(LINE))
                .text_xs()
                .child(div().text_color(c(DIM)).child(title))
                .child(div().flex_1())
                .child(div().text_color(c(FAINT)).child(meta)),
        )
}

/// Spinner plus a lowercase label, for in-flight work.
fn working(label: &'static str) -> AnyElement {
    div()
        .flex()
        .gap_2()
        .text_color(c(DIM))
        .child(div().w(px(8.)).text_color(c(ACCENT)).with_animation(
            SharedString::from(format!("spin-{label}")),
            Animation::new(Duration::from_millis(600)).repeat(),
            |el, t| el.child(spinner(t)),
        ))
        .child(label)
        .into_any_element()
}

pub(crate) fn note(text: &'static str) -> impl IntoElement {
    div().p_2().text_color(c(FAINT)).child(text)
}

pub(crate) fn fault(message: &str) -> impl IntoElement {
    div()
        .m_1()
        .px_3()
        .py_2()
        .border_l_2()
        .border_color(c(WARN))
        .bg(ca(WARN, 0.05))
        .text_color(c(WARN))
        .child(message.to_string())
}

fn skeleton() -> impl IntoElement {
    const WIDTHS: [f32; 6] = [0.7, 0.55, 0.8, 0.5, 0.65, 0.45];
    let mut rows = div().flex().flex_col();
    for width in WIDTHS {
        rows = rows.child(
            div()
                .flex()
                .gap_3()
                .items_center()
                .px_2()
                .py_2p5()
                .child(div().w(px(14.)).h(px(6.)).bg(c(LINE)))
                .child(div().w(relative(width)).h(px(6.)).bg(c(LINE_HI))),
        );
    }
    rows.with_animation(
        "skeleton",
        Animation::new(Duration::from_millis(1400))
            .repeat()
            .with_easing(pulsating_between(0.4, 0.9)),
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
