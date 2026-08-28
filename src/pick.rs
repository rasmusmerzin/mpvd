use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
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
}

pub fn run(dir: &str) {
    let dir = config::resolve_tilde(dir);
    let mut files = find::find_files(&dir);
    files.sort();
    if files.is_empty() {
        eprintln!("no audio files found in {}", dir.display());
        return;
    }

    enable_raw_mode().unwrap();
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen).unwrap();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut picker = Picker::new(files);

    loop {
        picker.view.resize();
        terminal.draw(|f| render(f, &picker)).ok();

        if event::poll(Duration::from_millis(100)).unwrap()
            && let Event::Key(key) = event::read().unwrap()
        {
            let done = if picker.search_mode {
                handle_search_input(&mut picker, key)
            } else {
                handle_main_input(&mut picker, key)
            };
            if done.is_some() {
                break;
            }
        }
    }

    disable_raw_mode().ok();
    execute!(terminal.backend_mut(), LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();

    submit(&picker);
}

fn render(f: &mut Frame, picker: &Picker) {
    let area = f.area();

    let items: Vec<Line> = picker.filtered[picker.view.offset..]
        .iter()
        .take(picker.view.height)
        .enumerate()
        .map(|(i, &file_idx)| {
            let file = &picker.files[file_idx];
            let is_hover = i + picker.view.offset == picker.view.cursor;
            let is_push = picker.to_push.contains(&file_idx);
            let is_insert = picker.to_insert.contains(&file_idx);

            let prefix = if is_insert {
                "i "
            } else if is_push {
                "* "
            } else {
                "  "
            };

            let name = if picker.absolute {
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
        Rect::new(0, 0, area.width, picker.view.height as u16),
    );

    if picker.filtered.is_empty() {
        let empty_msg = Line::from(Span::styled(
            "No matches.",
            Style::default().add_modifier(Modifier::ITALIC).dim(),
        ));
        f.render_widget(
            empty_msg,
            Rect::new(0, 0, area.width, picker.view.height as u16),
        );
    }

    if picker.search_mode {
        let before: String = picker.search.chars().take(picker.search_cursor).collect();
        let at_cursor: String = picker
            .search
            .chars()
            .skip(picker.search_cursor)
            .take(1)
            .collect();
        let after: String = picker
            .search
            .chars()
            .skip(picker.search_cursor + 1)
            .collect();
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
            Rect::new(0, picker.view.height as u16, area.width, 1),
        );
    } else if !picker.search.is_empty() {
        let search_line = Line::from(Span::raw(format!("/{}", picker.search)));
        f.render_widget(
            search_line,
            Rect::new(0, picker.view.height as u16, area.width, 1),
        );
    }
}

fn handle_main_input(picker: &mut Picker, key: KeyEvent) -> Option<bool> {
    let has_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => return Some(false),
        KeyCode::Char('c') if has_ctrl => return Some(false),
        KeyCode::Char('e') if has_ctrl => picker.view.scroll_down(1),
        KeyCode::Char('y') if has_ctrl => picker.view.scroll_up(1),
        KeyCode::Char('d') if has_ctrl => picker.view.page_down(),
        KeyCode::Char('u') if has_ctrl => picker.view.page_up(),
        KeyCode::Char('H') => picker.view.cursor_home(),
        KeyCode::Char('L') => picker.view.cursor_end(),
        KeyCode::Down | KeyCode::Char('j') => picker.view.cursor_down(),
        KeyCode::Char('n') if has_ctrl => picker.view.cursor_down(),
        KeyCode::Up | KeyCode::Char('k') => picker.view.cursor_up(),
        KeyCode::Char('p') if has_ctrl => picker.view.cursor_up(),
        KeyCode::Char('g') => picker.view.go_top(),
        KeyCode::Char('G') => picker.view.go_bottom(),
        KeyCode::Char('f') => picker.absolute = !picker.absolute,
        KeyCode::Char('r') => picker.shuffle(),
        KeyCode::Char(' ') | KeyCode::Tab => picker.toggle_push(),
        KeyCode::Char('i') => picker.toggle_insert(),
        KeyCode::Enter => return Some(true),
        KeyCode::Char('/') => picker.search_mode = true,
        _ => {}
    }
    None
}

