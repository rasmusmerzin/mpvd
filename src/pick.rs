use std::collections::HashSet;
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
    to_push: HashSet<usize>,
    to_insert: HashSet<usize>,
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
            to_push: HashSet::new(),
            to_insert: HashSet::new(),
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
        if self.to_insert.contains(&idx) {
            self.to_insert.remove(&idx);
            self.to_push.insert(idx);
        } else if self.to_push.contains(&idx) {
            self.to_push.remove(&idx);
        } else {
            self.to_push.insert(idx);
        }
    }

    fn toggle_insert(&mut self) {
        if self.view.cursor >= self.filtered.len() {
            return;
        }
        let idx = self.filtered[self.view.cursor];
        self.to_push.remove(&idx);
        if self.to_insert.contains(&idx) {
            self.to_insert.remove(&idx);
        } else {
            self.to_insert.insert(idx);
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
        let mut push_indices: Vec<usize> = self.to_push.iter().copied().collect();
        push_indices.sort_unstable();
        let mut insert_indices: Vec<usize> = self.to_insert.iter().copied().collect();
        insert_indices.sort_unstable();
        insert_indices.reverse();
        for idx in insert_indices {
            let file = &self.files[idx];
            let _ = control::insert_next(&file.to_string_lossy());
            println!("{}", file.display());
        }
        for idx in push_indices {
            let file = &self.files[idx];
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
