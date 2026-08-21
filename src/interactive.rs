use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::{Frame, Terminal};

use crate::control;
use crate::ipc;
use crate::pick;

struct PlaylistState {
    playlist: Vec<control::PlaylistItem>,
    offset: usize,
    cursor: usize,
    paused: bool,
    time: f64,
    duration: f64,
    absolute: bool,
}

impl PlaylistState {
    fn new() -> Self {
        Self {
            playlist: Vec::new(),
            offset: 0,
            cursor: 0,
            paused: false,
            time: 0.0,
            duration: 0.0,
            absolute: false,
        }
    }

    fn list_height(&self, term_height: u16) -> usize {
        term_height.saturating_sub(1) as usize
    }

    fn clamp_scroll(&mut self, term_height: u16) {
        let h = self.list_height(term_height);
        let max_offset = self.playlist.len().saturating_sub(h);
        self.offset = self.offset.min(max_offset);
        let min_cursor = self.offset;
        let max_cursor = (self.offset + h)
            .saturating_sub(1)
            .min(self.playlist.len().saturating_sub(1));
        self.cursor = self.cursor.clamp(min_cursor, max_cursor);
    }

    fn current_index(&self) -> Option<usize> {
        self.playlist
            .iter()
            .position(|item| item.current.unwrap_or(false))
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
        if self.cursor + 1 < self.playlist.len() {
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
            .playlist
            .len()
            .saturating_sub(self.list_height(term_height));
        self.cursor = self.playlist.len().saturating_sub(1);
        self.clamp_scroll(term_height);
    }

    fn cursor_home(&mut self) {
        self.cursor = self.offset;
    }

    fn cursor_end(&mut self, term_height: u16) {
        let h = self.list_height(term_height);
        self.cursor = (self.offset + h)
            .saturating_sub(1)
            .min(self.playlist.len().saturating_sub(1));
    }
}

pub fn run() {
    enable_raw_mode().unwrap();
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).unwrap();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut state = PlaylistState::new();

    let mut observer = match ipc::Observer::connect() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            disable_raw_mode().unwrap();
            execute!(terminal.backend_mut(), LeaveAlternateScreen).unwrap();
            terminal.show_cursor().unwrap();
            return;
        }
    };

    let _ = observer.observe(1, "playlist");
    let _ = observer.observe(2, "pause");
    let _ = observer.observe(3, "time-pos");
    let _ = observer.observe(4, "duration");

    if let Ok(playlist) = control::get_playlist() {
        state.playlist = playlist;
        if let Some(pos) = state.current_index() {
            state.cursor = pos;
            let term_height = terminal.size().unwrap().height;
            state.clamp_scroll(term_height);
        }
    }
    if let Ok(p) = control::get_pause() {
        state.paused = p;
    }
    if let Ok(t) = control::get_time() {
        state.time = t;
    }
    if let Ok(d) = control::get_duration() {
        state.duration = d;
    }

    loop {
        let term_height = terminal.size().unwrap().height;
        terminal.draw(|f| render(f, &state, term_height)).unwrap();

        if event::poll(Duration::from_millis(50)).unwrap()
            && let Event::Key(key) = event::read().unwrap()
            && handle_input(&mut state, &mut terminal, key, term_height)
        {
            break;
        }

        for (id, _name, data) in observer.poll() {
            match id {
                1 => {
                    if let Ok(playlist) = serde_json::from_value(data) {
                        state.playlist = playlist;
                    }
                }
                2 => state.paused = data.as_bool().unwrap_or(false),
                3 => state.time = data.as_f64().unwrap_or(0.0),
                4 => state.duration = data.as_f64().unwrap_or(0.0),
                _ => {}
            }
        }
    }

    disable_raw_mode().unwrap();
    execute!(terminal.backend_mut(), LeaveAlternateScreen).unwrap();
    terminal.show_cursor().unwrap();
}

