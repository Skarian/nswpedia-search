#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod style;
mod view;

use gpui_kit::component::{
    Theme, ThemeMode,
    input::{InputEvent, InputState},
};
use gpui_kit::*;
use nswpedia_search::resolver::{
    BATCH_SIZE, Cancel, FileEntry, Game, ResolveEvent, Resolver, SearchPage,
};
use std::{collections::HashMap, sync::Arc};
use style::{ACCENT, BG, DIM, LINE, LINE_HI, PANEL, TEXT, c, ca};

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
    copy_seq: u64,
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
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("type a game title…"));
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
            copy_seq: 0,
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
        self.copy_seq += 1;
        cx.notify();
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
        let theme = Theme::global_mut(cx);
        theme.font_family = theme.mono_font_family.clone();
        theme.radius = px(0.);
        theme.radius_lg = px(0.);
        theme.shadow = false;
        let colors = &mut theme.colors;
        colors.background = c(BG);
        colors.foreground = c(TEXT);
        colors.muted_foreground = c(DIM);
        colors.border = c(LINE);
        colors.input = c(LINE_HI);
        colors.caret = c(ACCENT);
        colors.selection = ca(ACCENT, 0.25);
        colors.primary = c(ACCENT);
        colors.primary_hover = ca(ACCENT, 0.85);
        colors.primary_active = ca(ACCENT, 0.7);
        colors.primary_foreground = c(BG);
        colors.ring = c(LINE_HI);
        colors.accent = ca(TEXT, 0.06);
        colors.accent_foreground = c(TEXT);
        colors.popover = c(PANEL);
        colors.popover_foreground = c(TEXT);
        colors.scrollbar = ca(BG, 0.);
        colors.scrollbar_thumb = c(LINE_HI);
        colors.scrollbar_thumb_hover = ca(DIM, 0.6);
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
                size(px(1180.), px(760.)),
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