fn cancel_search(picker: &mut Picker) {
    picker.search.clear();
    picker.search_cursor = 0;
    picker.search_mode = false;
    picker.update_filter();
}

fn handle_search_input(picker: &mut Picker, key: KeyEvent) -> Option<bool> {
    let has_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    match key.code {
        KeyCode::Enter => {
            picker.search_mode = false;
            picker.update_filter();
        }
        KeyCode::Esc => cancel_search(picker),
        KeyCode::Char('c') if has_ctrl => cancel_search(picker),
        KeyCode::Home => picker.search_cursor = 0,
        KeyCode::Char('a') if has_ctrl => picker.search_cursor = 0,
        KeyCode::End => picker.search_cursor = picker.search_len(),
        KeyCode::Char('e') if has_ctrl => picker.search_cursor = picker.search_len(),
        KeyCode::Char('w') if has_ctrl => {
            let target = picker.search_word_start(picker.search_cursor);
            picker.edit_search(|chars, cur| {
                chars.drain(target..cur);
            });
            picker.search_cursor = target;
            picker.update_filter();
        }
        KeyCode::Left if has_ctrl => {
            picker.search_cursor = picker.search_word_start(picker.search_cursor);
        }
        KeyCode::Right if has_ctrl => {
            picker.search_cursor = picker.search_word_end(picker.search_cursor);
        }
        KeyCode::Left => picker.search_cursor = picker.search_cursor.saturating_sub(1),
        KeyCode::Char('b') if has_ctrl => {
            picker.search_cursor = picker.search_cursor.saturating_sub(1);
        }
        KeyCode::Right => {
            if picker.search_cursor < picker.search_len() {
                picker.search_cursor += 1;
            }
        }
        KeyCode::Char('f') if has_ctrl => {
            if picker.search_cursor < picker.search_len() {
                picker.search_cursor += 1;
            }
        }
        KeyCode::Char('u') if has_ctrl => {
            picker.edit_search(|chars, cur| {
                chars.drain(..cur);
            });
            picker.search_cursor = 0;
            picker.update_filter();
        }
        KeyCode::Char('k') if has_ctrl => {
            picker.edit_search(|chars, cur| chars.truncate(cur));
            picker.update_filter();
        }
        KeyCode::Backspace => {
            if picker.search_cursor > 0 {
                picker.search_cursor -= 1;
                picker.edit_search(|chars, cur| {
                    chars.remove(cur);
                });
                picker.update_filter();
            }
        }
        KeyCode::Char('h') if has_ctrl => {
            if picker.search_cursor > 0 {
                picker.search_cursor -= 1;
                picker.edit_search(|chars, cur| {
                    chars.remove(cur);
                });
                picker.update_filter();
            }
        }
        KeyCode::Delete => {
            picker.edit_search(|chars, cur| {
                if cur < chars.len() {
                    chars.remove(cur);
                }
            });
            picker.update_filter();
        }
        KeyCode::Char('d') if has_ctrl => {
            picker.edit_search(|chars, cur| {
                if cur < chars.len() {
                    chars.remove(cur);
                }
            });
            picker.update_filter();
        }
        KeyCode::Char(c) if !has_ctrl => {
            picker.edit_search(|chars, cur| chars.insert(cur, c));
            picker.search_cursor += 1;
            picker.update_filter();
        }
        _ => {}
    }
    None
}

fn submit(picker: &Picker) {
    if picker.to_push.is_empty() && picker.to_insert.is_empty() {
        return;
    }
    // start() only returns once a newly spawned daemon's IPC socket is ready
    daemon::start();
    let mut push_indices: Vec<usize> = picker.to_push.iter().copied().collect();
    push_indices.sort_unstable();
    let mut insert_indices: Vec<usize> = picker.to_insert.iter().copied().collect();
    insert_indices.sort_unstable();
    insert_indices.reverse();
    for idx in push_indices {
        let file = &picker.files[idx];
        let _ = control::push_to_playlist(&file.to_string_lossy());
        println!("{}", file.display());
    }
    for idx in insert_indices {
        let file = &picker.files[idx];
        let _ = control::insert_next(&file.to_string_lossy());
        println!("{}", file.display());
    }
}