fn render(f: &mut Frame, state: &PlaylistState, term_height: u16) {
    let area = f.area();
    let list_height = state.list_height(term_height);

    if state.playlist.is_empty() {
        let empty_msg = Line::from(Span::styled(
            "Playlist is empty. Press p to pick files.",
            Style::default().add_modifier(Modifier::ITALIC).dim(),
        ));
        f.render_widget(empty_msg, Rect::new(0, 0, area.width, list_height as u16));
    } else {
        let items: Vec<Line> = state.playlist[state.offset..]
            .iter()
            .take(list_height)
            .enumerate()
            .map(|(i, item)| {
                let idx = i + state.offset;
                let is_hover = idx == state.cursor;
                let is_current = item.current.unwrap_or(false);

                let index_str = format!("{:>4} ", idx + 1);
                let cursor = if is_current {
                    if state.paused { "- " } else { "* " }
                } else {
                    "  "
                };

                let name = if state.absolute {
                    item.filename.clone()
                } else {
                    item.filename
                        .rsplit('/')
                        .next()
                        .unwrap_or(&item.filename)
                        .to_string()
                };

                let name_max = area.width as usize - index_str.len() - cursor.len();
                let name_display: String = name.chars().take(name_max).collect();
                let name_padded = format!("{name_display:<width$}", width = name_max);

                let index_style = if is_hover && is_current {
                    Style::default()
                        .add_modifier(Modifier::REVERSED)
                        .fg(Color::Green)
                } else if is_hover {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default().dim()
                };

                let cursor_style = if is_hover && is_current {
                    Style::default()
                        .add_modifier(Modifier::REVERSED)
                        .fg(Color::Green)
                } else if is_hover {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else if is_current {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default()
                };

                let name_style = if is_hover && is_current {
                    Style::default()
                        .add_modifier(Modifier::REVERSED)
                        .fg(Color::Green)
                } else if is_hover {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else if is_current {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default()
                };

                Line::from(vec![
                    Span::styled(index_str, index_style),
                    Span::styled(cursor, cursor_style),
                    Span::styled(name_padded, name_style),
                ])
            })
            .collect();

        let list = ratatui::widgets::Paragraph::new(items);
        f.render_widget(list, Rect::new(0, 0, area.width, list_height as u16));
    }

    if let Some(current) = state.current_index()
        && let Some(item) = state.playlist.get(current)
    {
        let index_str = format!("{:>4} ", current + 1);
        let cursor = if state.paused { "- " } else { "* " };
        let time_str = format!(
            " {}",
            control::format_time_string(state.time, state.duration)
        );

        let name = if state.absolute {
            item.filename.clone()
        } else {
            item.filename
                .rsplit('/')
                .next()
                .unwrap_or(&item.filename)
                .to_string()
        };

        let name_max = area.width as usize - index_str.len() - cursor.len() - time_str.len();
        let name_display: String = name.chars().take(name_max).collect();
        let name_padded = format!("{name_display:<width$}", width = name_max);

        let status_line = Line::from(Span::raw(format!(
            "{index_str}{cursor}{name_padded}{time_str}"
        )));
        f.render_widget(status_line, Rect::new(0, list_height as u16, area.width, 1));
    }
}

fn handle_input(
    state: &mut PlaylistState,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    key: KeyEvent,
    term_height: u16,
) -> bool {
    let m = key.modifiers;
    let has_ctrl = m.contains(KeyModifiers::CONTROL);
    let has_shift = m.contains(KeyModifiers::SHIFT);

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => return true,
        KeyCode::Char('c') if has_ctrl => return true,
        KeyCode::Char('e') if has_ctrl => state.scroll_down(1, term_height),
        KeyCode::Char('y') if has_ctrl => state.scroll_up(1),
        KeyCode::Char('d') if has_ctrl => state.page_down(term_height),
        KeyCode::Char('u') if has_ctrl => state.page_up(term_height),
        KeyCode::Char('H') => state.cursor_home(),
        KeyCode::Char('L') => state.cursor_end(term_height),
        KeyCode::Char('j') if !has_shift => state.cursor_down(term_height),
        KeyCode::Char('n') if has_ctrl => state.cursor_down(term_height),
        KeyCode::Char('k') if !has_shift => state.cursor_up(term_height),
        KeyCode::Char('p') if has_ctrl => state.cursor_up(term_height),
        KeyCode::Down if !has_shift => state.cursor_down(term_height),
        KeyCode::Up if !has_shift => state.cursor_up(term_height),
        KeyCode::Down if has_shift => {
            let position = state.cursor;
            if position >= state.playlist.len() {
                return false;
            }
            let _ = control::move_in_playlist(position + 1, position + 2);
            if let Ok(playlist) = control::get_playlist() {
                state.playlist = playlist;
            }
            state.cursor_down(term_height);
        }
        KeyCode::Up if has_shift => {
            let position = state.cursor;
            if position < 1 {
                return false;
            }
            let _ = control::move_in_playlist(position + 1, position);
            if let Ok(playlist) = control::get_playlist() {
                state.playlist = playlist;
            }
            state.cursor_up(term_height);
        }
        KeyCode::Char('J') => {
            let position = state.cursor;
            if position >= state.playlist.len() {
                return false;
            }
            let _ = control::move_in_playlist(position + 1, position + 2);
            if let Ok(playlist) = control::get_playlist() {
                state.playlist = playlist;
            }
            state.cursor_down(term_height);
        }
        KeyCode::Char('K') => {
            let position = state.cursor;
            if position < 1 {
                return false;
            }
            let _ = control::move_in_playlist(position + 1, position);
            if let Ok(playlist) = control::get_playlist() {
                state.playlist = playlist;
            }
            state.cursor_up(term_height);
        }
        KeyCode::Char('g') => state.go_top(),
        KeyCode::Char('G') => state.go_bottom(term_height),
        KeyCode::Char('f') if !has_ctrl => state.absolute = !state.absolute,
        KeyCode::Char('p') => {
            disable_raw_mode().unwrap();
            execute!(std::io::stdout(), LeaveAlternateScreen).unwrap();
            pick::run("~/Music");
            enable_raw_mode().unwrap();
            execute!(std::io::stdout(), EnterAlternateScreen).unwrap();
            if let Ok(playlist) = control::get_playlist() {
                state.playlist = playlist;
                state.clamp_scroll(term_height);
            }
            let term_height = terminal.size().unwrap().height;
            let _ = terminal.clear();
            terminal.draw(|f| render(f, &state, term_height)).unwrap();
        }
        KeyCode::Char('D') | KeyCode::Delete => {
            let position = state.cursor + 1;
            let _ = control::remove_from_playlist(position);
            if let Ok(playlist) = control::get_playlist() {
                state.playlist = playlist;
            }
            state.clamp_scroll(term_height);
        }
        KeyCode::Char(' ') => {
            let _ = control::set_pause(!state.paused);
        }
        KeyCode::Left if has_ctrl => {
            let _ = control::seek(-5.0);
        }
        KeyCode::Right if has_ctrl => {
            let _ = control::seek(5.0);
        }
        KeyCode::Char('b') if has_ctrl => {
            let _ = control::seek(-5.0);
        }
        KeyCode::Char('f') if has_ctrl => {
            let _ = control::seek(5.0);
        }
        KeyCode::Enter => {
            let position = state.cursor + 1;
            if Some(state.cursor) == state.current_index() {
                let _ = control::set_pause(!state.paused);
            } else {
                let _ = control::play_at_index(position);
            }
        }
        _ => {}
    }
    false
}
