use std::collections::HashSet;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;
use std::thread::sleep;
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
            view: ListView::new(),
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

    fn clamp_scroll(&mut self, term_height: u16) {
        self.view.clamp_scroll(self.filtered.len(), term_height);
    }

    fn cursor_up(&mut self, term_height: u16) {
        self.view.cursor_up(self.filtered.len(), term_height);
    }

    fn cursor_down(&mut self, term_height: u16) {
        self.view.cursor_down(self.filtered.len(), term_height);
    }

    fn scroll_up(&mut self, amount: usize) {
        self.view.scroll_up(amount);
    }

    fn scroll_down(&mut self, amount: usize, term_height: u16) {
        self.view
            .scroll_down(amount, self.filtered.len(), term_height);
    }

    fn page_up(&mut self, term_height: u16) {
        self.view.page_up(self.filtered.len(), term_height);
    }

    fn page_down(&mut self, term_height: u16) {
        self.view.page_down(self.filtered.len(), term_height);
    }

    fn go_top(&mut self) {
        self.view.go_top();
    }

    fn go_bottom(&mut self, term_height: u16) {
        self.view.go_bottom(self.filtered.len(), term_height);
    }

    fn cursor_home(&mut self) {
        self.view.cursor_home();
    }

    fn cursor_end(&mut self, term_height: u16) {
        self.view.cursor_end(self.filtered.len(), term_height);
    }

    fn update_filter(&mut self, term_height: u16) {
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
        self.clamp_scroll(term_height);
    }

    fn toggle_push(&mut self) {
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
        let idx = self.filtered[self.view.cursor];
        self.to_push.remove(&idx);
        if self.to_insert.contains(&idx) {
            self.to_insert.remove(&idx);
        } else {
            self.to_insert.insert(idx);
        }
    }

    fn shuffle(&mut self, term_height: u16) {
        use rand::seq::SliceRandom;
        self.files = self.original.clone();
        if self.shuffled {
            self.shuffled = false;
        } else {
            self.files.shuffle(&mut rand::rng());
            self.shuffled = true;
        }
        self.update_filter(term_height);
    }

    fn search_len(&self) -> usize {
        self.search.chars().count()
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
    let files = find::find_files(&dir);
    if files.is_empty() {
        eprintln!("no audio files found in {}", dir.display());
        return;
    }

    enable_raw_mode().unwrap();
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).unwrap();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut picker = Picker::new(files);
    let term_height = terminal.size().unwrap().height;
    picker.clamp_scroll(term_height);

    loop {
        let term_height = terminal.size().unwrap().height;
        terminal.draw(|f| render(f, &picker, term_height)).unwrap();

        if event::poll(Duration::from_millis(100)).unwrap()
            && let Event::Key(key) = event::read().unwrap()
        {
            let done = if picker.search_mode {
                handle_search_input(&mut picker, key, term_height)
            } else {
                handle_main_input(&mut picker, key, term_height)
            };
            if done.is_some() {
                break;
            }
        }
    }

    disable_raw_mode().unwrap();
    execute!(terminal.backend_mut(), LeaveAlternateScreen).unwrap();
    terminal.show_cursor().unwrap();

    submit(&picker);
}

fn render(f: &mut Frame, picker: &Picker, term_height: u16) {
    let area = f.area();
    let list_height = ListView::list_height(term_height);

    let items: Vec<Line> = picker.filtered[picker.view.offset..]
        .iter()
        .take(list_height)
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
        Rect::new(0, 0, area.width, list_height as u16),
    );

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
        f.render_widget(search_line, Rect::new(0, list_height as u16, area.width, 1));
    } else if !picker.search.is_empty() {
        let search_line = Line::from(Span::raw(format!("/{}", picker.search)));
        f.render_widget(search_line, Rect::new(0, list_height as u16, area.width, 1));
    }
}

