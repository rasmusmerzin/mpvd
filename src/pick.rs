use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::{Frame, Terminal};

use crate::config;
use crate::control;
use crate::daemon;
use crate::find;
use crate::list::ListView;
use crate::term::{term_alternate_raw, term_restore};

struct Picker {
    view: ListView,
    files: Vec<PathBuf>,
    original: Vec<PathBuf>,
    filtered: Vec<usize>,
    to_push: Vec<usize>,
    to_insert: Vec<usize>,
    search: String,
    search_cursor: usize,
    search_mode: bool,
    absolute: bool,
    shuffled: bool,
}

impl Picker {
    fn new(files: Vec<PathBuf>) -> Self {
        let len = files.len();
        Self {
            view: ListView::new(len),
            original: files.clone(),
            filtered: (0..len).collect(),
            files,
            to_push: Vec::new(),
            to_insert: Vec::new(),
            search: String::new(),
            search_cursor: 0,
            search_mode: false,
            absolute: false,
            shuffled: false,
        }
    }

    fn update_filter(&mut self) {
        self.filtered = if self.search.is_empty() {
            (0..self.files.len()).collect()
        } else {
            match regex::Regex::new(&self.search) {
                Ok(re) => self
                    .files
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| {
                        p.file_name()
                            .and_then(|n| n.to_str())
                            .is_some_and(|n| re.is_match(n))
                    })
                    .map(|(i, _)| i)
                    .collect(),
                Err(_) => (0..self.files.len()).collect(),
            }
        };
        self.view.count = self.filtered.len();
        self.view.clamp_scroll();
    }

    fn toggle_push(&mut self) {
        if self.view.cursor >= self.filtered.len() {
            return;
        }
        let idx = self.filtered[self.view.cursor];
        if let Some(pos) = self.to_insert.iter().position(|&i| i == idx) {
            self.to_insert.remove(pos);
            if !self.to_push.contains(&idx) {
                self.to_push.push(idx);
            }
        } else if let Some(pos) = self.to_push.iter().position(|&i| i == idx) {
            self.to_push.remove(pos);
        } else {
            self.to_push.push(idx);
        }
    }

    fn toggle_insert(&mut self) {
        if self.view.cursor >= self.filtered.len() {
            return;
        }
        let idx = self.filtered[self.view.cursor];
        if let Some(pos) = self.to_push.iter().position(|&i| i == idx) {
            self.to_push.remove(pos);
        }
        if let Some(pos) = self.to_insert.iter().position(|&i| i == idx) {
            self.to_insert.remove(pos);
        } else {
            self.to_insert.push(idx);
        }
    }

    fn toggle_all(&mut self) {
        let all_selected = self
            .filtered
            .iter()
            .all(|&idx| self.to_push.contains(&idx) || self.to_insert.contains(&idx));
        if all_selected {
            self.to_push.retain(|idx| !self.filtered.contains(idx));
            self.to_insert.retain(|idx| !self.filtered.contains(idx));
        } else {
            for &idx in &self.filtered {
                if !self.to_push.contains(&idx) && !self.to_insert.contains(&idx) {
                    self.to_push.push(idx);
                }
            }
        }
    }

    fn shuffle(&mut self) {
        use rand::seq::SliceRandom;
        self.files = self.original.clone();
        if self.shuffled {
            self.shuffled = false;
        } else {
            self.files.shuffle(&mut rand::rng());
            self.shuffled = true;
        }
        self.update_filter();
    }

    fn search_len(&self) -> usize {
        self.search.chars().count()
    }

    fn search_word_start(&self, pos: usize) -> usize {
        let chars: Vec<char> = self.search.chars().collect();
        let mut i = pos.min(chars.len());
        while i > 0 && chars[i - 1].is_whitespace() {
            i -= 1;
        }
        while i > 0 && !chars[i - 1].is_whitespace() {
            i -= 1;
        }
        i
    }

    fn search_word_end(&self, pos: usize) -> usize {
        let chars: Vec<char> = self.search.chars().collect();
        let mut i = pos;
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        while i < chars.len() && !chars[i].is_whitespace() {
            i += 1;
        }
        i
    }

    fn edit_search(&mut self, edit: impl FnOnce(&mut Vec<char>, usize)) {
        let mut chars: Vec<char> = self.search.chars().collect();
        edit(&mut chars, self.search_cursor);
        self.search_cursor = self.search_cursor.min(chars.len());
        self.search = chars.into_iter().collect();
    }

    fn render(&self, f: &mut Frame) {
        let area = f.area();

        self.render_items(f, area);

        if self.filtered.is_empty() {
            let empty_msg = Line::from(Span::styled(
                "No matches.",
                Style::default().add_modifier(Modifier::ITALIC).dim(),
            ));
            f.render_widget(
                empty_msg,
                Rect::new(0, 0, area.width, self.view.height as u16),
            );
        }

        self.render_search(f, area);
    }

    fn render_items(&self, f: &mut Frame, area: Rect) {
        let items: Vec<Line> = self.filtered[self.view.offset..]
            .iter()
            .take(self.view.height)
            .enumerate()
            .map(|(i, &file_idx)| {
                let file = &self.files[file_idx];
                let is_hover = i + self.view.offset == self.view.cursor;
                let is_push = self.to_push.contains(&file_idx);
                let is_insert = self.to_insert.contains(&file_idx);

                let prefix = if is_insert {
                    "i "
                } else if is_push {
                    "* "
                } else {
                    "  "
                };

                let name = if self.absolute {
                    file.to_string_lossy().to_string()
                } else {
                    file.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| file.to_string_lossy().to_string())
                };

                let text = format!("{prefix}{name}");
                let text = text.chars().take(area.width as usize).collect::<String>();
                let text = format!("{text:<width$}", width = area.width as usize);

                let style = if is_hover {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };

                Line::from(Span::styled(text, style))
            })
            .collect();

        f.render_widget(
            Paragraph::new(items),
            Rect::new(0, 0, area.width, self.view.height as u16),
        );
    }

    fn render_search(&self, f: &mut Frame, area: Rect) {
        if self.search_mode {
            let before: String = self.search.chars().take(self.search_cursor).collect();
            let at_cursor: String = self
                .search
                .chars()
                .skip(self.search_cursor)
                .take(1)
                .collect();
            let after: String = self.search.chars().skip(self.search_cursor + 1).collect();
            let mut spans = vec![Span::raw(format!("/{before}"))];
            if at_cursor.is_empty() {
                spans.push(Span::styled(
                    " ",
                    Style::default().add_modifier(Modifier::REVERSED),
                ));
            } else {
                spans.push(Span::styled(
                    at_cursor,
                    Style::default().add_modifier(Modifier::REVERSED),
                ));
                spans.push(Span::raw(after));
            }
            let search_line = Line::from(spans);
            f.render_widget(
                search_line,
                Rect::new(0, self.view.height as u16, area.width, 1),
            );
        } else if !self.search.is_empty() {
            let search_line = Line::from(Span::raw(format!("/{}", self.search)));
            f.render_widget(
                search_line,
                Rect::new(0, self.view.height as u16, area.width, 1),
            );
        }
    }

    fn handle_main_input(&mut self, key: KeyEvent) -> Option<bool> {
        let has_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return Some(false),
            KeyCode::Char('c') if has_ctrl => return Some(false),
            KeyCode::Char('e') if has_ctrl => self.view.scroll_down(1),
            KeyCode::Char('y') if has_ctrl => self.view.scroll_up(1),
            KeyCode::Char('d') if has_ctrl => self.view.page_down(),
            KeyCode::Char('u') if has_ctrl => self.view.page_up(),
            KeyCode::Char('H') => self.view.cursor_home(),
            KeyCode::Char('L') => self.view.cursor_end(),
            KeyCode::Down | KeyCode::Char('j') => self.view.cursor_down(),
            KeyCode::Char('n') if has_ctrl => self.view.cursor_down(),
            KeyCode::Up | KeyCode::Char('k') => self.view.cursor_up(),
            KeyCode::Char('p') if has_ctrl => self.view.cursor_up(),
            KeyCode::Char('g') => self.view.go_top(),
            KeyCode::Char('G') => self.view.go_bottom(),
            KeyCode::Char('f') => self.absolute = !self.absolute,
            KeyCode::Char('a') if has_ctrl => self.toggle_all(),
            KeyCode::Char('r') => self.shuffle(),
            KeyCode::Char(' ') | KeyCode::Tab => self.toggle_push(),
            KeyCode::Char('i') => self.toggle_insert(),
            KeyCode::Enter => return Some(true),
            KeyCode::Char('/') => self.search_mode = true,
            _ => {}
        }
        None
    }

    fn cancel_search(&mut self) {
        self.search.clear();
        self.search_cursor = 0;
        self.search_mode = false;
        self.update_filter();
    }

    fn handle_search_input(&mut self, key: KeyEvent) -> Option<bool> {
        let has_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        match key.code {
            KeyCode::Enter => {
                self.search_mode = false;
                self.update_filter();
            }
            KeyCode::Esc => self.cancel_search(),
            KeyCode::Char('c') if has_ctrl => self.cancel_search(),
            KeyCode::Home => self.search_cursor = 0,
            KeyCode::Char('a') if has_ctrl => self.search_cursor = 0,
            KeyCode::End => self.search_cursor = self.search_len(),
            KeyCode::Char('e') if has_ctrl => self.search_cursor = self.search_len(),
            KeyCode::Char('w') if has_ctrl => {
                let target = self.search_word_start(self.search_cursor);
                self.edit_search(|chars, cur| {
                    chars.drain(target..cur);
                });
                self.search_cursor = target;
                self.update_filter();
            }
            KeyCode::Left if has_ctrl => {
                self.search_cursor = self.search_word_start(self.search_cursor);
            }
            KeyCode::Right if has_ctrl => {
                self.search_cursor = self.search_word_end(self.search_cursor);
            }
            KeyCode::Left => self.search_cursor = self.search_cursor.saturating_sub(1),
            KeyCode::Char('b') if has_ctrl => {
                self.search_cursor = self.search_cursor.saturating_sub(1);
            }
            KeyCode::Right => {
                if self.search_cursor < self.search_len() {
                    self.search_cursor += 1;
                }
            }
            KeyCode::Char('f') if has_ctrl => {
                if self.search_cursor < self.search_len() {
                    self.search_cursor += 1;
                }
            }
            KeyCode::Char('u') if has_ctrl => {
                self.edit_search(|chars, cur| {
                    chars.drain(..cur);
                });
                self.search_cursor = 0;
                self.update_filter();
            }
            KeyCode::Char('k') if has_ctrl => {
                self.edit_search(|chars, cur| chars.truncate(cur));
                self.update_filter();
            }
            KeyCode::Backspace => {
                if self.search_cursor > 0 {
                    self.search_cursor -= 1;
                    self.edit_search(|chars, cur| {
                        chars.remove(cur);
                    });
                    self.update_filter();
                }
            }
            KeyCode::Char('h') if has_ctrl => {
                if self.search_cursor > 0 {
                    self.search_cursor -= 1;
                    self.edit_search(|chars, cur| {
                        chars.remove(cur);
                    });
                    self.update_filter();
                }
            }
            KeyCode::Delete => {
                self.edit_search(|chars, cur| {
                    if cur < chars.len() {
                        chars.remove(cur);
                    }
                });
                self.update_filter();
            }
            KeyCode::Char('d') if has_ctrl => {
                self.edit_search(|chars, cur| {
                    if cur < chars.len() {
                        chars.remove(cur);
                    }
                });
                self.update_filter();
            }
            KeyCode::Char(c) if !has_ctrl => {
                self.edit_search(|chars, cur| chars.insert(cur, c));
                self.search_cursor += 1;
                self.update_filter();
            }
            _ => {}
        }
        None
    }

    fn submit(&self) {
        if self.to_push.is_empty() && self.to_insert.is_empty() {
            return;
        }
        daemon::start();
        let insert_indices = self.to_insert.iter().copied().rev().collect::<Vec<_>>();
        for idx in insert_indices {
            let file = &self.files[idx];
            let _ = control::insert_next(&file.to_string_lossy());
            println!("{}", file.display());
        }
        for idx in &self.to_push {
            let file = &self.files[*idx];
            let _ = control::push_to_playlist(&file.to_string_lossy());
            println!("{}", file.display());
        }
    }
}

