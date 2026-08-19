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
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::{Frame, Terminal};

use crate::config;
use crate::control;
use crate::daemon;
use crate::find;

struct Picker {
    files: Vec<PathBuf>,
    original: Vec<PathBuf>,
    filtered: Vec<usize>,
    cursor: usize,
    offset: usize,
    to_push: HashSet<usize>,
    to_insert: HashSet<usize>,
    search: String,
    search_mode: bool,
    absolute: bool,
    shuffled: bool,
}

impl Picker {
    fn new(files: Vec<PathBuf>) -> Self {
        let len = files.len();
        Self {
            original: files.clone(),
            filtered: (0..len).collect(),
            files,
            cursor: 0,
            offset: 0,
            to_push: HashSet::new(),
            to_insert: HashSet::new(),
            search: String::new(),
            search_mode: false,
            absolute: false,
            shuffled: false,
        }
    }

    fn list_height(&self, term_height: u16) -> usize {
        term_height.saturating_sub(1) as usize
    }

    fn clamp_scroll(&mut self, term_height: u16) {
        let h = self.list_height(term_height);
        let max_offset = self.filtered.len().saturating_sub(h);
        self.offset = self.offset.min(max_offset);
        let min_cursor = self.offset;
        let max_cursor = (self.offset + h)
            .saturating_sub(1)
            .min(self.filtered.len().saturating_sub(1));
        self.cursor = self.cursor.clamp(min_cursor, max_cursor);
    }

    fn update_filter(&mut self, term_height: u16) {
        if self.search.is_empty() {
            self.filtered = (0..self.files.len()).collect();
        } else {
            match regex::Regex::new(&self.search) {
                Ok(re) => {
                    self.filtered = self
                        .files
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| {
                            p.file_name()
                                .and_then(|n| n.to_str())
                                .map(|n| re.is_match(n))
                                .unwrap_or(false)
                        })
                        .map(|(i, _)| i)
                        .collect();
                }
                Err(_) => {
                    self.filtered = (0..self.files.len()).collect();
                }
            }
        }
        self.clamp_scroll(term_height);
    }

    fn cursor_up(&mut self, term_height: u16) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
        if self.cursor < self.offset {
            self.offset = self.cursor;
        }
        self.clamp_scroll(term_height);
    }

    fn cursor_down(&mut self, term_height: u16) {
        if self.cursor + 1 < self.filtered.len() {
            self.cursor += 1;
        }
        let h = self.list_height(term_height);
        if self.cursor >= self.offset + h {
            self.offset = self.cursor + 1 - h;
        }
        self.clamp_scroll(term_height);
    }

    fn scroll_up(&mut self, amount: usize) {
        self.offset = self.offset.saturating_sub(amount);
    }

    fn scroll_down(&mut self, amount: usize, term_height: u16) {
        self.offset += amount;
        self.clamp_scroll(term_height);
    }

    fn page_up(&mut self, term_height: u16) {
        let h = self.list_height(term_height);
        let delta = h / 2;
        let saved = self.cursor;
        self.scroll_up(delta);
        self.cursor = saved.saturating_sub(delta);
        self.clamp_scroll(term_height);
    }

    fn page_down(&mut self, term_height: u16) {
        let h = self.list_height(term_height);
        let delta = h / 2;
        let saved = self.cursor;
        self.scroll_down(delta, term_height);
        self.cursor = saved.saturating_add(delta);
        self.clamp_scroll(term_height);
    }

    fn go_top(&mut self) {
        self.offset = 0;
        self.cursor = 0;
    }

    fn go_bottom(&mut self, term_height: u16) {
        self.offset = self
            .filtered
            .len()
            .saturating_sub(self.list_height(term_height));
        self.cursor = self.filtered.len().saturating_sub(1);
        self.clamp_scroll(term_height);
    }

    fn cursor_home(&mut self) {
        self.cursor = self.offset;
    }

    fn cursor_end(&mut self, term_height: u16) {
        let h = self.list_height(term_height);
        self.cursor = (self.offset + h)
            .saturating_sub(1)
            .min(self.filtered.len().saturating_sub(1));
    }

    fn toggle_push(&mut self) {
        let idx = self.filtered[self.cursor];
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
        let idx = self.filtered[self.cursor];
        self.to_push.remove(&idx);
        if self.to_insert.contains(&idx) {
            self.to_insert.remove(&idx);
        } else {
            self.to_insert.insert(idx);
        }
    }

    fn shuffle(&mut self, term_height: u16) {
        use rand::seq::SliceRandom;
        if self.shuffled {
            self.files = self.original.clone();
            self.shuffled = false;
        } else {
            let mut rng = rand::rng();
            self.files = self.original.clone();
            self.files.shuffle(&mut rng);
            self.shuffled = true;
        }
        self.update_filter(term_height);
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
        terminal
            .draw(|f| {
                render(f, &picker, term_height);
            })
            .unwrap();

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
    let list_height = picker.list_height(term_height);

    let items: Vec<ListItem> = picker.filtered[picker.offset..]
        .iter()
        .take(list_height)
        .enumerate()
        .map(|(i, &file_idx)| {
            let file = &picker.files[file_idx];
            let is_hover = i + picker.offset == picker.cursor;
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

            ListItem::new(Line::from(Span::styled(text, style)))
        })
        .collect();

    let list = List::new(items).block(Block::default().borders(Borders::NONE));

    let mut state = ListState::default();
    state.select(Some(0));

    f.render_stateful_widget(
        list,
        Rect::new(0, 0, area.width, list_height as u16),
        &mut state,
    );

    if picker.search_mode {
        let search_line = Line::from(vec![
            Span::raw("/"),
            Span::raw(&picker.search),
            Span::styled(" ", Style::default().add_modifier(Modifier::REVERSED)),
        ]);
        f.render_widget(search_line, Rect::new(0, list_height as u16, area.width, 1));
    } else if !picker.search.is_empty() {
        let search_line = Line::from(Span::raw(format!("/{}", picker.search)));
        f.render_widget(search_line, Rect::new(0, list_height as u16, area.width, 1));
    }
}