fn handle_main_input(picker: &mut Picker, key: KeyEvent, term_height: u16) -> Option<bool> {
    let has_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => return Some(false),
        KeyCode::Char('c') if has_ctrl => return Some(false),
        KeyCode::Char('e') if has_ctrl => picker.scroll_down(1, term_height),
        KeyCode::Char('y') if has_ctrl => picker.scroll_up(1),
        KeyCode::Char('d') if has_ctrl => picker.page_down(term_height),
        KeyCode::Char('u') if has_ctrl => picker.page_up(term_height),
        KeyCode::Char('H') => picker.cursor_home(),
        KeyCode::Char('L') => picker.cursor_end(term_height),
        KeyCode::Down | KeyCode::Char('j') => picker.cursor_down(term_height),
        KeyCode::Char('n') if has_ctrl => picker.cursor_down(term_height),
        KeyCode::Up | KeyCode::Char('k') => picker.cursor_up(term_height),
        KeyCode::Char('p') if has_ctrl => picker.cursor_up(term_height),
        KeyCode::Char('g') => picker.go_top(),
        KeyCode::Char('G') => picker.go_bottom(term_height),
        KeyCode::Char('f') => picker.absolute = !picker.absolute,
        KeyCode::Char('r') => picker.shuffle(term_height),
        KeyCode::Char(' ') | KeyCode::Tab => picker.toggle_push(),
        KeyCode::Char('i') => picker.toggle_insert(),
        KeyCode::Enter => return Some(true),
        KeyCode::Char('/') => picker.search_mode = true,
        _ => {}
    }
    None
}

fn cancel_search(picker: &mut Picker, term_height: u16) {
    picker.search.clear();
    picker.search_cursor = 0;
    picker.search_mode = false;
    picker.update_filter(term_height);
}

fn handle_search_input(picker: &mut Picker, key: KeyEvent, term_height: u16) -> Option<bool> {
    let has_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    match key.code {
        KeyCode::Enter => {
            picker.search_mode = false;
            picker.update_filter(term_height);
        }
        KeyCode::Esc => cancel_search(picker, term_height),
        KeyCode::Char('c') if has_ctrl => cancel_search(picker, term_height),
        KeyCode::Home => picker.search_cursor = 0,
        KeyCode::Char('a') if has_ctrl => picker.search_cursor = 0,
        KeyCode::End => picker.search_cursor = picker.search_len(),
        KeyCode::Char('e') if has_ctrl => picker.search_cursor = picker.search_len(),
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
            picker.update_filter(term_height);
        }
        KeyCode::Char('k') if has_ctrl => {
            picker.edit_search(|chars, cur| chars.truncate(cur));
            picker.update_filter(term_height);
        }
        KeyCode::Backspace => {
            if picker.search_cursor > 0 {
                picker.search_cursor -= 1;
                picker.edit_search(|chars, cur| {
                    chars.remove(cur);
                });
                picker.update_filter(term_height);
            }
        }
        KeyCode::Char('h') if has_ctrl => {
            if picker.search_cursor > 0 {
                picker.search_cursor -= 1;
                picker.edit_search(|chars, cur| {
                    chars.remove(cur);
                });
                picker.update_filter(term_height);
            }
        }
        KeyCode::Delete => {
            picker.edit_search(|chars, cur| {
                if cur < chars.len() {
                    chars.remove(cur);
                }
            });
            picker.update_filter(term_height);
        }
        KeyCode::Char('d') if has_ctrl => {
            picker.edit_search(|chars, cur| {
                if cur < chars.len() {
                    chars.remove(cur);
                }
            });
            picker.update_filter(term_height);
        }
        KeyCode::Char(c) if !has_ctrl => {
            picker.edit_search(|chars, cur| chars.insert(cur, c));
            picker.search_cursor += 1;
            picker.update_filter(term_height);
        }
        _ => {}
    }
    None
}

fn submit(picker: &Picker) {
    if picker.to_push.is_empty() && picker.to_insert.is_empty() {
        return;
    }
    let started = daemon::start();
    if started == ExitCode::SUCCESS {
        sleep(Duration::from_millis(200));
    }
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