pub fn run(dir: &str) {
    let dir = config::resolve_tilde(dir);
    let mut files = find::find_files(&dir);
    files.sort();
    if files.is_empty() {
        eprintln!("no audio files found in {}", dir.display());
        return;
    }

    term_alternate_raw();
    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout())).unwrap();
    let mut picker = Picker::new(files);
    let perform: bool;

    loop {
        picker.view.resize();
        terminal.draw(|f| picker.render(f)).ok();

        if event::poll(Duration::from_millis(100)).unwrap()
            && let Event::Key(key) = event::read().unwrap()
        {
            let done = if picker.search_mode {
                picker.handle_search_input(key)
            } else {
                picker.handle_main_input(key)
            };
            if let Some(d) = done {
                perform = d;
                break;
            }
        }
    }

    term_restore();

    if perform {
        picker.submit();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{FakeServer, handler_fn, mpv_ok};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::backend::TestBackend;
    use serde_json::Value;
    use std::path::PathBuf;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    fn files(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    fn picker_of(names: &[&str]) -> Picker {
        let mut p = Picker::new(files(names));
        p.view.height = 5;
        p
    }

    fn noop_handler() -> crate::test_util::MpvHandler {
        handler_fn(|_| mpv_ok(Value::Null))
    }

    fn buffer_str(p: &Picker, width: u16, height: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(width, height)).unwrap();
        t.draw(|f| p.render(f)).unwrap();
        let buf = t.backend_mut().buffer().clone();
        let mut out = String::new();
        for y in 0..height {
            for x in 0..width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn new_sets_initial_state() {
        let p = Picker::new(files(&["a.mp3", "b.flac"]));
        assert_eq!(p.files, files(&["a.mp3", "b.flac"]));
        assert_eq!(p.original, p.files);
        assert_eq!(p.filtered, vec![0, 1]);
        assert!(!p.search_mode);
        assert!(!p.shuffled);
        assert!(!p.absolute);
        assert!(p.to_push.is_empty());
        assert!(p.to_insert.is_empty());
        assert_eq!(p.view.count, 2);
        assert_eq!(p.search_cursor, 0);
    }

    #[test]
    fn update_filter_without_search_selects_all() {
        let mut p = picker_of(&["a.mp3", "b.flac"]);
        p.update_filter();
        assert_eq!(p.filtered, vec![0, 1]);
    }

    #[test]
    fn update_filter_matches_regex_on_names() {
        let mut p = picker_of(&["first.mp3", "second.flac", "3rd.ogg"]);
        p.search = "mp3".into();
        p.update_filter();
        assert_eq!(p.filtered, vec![0]);
        p.search = "(flac|ogg)".into();
        p.update_filter();
        assert_eq!(p.filtered, vec![1, 2]);
    }

    #[test]
    fn update_filter_invalid_regex_keeps_all() {
        let mut p = picker_of(&["a.mp3", "b.flac"]);
        p.search = "[".into();
        p.update_filter();
        assert_eq!(p.filtered, vec![0, 1]);
        assert_eq!(p.view.count, 2);
    }

    #[test]
    fn update_filter_updates_view_count() {
        let mut p = picker_of(&["a.mp3", "b.flac", "c.ogg"]);
        p.search = "mp3".into();
        p.update_filter();
        assert_eq!(p.view.count, 1);
    }

    #[test]
    fn toggle_push_adds_and_removes() {
        let mut p = picker_of(&["a.mp3", "b.flac"]);
        p.toggle_push();
        assert_eq!(p.to_push, vec![0]);
        p.toggle_push();
        assert!(p.to_push.is_empty());
    }

    #[test]
    fn toggle_push_switches_insert_to_push() {
        let mut p = picker_of(&["a.mp3", "b.flac"]);
        p.to_insert = vec![0];
        p.toggle_push();
        assert!(p.to_insert.is_empty());
        assert_eq!(p.to_push, vec![0]);
    }

    #[test]
    fn toggle_push_ignores_cursor_out_of_range() {
        let mut p = picker_of(&["a.mp3"]);
        p.view.cursor = 5;
        p.toggle_push();
        assert!(p.to_push.is_empty());
    }

    #[test]
    fn toggle_insert_adds_removes_and_converts_push() {
        let mut p = picker_of(&["a.mp3", "b.flac"]);
        p.toggle_insert();
        assert_eq!(p.to_insert, vec![0]);
        p.toggle_insert();
        assert!(p.to_insert.is_empty());
        p.view.cursor = 1;
        p.to_push = vec![1];
        p.toggle_insert();
        assert!(p.to_push.is_empty());
        assert_eq!(p.to_insert, vec![1]);
    }

    #[test]
    fn toggle_all_selects_and_clears() {
        let mut p = picker_of(&["a.mp3", "b.flac", "c.ogg"]);
        p.toggle_all();
        assert_eq!(p.to_push, vec![0, 1, 2]);
        p.toggle_all();
        assert!(p.to_push.is_empty() && p.to_insert.is_empty());
    }

    #[test]
    fn toggle_all_respects_filter() {
        let mut p = picker_of(&["a.mp3", "b.flac", "c.ogg"]);
        p.search = "f".into();
        p.update_filter();
        p.toggle_all();
        assert_eq!(p.to_push, vec![1]);
    }

    #[test]
    fn shuffle_toggles_and_restores() {
        let mut p = picker_of(&["a.mp3", "b.flac", "c.ogg", "d.wav"]);
        p.shuffle();
        assert!(p.shuffled);
        assert_eq!(p.files.len(), 4);
        assert_eq!(p.filtered.len(), 4);
        p.shuffle();
        assert!(!p.shuffled);
        assert_eq!(p.files, files(&["a.mp3", "b.flac", "c.ogg", "d.wav"]));
    }

    #[test]
    fn search_len_counts_chars() {
        let p = Picker {
            search: "héllo x".into(),
            search_cursor: 3,
            ..picker_of(&[])
        };
        assert_eq!(p.search_len(), 7);
    }

    #[test]
    fn search_word_start_bounds() {
        let mut p = picker_of(&[]);
        p.search = "one two three".into();
        assert_eq!(p.search_word_start(0), 0);
        assert_eq!(p.search_word_start(4), 0);
        assert_eq!(p.search_word_start(8), 4);
        assert_eq!(p.search_word_start(99), 8);
    }

    #[test]
    fn search_word_end_bounds() {
        let mut p = picker_of(&[]);
        p.search = "one two three".into();
        assert_eq!(p.search_word_end(0), 3);
        assert_eq!(p.search_word_end(3), 7);
        assert_eq!(p.search_word_end(4), 7);
        assert_eq!(p.search_word_end(12), 13);
    }

    #[test]
    fn edit_search_inserts_and_clamps_cursor() {
        let mut p = picker_of(&[]);
        p.search = "ac".into();
        p.search_cursor = 1;
        p.edit_search(|chars, cur| chars.insert(cur, 'b'));
        assert_eq!(p.search, "abc");
        assert_eq!(p.search_cursor, 1);
        p.search_cursor = 99;
        p.edit_search(|_, _| {});
        assert_eq!(p.search_cursor, 3);
    }

    #[test]
    fn edit_search_removes_at_cursor() {
        let mut p = picker_of(&[]);
        p.search = "abcd".into();
        p.search_cursor = 2;
        p.edit_search(|chars, cur| {
            chars.remove(cur);
        });
        assert_eq!(p.search, "abd");
    }

    #[test]
    fn render_is_empty_list_shows_prompt() {
        let mut p = picker_of(&[]);
        p.view.height = 3;
        p.update_filter();
        assert!(buffer_str(&p, 40, 4).contains("No matches."));
    }

    #[test]
    fn render_items_shows_selection_prefixes() {
        let mut p = picker_of(&["a.mp3", "b.flac"]);
        p.to_push = vec![0];
        p.to_insert = vec![1];
        p.view.height = 2;
        let out = buffer_str(&p, 40, 3);
        let row: String = out.lines().next().unwrap().to_string();
        assert!(row.trim_start().starts_with("* "));
        let row2: String = out.lines().nth(1).unwrap().to_string();
        assert!(row2.trim_start().starts_with("i "));
    }

    #[test]
    fn render_search_mode_shows_cursor() {
        let mut p = picker_of(&["a.mp3"]);
        p.view.height = 2;
        p.search_mode = true;
        p.search = "abc".into();
        p.search_cursor = 2;
        let out = buffer_str(&p, 40, 5);
        assert!(out.contains("/ab"));
    }

    #[test]
    fn render_search_inactive_shows_query() {
        let mut p = picker_of(&["a.mp3"]);
        p.view.height = 2;
        p.search = "clip".into();
        let out = buffer_str(&p, 40, 5);
        assert!(out.contains("/clip"));
    }

    #[test]
    fn handle_main_input_quit_and_submit() {
        let mut p = picker_of(&["a.mp3"]);
        assert_eq!(p.handle_main_input(key(KeyCode::Esc)), Some(false));
        assert_eq!(p.handle_main_input(key(KeyCode::Char('q'))), Some(false));
        assert_eq!(p.handle_main_input(ctrl(KeyCode::Char('c'))), Some(false));
        assert_eq!(p.handle_main_input(key(KeyCode::Enter)), Some(true));
    }

    #[test]
    fn handle_main_input_navigation() {
        let mut p = picker_of(&["a.mp3", "b.flac", "c.ogg"]);
        assert_eq!(p.handle_main_input(key(KeyCode::Down)), None);
        assert_eq!(p.view.cursor, 1);
        assert_eq!(p.handle_main_input(key(KeyCode::Char('j'))), None);
        assert_eq!(p.view.cursor, 2);
        assert_eq!(p.handle_main_input(key(KeyCode::Char('k'))), None);
        assert_eq!(p.view.cursor, 1);
        assert_eq!(p.handle_main_input(key(KeyCode::Up)), None);
        assert_eq!(p.view.cursor, 0);
        assert_eq!(p.handle_main_input(ctrl(KeyCode::Char('n'))), None);
        assert_eq!(p.view.cursor, 1);
        assert_eq!(p.handle_main_input(ctrl(KeyCode::Char('p'))), None);
        assert_eq!(p.view.cursor, 0);
        assert_eq!(p.handle_main_input(key(KeyCode::Char('g'))), None);
        assert_eq!((p.view.offset, p.view.cursor), (0, 0));
        assert_eq!(p.handle_main_input(key(KeyCode::Char('G'))), None);
        assert_eq!(p.view.cursor, 2);
        p.view.cursor = 1;
        p.view.offset = 1;
        assert_eq!(p.handle_main_input(key(KeyCode::Char('H'))), None);
        assert_eq!(p.view.cursor, 1);
        assert_eq!(p.handle_main_input(key(KeyCode::Char('L'))), None);
        assert_eq!(p.view.cursor, 2);
    }

    #[test]
    fn handle_main_input_scroll_and_page() {
        let mut p = picker_of(&["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"]);
        p.view.height = 3;
        p.view.cursor = 2;
        p.view.offset = 0;
        assert_eq!(p.handle_main_input(ctrl(KeyCode::Char('e'))), None);
        assert_eq!(p.view.offset, 1);
        assert_eq!(p.handle_main_input(ctrl(KeyCode::Char('y'))), None);
        assert_eq!(p.view.offset, 0);
        assert_eq!(p.handle_main_input(ctrl(KeyCode::Char('d'))), None);
        assert!(p.view.offset > 0);
        assert_eq!(p.handle_main_input(ctrl(KeyCode::Char('u'))), None);
        assert!(p.view.offset < 2);
    }

    #[test]
    fn handle_main_input_select_keys() {
        let mut p = picker_of(&["a.mp3", "b.flac"]);
        assert_eq!(p.handle_main_input(key(KeyCode::Char(' '))), None);
        assert_eq!(p.to_push, vec![0]);
        assert_eq!(p.handle_main_input(key(KeyCode::Tab)), None);
        assert!(p.to_push.is_empty());
        assert_eq!(p.handle_main_input(key(KeyCode::Char('i'))), None);
        assert_eq!(p.to_insert, vec![0]);
        assert_eq!(p.handle_main_input(ctrl(KeyCode::Char('a'))), None);
        assert_eq!(p.to_push, vec![1]);
    }

    #[test]
    fn handle_main_input_toggles() {
        let mut p = picker_of(&["a.mp3", "b.flac", "c.ogg"]);
        assert_eq!(p.handle_main_input(key(KeyCode::Char('f'))), None);
        assert!(p.absolute);
        assert_eq!(p.handle_main_input(key(KeyCode::Char('r'))), None);
        assert!(p.shuffled);
        assert_eq!(p.handle_main_input(key(KeyCode::Char('/'))), None);
        assert!(p.search_mode);
    }

    #[test]
    fn handle_search_input_enter_applies() {
        let mut p = picker_of(&["a.mp3", "b.flac"]);
        p.search_mode = true;
        p.search = "flac".into();
        assert_eq!(p.handle_search_input(key(KeyCode::Enter)), None);
        assert!(!p.search_mode);
        assert_eq!(p.filtered, vec![1]);
    }

    #[test]
    fn handle_search_input_cancel() {
        let mut p = picker_of(&["a.mp3", "b.flac"]);
        p.search_mode = true;
        p.search = "zz".into();
        p.search_cursor = 1;
        assert_eq!(p.handle_search_input(key(KeyCode::Esc)), None);
        assert!(!p.search_mode);
        assert!(p.search.is_empty());
        assert_eq!(p.search_cursor, 0);

        let mut p = picker_of(&["a.mp3"]);
        p.search_mode = true;
        assert_eq!(p.handle_search_input(ctrl(KeyCode::Char('c'))), None);
        assert!(!p.search_mode);
    }

    #[test]
    fn handle_search_input_cursor_movement() {
        let mut p = picker_of(&[]);
        p.search = "abcd".into();
        assert_eq!(p.handle_search_input(key(KeyCode::Home)), None);
        assert_eq!(p.search_cursor, 0);
        assert_eq!(p.handle_search_input(key(KeyCode::End)), None);
        assert_eq!(p.search_cursor, 4);
        assert_eq!(p.handle_search_input(ctrl(KeyCode::Char('a'))), None);
        assert_eq!(p.search_cursor, 0);
        assert_eq!(p.handle_search_input(ctrl(KeyCode::Char('e'))), None);
        assert_eq!(p.search_cursor, 4);
        assert_eq!(p.handle_search_input(key(KeyCode::Left)), None);
        assert_eq!(p.search_cursor, 3);
        assert_eq!(p.handle_search_input(ctrl(KeyCode::Char('b'))), None);
        assert_eq!(p.search_cursor, 2);
        assert_eq!(p.handle_search_input(key(KeyCode::Right)), None);
        assert_eq!(p.search_cursor, 3);
        assert_eq!(p.handle_search_input(ctrl(KeyCode::Char('f'))), None);
        assert_eq!(p.search_cursor, 4);
    }

    #[test]
    fn handle_search_input_word_navigation() {
        let mut p = picker_of(&[]);
        p.search = "one two".into();
        p.search_cursor = 4;
        assert_eq!(p.handle_search_input(ctrl(KeyCode::Left)), None);
        assert_eq!(p.search_cursor, 0);
        p.search_cursor = 0;
        assert_eq!(p.handle_search_input(ctrl(KeyCode::Right)), None);
        assert_eq!(p.search_cursor, 3);
    }

    #[test]
    fn handle_search_input_delete_word() {
        let mut p = picker_of(&[]);
        p.search = "one two".into();
        p.search_cursor = 4;
        assert_eq!(p.handle_search_input(ctrl(KeyCode::Char('w'))), None);
        assert_eq!(p.search, "two");
        assert_eq!(p.search_cursor, 0);
    }

    #[test]
    fn handle_search_input_kill_line() {
        let mut p = picker_of(&[]);
        p.search = "abc def".into();
        p.search_cursor = 4;
        assert_eq!(p.handle_search_input(ctrl(KeyCode::Char('k'))), None);
        assert_eq!(p.search, "abc ");
        assert_eq!(p.handle_search_input(ctrl(KeyCode::Char('u'))), None);
        assert_eq!(p.search, "");
    }

    #[test]
    fn handle_search_input_delete_chars() {
        let mut p = picker_of(&[]);
        p.search = "abcd".into();
        p.search_cursor = 2;
        assert_eq!(p.handle_search_input(key(KeyCode::Backspace)), None);
        assert_eq!(p.search, "acd");
        assert_eq!(p.search_cursor, 1);
        assert_eq!(p.handle_search_input(ctrl(KeyCode::Char('h'))), None);
        assert_eq!(p.search, "cd");
        assert_eq!(p.handle_search_input(key(KeyCode::Delete)), None);
        assert_eq!(p.search, "d");
        assert_eq!(p.handle_search_input(ctrl(KeyCode::Char('d'))), None);
        assert_eq!(p.search, "");
    }

    #[test]
    fn handle_search_input_backspace_at_origin_is_noop() {
        let mut p = picker_of(&[]);
        p.search_cursor = 0;
        assert_eq!(p.handle_search_input(key(KeyCode::Backspace)), None);
        assert_eq!(p.search_cursor, 0);
    }

    #[test]
    fn handle_search_input_inserts_char() {
        let mut p = picker_of(&[]);
        assert_eq!(p.handle_search_input(key(KeyCode::Char('h'))), None);
        assert_eq!(p.search, "h");
        assert_eq!(p.search_cursor, 1);
        p.search_cursor = 0;
        assert_eq!(p.handle_search_input(key(KeyCode::Char('x'))), None);
        assert_eq!(p.search, "xh");
    }

    #[test]
    fn handle_search_input_ignores_ctrl_chars() {
        let mut p = picker_of(&[]);
        assert_eq!(p.handle_search_input(ctrl(KeyCode::Char('q'))), None);
        assert_eq!(p.search, "");
    }

    #[test]
    fn cancel_search_clears_state() {
        let mut p = picker_of(&["a.mp3", "b.flac"]);
        p.search_mode = true;
        p.search = "nope".into();
        p.search_cursor = 2;
        p.cancel_search();
        assert!(!p.search_mode);
        assert!(p.search.is_empty());
        assert_eq!(p.search_cursor, 0);
        assert_eq!(p.filtered, vec![0, 1]);
    }

    #[test]
    fn submit_without_selection_is_noop() {
        let p = picker_of(&["a.mp3"]);
        p.submit();
    }

    #[test]
    fn submit_sends_insert_then_push() {
        let server = FakeServer::start(noop_handler());
        server.with_env(|| {
            let mut p = picker_of(&["a.mp3", "b.flac", "c.ogg"]);
            p.to_insert = vec![1];
            p.to_push = vec![2];
            p.submit();
        });
        let cmds: Vec<Value> = server
            .received
            .try_iter()
            .filter_map(|r| r.get("command").cloned())
            .collect();
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0][0], "loadfile");
        assert_eq!(cmds[0][2], "insert-next");
        assert_eq!(cmds[1][2], "append-play");
    }

    #[test]
    fn run_with_empty_dir_returns_quietly() {
        let dir = crate::test_util::Temp::new("pick-empty");
        run(dir.path().to_str().unwrap());
    }

    #[test]
    fn run_with_nonexistent_dir_returns_quietly() {
        run("/nonexistent/mpvd-pick-dir");
    }
}
