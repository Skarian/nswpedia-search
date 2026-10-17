#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
use gpui_kit::component::{
    Disableable, Sizable, Theme, ThemeMode,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
};
use gpui_kit::*;
use nswpedia_search::resolver::{
    BATCH_SIZE, Cancel, FileEntry, Game, ResolveEvent, Resolver, SearchPage,
};
use std::{collections::HashMap, sync::Arc};

gpui_kit::actions!(search_app, [FocusSearch, Quit]);

enum Message {
    Search(u64, Result<SearchPage, String>),
    Resolve(u64, ResolveEvent),
    Failed(u64, String),
}
struct FileRow {
    file: FileEntry,
    url: Option<String>,
    error: Option<String>,
}

fn failed_batch(files: &[FileRow]) -> Option<usize> {
    files
        .iter()
        .position(|row| row.error.is_some())
        .map(|index| index / BATCH_SIZE * BATCH_SIZE)
}

#[cfg(test)]
mod tests {
    use super::{BATCH_SIZE, FileEntry, FileRow, failed_batch};

    #[::core::prelude::v1::test]
    fn retries_earlier_failures_after_later_batches_finish() {
        let mut rows: Vec<_> = (0..BATCH_SIZE * 3)
            .map(|index| FileRow {
                file: FileEntry {
                    key: index.to_string(),
                    name: String::new(),
                    size: String::new(),
                    format: String::new(),
                    path: String::new(),
                },
                url: Some("https://1fichier.com/?abcdefghijkl".into()),
                error: None,
            })
            .collect();
        rows[1].url = None;
        rows[1].error = Some("Temporary failure".into());
        // Later batches have succeeded, but the first batch still needs Retry.
        assert_eq!(failed_batch(&rows), Some(0));
        rows[BATCH_SIZE + 2].url = None;
        rows[BATCH_SIZE + 2].error = Some("Another failure".into());
        assert_eq!(failed_batch(&rows), Some(0));
        rows[1].error = None;
        assert_eq!(failed_batch(&rows), Some(BATCH_SIZE));
        rows[BATCH_SIZE + 2].error = None;
        assert_eq!(failed_batch(&rows), None);
    }
}
struct SearchApp {
    input: Entity<InputState>,
    resolver: Arc<Resolver>,
    sender: async_channel::Sender<Message>,
    _events: Task<()>,
    _subscriptions: Vec<Subscription>,
    search_cancel: Cancel,
    resolve_cancel: Cancel,
    search_id: u64,
    resolve_id: u64,
    query: String,
    games: Vec<Game>,
    selected: Option<usize>,
    files: Vec<FileRow>,
    search_busy: bool,
    resolve_busy: bool,
    searched: bool,
    page: u32,
    total_pages: u32,
    next_offset: Option<usize>,
    batch_offset: usize,
    search_error: Option<String>,
    resolve_error: Option<String>,
    copied: Option<String>,
}
impl Drop for SearchApp {
    fn drop(&mut self) {
        self.search_cancel.cancel();
        self.resolve_cancel.cancel();
    }
}
impl SearchApp {
    fn new(
        resolver: Arc<Resolver>,
        initial: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search games"));
        let subscription = cx.subscribe_in(&input, window, |this, _, event, _, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                this.search(false, cx);
            }
        });
        input.update(cx, |input, cx| input.focus(window, cx));
        let (sender, receiver) = async_channel::bounded(96);
        let events = cx.spawn(async move |this, cx| {
            while let Ok(message) = receiver.recv().await {
                if this
                    .update(cx, |this, cx| this.receive(message, cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        let mut this = Self {
            input,
            resolver,
            sender,
            _events: events,
            _subscriptions: vec![subscription],
            search_cancel: Cancel::default(),
            resolve_cancel: Cancel::default(),
            search_id: 0,
            resolve_id: 0,
            query: String::new(),
            games: vec![],
            selected: None,
            files: vec![],
            search_busy: false,
            resolve_busy: false,
            searched: false,
            page: 0,
            total_pages: 0,
            next_offset: None,
            batch_offset: 0,
            search_error: None,
            resolve_error: None,
            copied: None,
        };
        if let Some(initial) = initial {
            this.input
                .update(cx, |input, cx| input.set_value(initial, window, cx));
            this.search(false, cx);
        }
        this
    }
    fn search(&mut self, more: bool, cx: &mut Context<Self>) {
        let query = if more {
            self.query.clone()
        } else {
            self.input.read(cx).value().trim().to_string()
        };
        if !(2..=120).contains(&query.chars().count()) {
            self.search_error = Some("Enter 2–120 characters".into());
            cx.notify();
            return;
        }
        self.search_cancel.cancel();
        self.search_cancel = Cancel::default();
        self.search_id += 1;
        self.search_busy = true;
        self.search_error = None;
        self.searched = true;
        let page = if more { self.page + 1 } else { 1 };
        if !more {
            self.resolve_cancel.cancel();
            self.resolve_id += 1;
            self.resolve_busy = false;
            self.query = query.clone();
            self.games.clear();
            self.files.clear();
            self.selected = None;
            self.resolve_error = None;
            self.next_offset = None;
            self.page = 0;
            self.total_pages = 0;
            self.copied = None;
        }
        let (resolver, cancel, sender, id) = (
            self.resolver.clone(),
            self.search_cancel.clone(),
            self.sender.clone(),
            self.search_id,
        );
        std::thread::spawn(move || {
            let result = resolver
                .search(&query, page, &cancel)
                .map_err(|e| e.to_string());
            if !cancel.is_cancelled() {
                let _ = sender.send_blocking(Message::Search(id, result));
            }
        });
        cx.notify();
    }
    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected = Some(index);
        self.files.clear();
        self.copied = None;
        self.resolve(0, cx);
    }
    fn resolve(&mut self, offset: usize, cx: &mut Context<Self>) {
        let Some(game) = self.selected.and_then(|i| self.games.get(i)).cloned() else {
            return;
        };
        self.resolve_cancel.cancel();
        self.resolve_cancel = Cancel::default();
        self.resolve_id += 1;
        self.resolve_busy = true;
        self.resolve_error = None;
        self.next_offset = None;
        self.batch_offset = offset;
        for row in self.files.iter_mut().skip(offset).take(BATCH_SIZE) {
            row.error = None;
        }
        let (resolver, cancel, sender, id) = (
            self.resolver.clone(),
            self.resolve_cancel.clone(),
            self.sender.clone(),
            self.resolve_id,
        );
        std::thread::spawn(move || {
            let result = resolver.resolve(&game, offset, &cancel, &|event| {
                if !cancel.is_cancelled() {
                    let _ = sender.send_blocking(Message::Resolve(id, event));
                }
            });
            if let Err(error) = result {
                if !cancel.is_cancelled() {
                    let _ = sender.send_blocking(Message::Failed(id, error.to_string()));
                }
            }
        });
        cx.notify();
    }
    fn receive(&mut self, message: Message, cx: &mut Context<Self>) {
        match message {
            Message::Search(id, result) if id == self.search_id => {
                self.search_busy = false;
                match result {
                    Ok(page) => {
                        self.page = page.page;
                        self.total_pages = page.total_pages;
                        for game in page.games {
                            if !self.games.iter().any(|g| g.id == game.id) {
                                self.games.push(game);
                            }
                        }
                        if self.selected.is_none() && !self.games.is_empty() {
                            self.select(0, cx);
                        }
                    }
                    Err(error) => self.search_error = Some(error),
                }
            }
            Message::Resolve(id, event) if id == self.resolve_id => match event {
                ResolveEvent::Files(files) => {
                    let mut old: HashMap<_, _> = self
                        .files
                        .drain(..)
                        .map(|r| (r.file.key.clone(), r))
                        .collect();
                    self.files = files
                        .into_iter()
                        .map(|file| {
                            old.remove(&file.key).unwrap_or(FileRow {
                                file,
                                url: None,
                                error: None,
                            })
                        })
                        .collect();
                }
                ResolveEvent::Link { key, url } => {
                    if let Some(row) = self.files.iter_mut().find(|r| r.file.key == key) {
                        row.url = Some(url);
                        row.error = None;
                    }
                }
                ResolveEvent::Error { key, message } => {
                    if let Some(row) = self.files.iter_mut().find(|r| r.file.key == key) {
                        row.error = Some(message);
                    }
                }
                ResolveEvent::Done {
                    next_offset,
                    failures: _,
                } => {
                    self.resolve_busy = false;
                    self.next_offset = next_offset;
                    if failed_batch(&self.files).is_some() {
                        self.resolve_error = Some("Some links could not be resolved.".into());
                    }
                }
            },
            Message::Failed(id, error) if id == self.resolve_id => {
                self.resolve_busy = false;
                self.resolve_error = Some(error);
            }
            _ => return,
        }
        cx.notify();
    }
    fn copy(&mut self, key: Option<&str>, cx: &mut Context<Self>) {
        let urls: Vec<_> = self
            .files
            .iter()
            .filter(|r| key.is_none_or(|k| k == r.file.key))
            .filter_map(|r| r.url.clone())
            .collect();
        if urls.is_empty() {
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(urls.join("\n")));
        self.copied = Some(key.unwrap_or("all").to_string());
        cx.notify();
    }
}
impl Render for SearchApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut results = div()
            .id("results")
            .w(px(286.))
            .flex_shrink_0()
            .h_full()
            .overflow_y_scroll()
            .p_3()
            .border_r_1()
            .border_color(rgb(0x2a373b))
            .bg(rgb(0x131d20));
        if self.search_busy {
            results = results.child(div().p_2().text_color(rgb(0x9caeac)).child("Searching…"));
        }
        if let Some(error) = &self.search_error {
            results = results.child(div().p_2().text_color(rgb(0xefb786)).child(error.clone()));
        }
        if self.searched
            && !self.search_busy
            && self.games.is_empty()
            && self.search_error.is_none()
        {
            results = results.child(div().p_2().child("No results"));
        }
        for (index, game) in self.games.iter().enumerate() {
            let url = game.source_url.clone();
            results = results.child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .mb_1()
                    .child(
                        Button::new(("game", index))
                            .ghost()
                            .flex_1()
                            .min_w_0()
                            .h_auto()
                            .py_3()
                            .bg(rgb(if self.selected == Some(index) {
                                0x27352b
                            } else {
                                0x131d20
                            }))
                            .label(game.title.clone())
                            .on_click(cx.listener(move |this, _, _, cx| this.select(index, cx))),
                    )
                    .child(
                        Button::new(("source", index))
                            .ghost()
                            .small()
                            .label("↗")
                            .tooltip("Open source page")
                            .on_click(move |_, _, cx| cx.open_url(&url)),
                    ),
            );
        }
        if self.page < self.total_pages && self.page < 100 {
            results = results.child(
                Button::new("more-results")
                    .ghost()
                    .label("More results")
                    .disabled(self.search_busy)
                    .on_click(cx.listener(|this, _, _, cx| this.search(true, cx))),
            );
        }
        let mut detail = div()
            .id("files")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .p_6();
        if let Some(game) = self.selected.and_then(|i| self.games.get(i)) {
            let url = game.source_url.clone();
            detail = detail.child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .mb_5()
                    .child(div().flex_1().text_xl().child(game.title.clone()))
                    .child(
                        Button::new("source-page")
                            .ghost()
                            .small()
                            .label("Source page ↗")
                            .on_click(move |_, _, cx| cx.open_url(&url)),
                    ),
            );
            if self.files.iter().filter(|r| r.url.is_some()).count() > 1 {
                detail = detail.child(
                    div().flex().justify_end().mb_2().child(
                        Button::new("copy-all")
                            .small()
                            .label(if self.copied.as_deref() == Some("all") {
                                "Copied"
                            } else {
                                "Copy all"
                            })
                            .on_click(cx.listener(|this, _, _, cx| this.copy(None, cx))),
                    ),
                );
            }
            if self.files.is_empty() {
                let status = if self.resolve_busy {
                    "Loading files…"
                } else if self.resolve_error.is_none() {
                    "No 1fichier links listed"
                } else {
                    ""
                };
                detail = detail.child(div().text_color(rgb(0x9caeac)).child(status));
            }
            for (index, row) in self.files.iter().enumerate() {
                let key = row.file.key.clone();
                let mut info = div().flex_1().min_w_0().child(row.file.name.clone()).child(
                    div()
                        .text_xs()
                        .mt_1()
                        .text_color(rgb(0x9caeac))
                        .child(format!("{}  {}", row.file.size, row.file.format)),
                );
                if let Some(url) = &row.url {
                    info = info.child(
                        div()
                            .text_xs()
                            .mt_2()
                            .text_color(rgb(0xc0dca0))
                            .child(url.clone()),
                    );
                } else if let Some(error) = &row.error {
                    info = info.child(
                        div()
                            .text_xs()
                            .text_color(rgb(0xefb786))
                            .child(error.clone()),
                    );
                } else {
                    info = info.child(div().text_xs().text_color(rgb(0x9caeac)).child(
                        if self.resolve_busy
                            && (self.batch_offset..self.batch_offset + BATCH_SIZE).contains(&index)
                        {
                            "Resolving…"
                        } else {
                            "Pending"
                        },
                    ));
                }
                let mut line = div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .py_4()
                    .border_t_1()
                    .border_color(rgb(0x2a373b))
                    .child(info);
                if row.url.is_some() {
                    line = line.child(
                        Button::new(("copy", index))
                            .small()
                            .label(if self.copied.as_deref() == Some(&key) {
                                "Copied"
                            } else {
                                "Copy"
                            })
                            .on_click(cx.listener(move |this, _, _, cx| this.copy(Some(&key), cx))),
                    );
                }
                detail = detail.child(line);
            }
            if let Some(error) = &self.resolve_error {
                detail = detail
                    .child(div().text_color(rgb(0xefb786)).my_3().child(error.clone()))
                    .child(Button::new("retry").label("Retry").on_click(cx.listener(
                        |this, _, _, cx| {
                            let offset = failed_batch(&this.files).unwrap_or(this.batch_offset);
                            this.resolve(offset, cx);
                        },
                    )));
            }
            if let Some(offset) = self.next_offset {
                detail = detail.child(
                    Button::new("more-files")
                        .label("Resolve more")
                        .on_click(cx.listener(move |this, _, _, cx| this.resolve(offset, cx))),
                );
            }
        }
        div()
            .key_context("SearchApp")
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x101719))
            .text_color(rgb(0xedf4ed))
            .text_sm()
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                this.input.update(cx, |input, cx| input.focus(window, cx))
            }))
            .child(
                div()
                    .p_6()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(div().flex_1().child(Input::new(&self.input)))
                    .child(
                        Button::new("search")
                            .primary()
                            .label("Search")
                            .on_click(cx.listener(|this, _, _, cx| this.search(false, cx))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .border_t_1()
                    .border_color(rgb(0x2a373b))
                    .bg(rgb(0x172023))
                    .child(results)
                    .child(detail),
            )
    }
}
fn main() -> anyhow::Result<()> {
    let dirs = directories::ProjectDirs::from("com", "NSWpedia", "NSWpedia Search")
        .ok_or_else(|| anyhow::anyhow!("No application cache directory"))?;
    let resolver = Arc::new(Resolver::new(&dirs.cache_dir().join("cache.sqlite"))?);
    let args: Vec<_> = std::env::args().collect();
    let initial = args
        .iter()
        .position(|v| v == "--search")
        .and_then(|i| args.get(i + 1))
        .cloned();
    application().with_assets(assets::Assets).run(move |cx| {
        init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
        let colors = &mut Theme::global_mut(cx).colors;
        colors.primary = rgb(0xd6ef91).into();
        colors.primary_hover = rgb(0xe1f5b0).into();
        colors.primary_active = rgb(0xc0dca0).into();
        colors.primary_foreground = rgb(0x273221).into();
        colors.ring = rgb(0xd6ef91).into();
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([
            KeyBinding::new("secondary-l", FocusSearch, Some("SearchApp")),
            KeyBinding::new("secondary-q", Quit, None),
        ]);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1100.), px(720.)),
                cx,
            ))),
            window_min_size: Some(size(px(760.), px(420.))),
            ..Default::default()
        };
        open_window(options, cx, |window, cx| {
            window.set_window_title("NSWpedia Search");
            cx.new(|cx| SearchApp::new(resolver, initial, window, cx))
        })
        .expect("Could not open NSWpedia Search");
        cx.activate(true);
    });
    Ok(())
}
