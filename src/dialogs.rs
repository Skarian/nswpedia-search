//! Settings and download-queue dialogs, drawn as an overlay on the main view.
use super::{Message, Modal, SearchApp, secrets};
use crate::style::*;
use crate::view::{fault, note};
use gpui_kit::component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::Input,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use nswpedia_search::{
    debrid::{TOKEN_PAGE, valid_token},
    downloads::{Job, Status},
    settings::{Settings, default_download_dir, parse_download_dir},
};
use std::time::Duration;

impl SearchApp {
    pub(crate) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.modal = Some(Modal::Settings);
        self.settings_error = None;
        self.settings_note = None;
        let dir = self.settings.download_dir().display().to_string();
        self.dir_input
            .update(cx, |input, cx| input.set_value(dir, window, cx));
        self.key_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    pub(crate) fn open_queue(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.modal = Some(Modal::Queue);
        self.modal_focus.focus(window, cx);
        cx.notify();
    }

    pub(crate) fn close_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.modal = None;
        self.verify_id += 1;
        self.verify_busy = false;
        // Do not keep a typed key around in the hidden input.
        self.key_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.input.update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }

    fn typed_key(&self, cx: &App) -> String {
        self.key_input.read(cx).value().trim().to_string()
    }

    fn settings_failed(&mut self, error: impl Into<String>, cx: &mut Context<Self>) {
        self.settings_error = Some(error.into());
        self.settings_note = None;
        cx.notify();
    }

    pub(crate) fn save_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.modal != Some(Modal::Settings) {
            return;
        }
        let key = self.typed_key(cx);
        let download_dir = match parse_download_dir(&self.dir_input.read(cx).value()) {
            Ok(dir) => dir,
            Err(error) => return self.settings_failed(error.to_string(), cx),
        };
        if !key.is_empty() {
            if !valid_token(&key) {
                return self.settings_failed(
                    "That does not look like a Real-Debrid API key (letters and digits only).",
                    cx,
                );
            }
            if let Err(error) = secrets::save(&key) {
                return self.settings_failed(error.to_string(), cx);
            }
            self.downloads.set_token(Some(key));
        }
        let settings = Settings { download_dir };
        if let Err(error) = settings.save(&self.settings_path) {
            return self.settings_failed(format!("Could not save settings: {error}"), cx);
        }
        self.settings = settings;
        self.close_modal(window, cx);
        self.set_note("settings saved", false, cx);
    }

    fn remove_key(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(error) = secrets::delete() {
            return self.settings_failed(error.to_string(), cx);
        }
        self.downloads.set_token(None);
        self.key_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.settings_error = None;
        self.settings_note = Some("api key removed".into());
        cx.notify();
    }

    fn verify_key(&mut self, cx: &mut Context<Self>) {
        let typed = self.typed_key(cx);
        let token = if typed.is_empty() {
            if !self.downloads.has_token() {
                return self.settings_failed("Paste your API key first.", cx);
            }
            None
        } else if valid_token(&typed) {
            Some(typed)
        } else {
            return self.settings_failed(
                "That does not look like a Real-Debrid API key (letters and digits only).",
                cx,
            );
        };
        self.verify_id += 1;
        self.verify_busy = true;
        self.settings_error = None;
        self.settings_note = None;
        let (downloads, sender, id) = (self.downloads.clone(), self.sender.clone(), self.verify_id);
        std::thread::spawn(move || {
            let result = downloads.check_account(token).map_err(|e| e.to_string());
            let _ = sender.send_blocking(Message::Verified(id, result));
        });
        cx.notify();
    }

    fn browse_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose download folder".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update_in(cx, |this, window, cx| {
                this.dir_input.update(cx, |input, cx| {
                    input.set_value(path.display().to_string(), window, cx)
                });
            });
        })
        .detach();
    }

    pub(crate) fn render_modal(&self, modal: Modal, cx: &mut Context<Self>) -> impl IntoElement {
        let (title, label, meta, body) = match modal {
            Modal::Settings => (
                "settings",
                "Settings",
                String::new(),
                self.render_settings(cx).into_any_element(),
            ),
            Modal::Queue => {
                let jobs = self.downloads.jobs();
                let active = jobs.iter().filter(|j| j.status.is_active()).count();
                let meta = if jobs.is_empty() {
                    String::new()
                } else {
                    format!("{active} active · {} total", jobs.len())
                };
                (
                    "downloads",
                    "Downloads",
                    meta,
                    self.render_queue(jobs, cx).into_any_element(),
                )
            }
        };
        div()
            .id("modal-backdrop")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .bg(ca(BG, 0.8))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .id("modal")
                    .role(Role::Dialog)
                    .aria_label(label)
                    .track_focus(&self.modal_focus)
                    .w(px(640.))
                    .max_w(relative(0.92))
                    .max_h(relative(0.88))
                    .flex()
                    .flex_col()
                    .bg(c(PANEL))
                    .border_1()
                    .border_color(c(LINE_HI))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .pl_5()
                            .pr_2()
                            .py_2()
                            .border_b_1()
                            .border_color(c(LINE))
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(c(TEXT))
                                    .child(title),
                            )
                            .child(div().text_xs().text_color(c(FAINT)).child(meta))
                            .child(div().flex_1())
                            .child(
                                Button::new("modal-close")
                                    .custom(ghost_button(cx, DIM))
                                    .small()
                                    .cursor_pointer()
                                    .accessibility_label(format!("Close {title}"))
                                    .label("esc ✕")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.close_modal(window, cx)
                                    })),
                            ),
                    )
                    .child(body)
                    .with_animation(("modal", modal as usize), fade(), |el, t| el.opacity(t)),
            )
    }

    fn render_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let saved = self.downloads.has_token();
        let key_status = if saved {
            format!("key saved in {}", secrets::STORE_NAME)
        } else {
            "no key saved".into()
        };
        let folder = self
            .dir_input
            .read(cx)
            .value()
            .trim()
            .trim_matches('"')
            .to_string();
        let folder = if folder.is_empty() {
            default_download_dir().display().to_string()
        } else {
            folder
        };
        let sep = std::path::MAIN_SEPARATOR;
        div()
            .id("settings-body")
            .flex()
            .flex_col()
            .gap_5()
            .p_5()
            .overflow_y_scroll()
            .child(
                section("real-debrid api key")
                    .child(
                        field().child(
                            Input::new(&self.key_input)
                                .appearance(false)
                                .mask_toggle()
                                .aria_label("Real-Debrid API key"),
                        ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_xs()
                            .text_color(if saved { c(DIM) } else { c(FAINT) })
                            .child(key_status)
                            .child(div().flex_1())
                            .child(
                                action(
                                    "get-key",
                                    "get key ↗",
                                    "Open the Real-Debrid API token page",
                                    DIM,
                                    cx,
                                )
                                .on_click(|_, _, cx| cx.open_url(TOKEN_PAGE)),
                            )
                            .child(
                                action(
                                    "verify-key",
                                    if self.verify_busy {
                                        "checking…"
                                    } else {
                                        "verify"
                                    },
                                    "Verify API key with Real-Debrid",
                                    DIM,
                                    cx,
                                )
                                .disabled(self.verify_busy)
                                .on_click(cx.listener(|this, _, _, cx| this.verify_key(cx))),
                            )
                            .when(saved, |el| {
                                el.child(
                                    action(
                                        "remove-key",
                                        "remove",
                                        "Remove saved API key",
                                        WARN,
                                        cx,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, window, cx| this.remove_key(window, cx),
                                    )),
                                )
                            }),
                    )
                    .child(div().text_xs().text_color(c(FAINT)).child(if saved {
                        "paste a new key to replace the saved one, or leave empty to keep it"
                    } else {
                        "paste the private api token from real-debrid.com/apitoken"
                    })),
            )
            .child(
                section("download folder")
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                field().flex_1().min_w_0().child(
                                    Input::new(&self.dir_input)
                                        .appearance(false)
                                        .aria_label("Download folder"),
                                ),
                            )
                            .child(
                                action(
                                    "browse-folder",
                                    "browse",
                                    "Choose download folder",
                                    DIM,
                                    cx,
                                )
                                .on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.browse_folder(window, cx)
                                    }),
                                ),
                            )
                            .child(
                                action(
                                    "default-folder",
                                    "default",
                                    "Use the Downloads folder",
                                    DIM,
                                    cx,
                                )
                                .on_click(cx.listener(
                                    |this, _, window, cx| {
                                        let dir = default_download_dir().display().to_string();
                                        this.dir_input.update(cx, |input, cx| {
                                            input.set_value(dir, window, cx)
                                        });
                                    },
                                )),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(c(FAINT))
                            .child(format!("files save to {folder}{sep}<result title>{sep}")),
                    ),
            )
            .when_some(self.settings_error.clone(), |el, error| {
                el.child(fault(&error))
            })
            .when_some(self.settings_note.clone(), |el, text| {
                el.child(div().text_xs().text_color(c(ACCENT)).child(text))
            })
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        action("settings-cancel", "cancel", "Cancel", DIM, cx).on_click(
                            cx.listener(|this, _, window, cx| this.close_modal(window, cx)),
                        ),
                    )
                    .child(
                        Button::new("settings-save")
                            .custom(outline_button(cx, ACCENT))
                            .small()
                            .border_1()
                            .border_color(ca(ACCENT, 0.4))
                            .cursor_pointer()
                            .accessibility_label("Save settings")
                            .label("save")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.save_settings(window, cx)),
                            ),
                    ),
            )
    }

    fn render_queue(&self, jobs: Vec<Job>, cx: &mut Context<Self>) -> impl IntoElement {
        let finished = jobs.iter().any(|j| !j.status.is_active());
        let mut list = div()
            .id("queue")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px_5();
        if jobs.is_empty() {
            list = list.child(div().py_4().child(note(
                "no downloads yet — press download next to a ready link",
            )));
        }
        for job in jobs {
            list = list.child(self.render_job(job, cx));
        }
        let root = self.settings.download_dir();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(list)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_5()
                    .py_3()
                    .border_t_1()
                    .border_color(c(LINE))
                    .child(
                        action("open-root", "open folder", "Open download folder", DIM, cx)
                            .on_click(move |_, _, cx| {
                                if root.is_dir() {
                                    cx.open_with_system(&root)
                                }
                            }),
                    )
                    .child(div().flex_1())
                    .child(
                        action(
                            "clear-finished",
                            "clear finished",
                            "Clear finished downloads",
                            DIM,
                            cx,
                        )
                        .disabled(!finished)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.downloads.clear_finished();
                            cx.notify();
                        })),
                    ),
            )
    }

    fn render_job(&self, job: Job, cx: &mut Context<Self>) -> impl IntoElement {
        let id = job.id;
        let mark = match &job.status {
            Status::Done => div().text_color(c(ACCENT)).child("✓").into_any_element(),
            Status::Failed(_) => div().text_color(c(WARN)).child("!").into_any_element(),
            Status::Cancelled => div().text_color(c(FAINT)).child("×").into_any_element(),
            Status::Queued => div().text_color(c(FAINT)).child("·").into_any_element(),
            Status::Resolving | Status::Downloading => div()
                .text_color(c(ACCENT))
                .with_animation(
                    ("job-spin", id as usize),
                    Animation::new(Duration::from_millis(600)).repeat_synced(),
                    |el, t| el.child(spinner(t)),
                )
                .into_any_element(),
        };
        let progress = match job.total {
            Some(total) if total > 0 => Some((job.received as f32 / total as f32).min(1.)),
            _ => None,
        };
        let status = match &job.status {
            Status::Queued => "queued".to_string(),
            Status::Resolving => "unrestricting via real-debrid…".into(),
            Status::Downloading => {
                let mut text = match job.total {
                    Some(total) => format!("{} / {}", bytes(job.received), bytes(total)),
                    None => bytes(job.received),
                };
                if let Some(ratio) = progress {
                    text.push_str(&format!(" · {:.0}%", ratio * 100.));
                }
                if job.rate > 0. {
                    text.push_str(&format!(" · {}/s", bytes(job.rate as u64)));
                }
                text
            }
            Status::Done => format!("saved · {}", bytes(job.received)),
            Status::Failed(_) => "failed".into(),
            Status::Cancelled => "cancelled".into(),
        };
        let mut info = div()
            .flex_1()
            .min_w_0()
            .child(
                div()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(job.name.clone()),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .mt_0p5()
                    .text_xs()
                    .text_color(c(DIM))
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(job.title.clone()),
                    )
                    .child(div().text_color(c(FAINT)).child("·"))
                    .child(div().flex_shrink_0().child(status)),
            );
        if job.status == Status::Downloading {
            info = info.child(
                div().mt_1p5().h(px(2.)).w_full().bg(c(LINE)).child(
                    div()
                        .h_full()
                        .w(relative(progress.unwrap_or(0.)))
                        .bg(ca(ACCENT, 0.7)),
                ),
            );
        }
        if let Status::Failed(error) = &job.status {
            info = info.child(
                div()
                    .text_xs()
                    .mt_1()
                    .text_color(c(WARN))
                    .child(error.clone()),
            );
        }
        if let (Status::Done, Some(path)) = (&job.status, &job.path) {
            info = info.child(
                div()
                    .text_xs()
                    .mt_1()
                    .text_color(c(FAINT))
                    .child(path.display().to_string()),
            );
        }
        let button = match &job.status {
            status if status.is_active() => Some(
                action(
                    ("job-cancel", id as usize),
                    "cancel",
                    format!("Cancel download of {}", job.name),
                    DIM,
                    cx,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.downloads.cancel(id);
                    cx.notify();
                })),
            ),
            Status::Failed(_) | Status::Cancelled => Some(
                action(
                    ("job-retry", id as usize),
                    "retry",
                    format!("Retry download of {}", job.name),
                    WARN,
                    cx,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if !this.downloads.has_token() {
                        this.set_note(
                            "Add your Real-Debrid API key in settings to download.",
                            true,
                            cx,
                        );
                    }
                    this.downloads.retry(id);
                    cx.notify();
                })),
            ),
            Status::Done => job.path.clone().filter(|p| p.exists()).map(|path| {
                action(
                    ("job-show", id as usize),
                    "show",
                    format!("Show {} in folder", job.name),
                    DIM,
                    cx,
                )
                .on_click(move |_, _, cx| cx.reveal_path(&path))
            }),
            _ => None,
        };
        div()
            .flex()
            .items_start()
            .gap_3()
            .py_3()
            .border_b_1()
            .border_color(c(LINE))
            .child(div().flex_shrink_0().w(px(12.)).child(mark))
            .child(info)
            .when_some(button, |el, button| el.child(button))
    }
}

/// Small bordered text button used inside dialogs.
pub(crate) fn action(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    name: impl Into<SharedString>,
    hex: u32,
    cx: &App,
) -> Button {
    Button::new(id)
        .custom(ghost_button(cx, hex))
        .small()
        .flex_shrink_0()
        .border_1()
        .border_color(c(LINE_HI))
        .cursor_pointer()
        .accessibility_label(name)
        .label(label)
}

fn section(title: &'static str) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_xs().text_color(c(DIM)).child(title))
}

fn field() -> Div {
    div()
        .px_2()
        .py_0p5()
        .bg(c(BG))
        .border_1()
        .border_color(c(LINE_HI))
}

pub(crate) fn bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1024. && unit < UNITS.len() - 1 {
        value /= 1024.;
        unit += 1;
    }
    if unit == 0 {
        format!("{n} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    #[::core::prelude::v1::test]
    fn formats_byte_counts() {
        assert_eq!(super::bytes(512), "512 B");
        assert_eq!(super::bytes(1536), "1.5 KB");
        assert_eq!(super::bytes(3 * 1024 * 1024 * 1024), "3.0 GB");
    }
}