fn handle_main_input(picker: &mut Picker, key: KeyEvent, term_height: u16) -> Option<bool> {
    let modifiers = key.modifiers;
    let code = key.code;

    match code {
        KeyCode::Esc | KeyCode::Char('q') => return Some(false),
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => return Some(false),
        KeyCode::Char('e') if modifiers.contains(KeyModifiers::CONTROL) => {
            picker.scroll_down(1, term_height);
        }
        KeyCode::Char('y') if modifiers.contains(KeyModifiers::CONTROL) => {
            picker.scroll_up(1);
        }
        KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
            picker.page_down(term_height);
        }
        KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
            picker.page_up(term_height);
        }
        KeyCode::Char('H') => picker.cursor_home(),
        KeyCode::Char('L') => picker.cursor_end(term_height),
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('n')
            if modifiers.contains(KeyModifiers::CONTROL)
                || code == KeyCode::Down
                || code == KeyCode::Char('j') =>
        {
            picker.cursor_down(term_height);
        }
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('p')
            if modifiers.contains(KeyModifiers::CONTROL)
                || code == KeyCode::Up
                || code == KeyCode::Char('k') =>
        {
            picker.cursor_up(term_height);
        }
        KeyCode::Char('g') => picker.go_top(),
        KeyCode::Char('G') => picker.go_bottom(term_height),
        KeyCode::Char('f') => picker.absolute = !picker.absolute,
        KeyCode::Char('r') => picker.shuffle(term_height),
        KeyCode::Char(' ') | KeyCode::Tab => picker.toggle_push(),
        KeyCode::Char('i') => picker.toggle_insert(),
        KeyCode::Enter => return Some(true),
        KeyCode::Char('/') => {
            picker.search_mode = true;
        }
        _ => {}
    }
    None
}

fn handle_search_input(picker: &mut Picker, key: KeyEvent, term_height: u16) -> Option<bool> {
    match key.code {
        KeyCode::Esc => {
            picker.search.clear();
            picker.search_mode = false;
            picker.update_filter(term_height);
        }
        KeyCode::Enter => {
            picker.search_mode = false;
            picker.update_filter(term_height);
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            picker.search.clear();
            picker.search_mode = false;
            picker.update_filter(term_height);
        }
        KeyCode::Backspace | KeyCode::Char('h')
            if key.modifiers.contains(KeyModifiers::CONTROL) =>
        {
            picker.search.pop();
            picker.update_filter(term_height);
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            picker.search.push(c);
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
    let mut insert_files: Vec<&PathBuf> = picker
        .to_insert
        .iter()
        .map(|&idx| &picker.files[idx])
        .collect();
    insert_files.reverse();
    for idx in &picker.to_push {
        let file = &picker.files[*idx];
        let _ = control::push_to_playlist(&file.to_string_lossy());
        println!("{}", file.display());
    }
    for file in &insert_files {
        let _ = control::insert_next(&file.to_string_lossy());
        println!("{}", file.display());
    }
}
