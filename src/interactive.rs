use std::io;
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
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::{Frame, Terminal};

use crate::control;
use crate::ipc;
use crate::list::ListView;
use crate::pick;
use crate::{config, daemon};

struct PlaylistState {
    view: ListView,
    playlist: Vec<control::PlaylistItem>,
    paused: bool,
    time: f64,
    duration: f64,
    absolute: bool,
}

impl PlaylistState {
    fn new() -> Self {
        Self {
            view: ListView::new(),
            playlist: Vec::new(),
            paused: false,
            time: 0.0,
            duration: 0.0,
            absolute: false,
        }
    }

    fn clamp_scroll(&mut self, term_height: u16) {
        self.view.clamp_scroll(self.playlist.len(), term_height);
    }

    fn current_index(&self) -> Option<usize> {
        self.playlist
            .iter()
            .position(|item| item.current.unwrap_or(false))
    }

    fn cursor_up(&mut self, term_height: u16) {
        self.view.cursor_up(self.playlist.len(), term_height);
    }

    fn cursor_down(&mut self, term_height: u16) {
        self.view.cursor_down(self.playlist.len(), term_height);
    }

    fn scroll_up(&mut self, amount: usize) {
        self.view.scroll_up(amount);
    }

    fn scroll_down(&mut self, amount: usize, term_height: u16) {
        self.view
            .scroll_down(amount, self.playlist.len(), term_height);
    }

    fn page_up(&mut self, term_height: u16) {
        self.view.page_up(self.playlist.len(), term_height);
    }

    fn page_down(&mut self, term_height: u16) {
        self.view.page_down(self.playlist.len(), term_height);
    }

    fn go_top(&mut self) {
        self.view.go_top();
    }

    fn go_bottom(&mut self, term_height: u16) {
        self.view.go_bottom(self.playlist.len(), term_height);
    }

    fn cursor_home(&mut self) {
        self.view.cursor_home();
    }

    fn cursor_end(&mut self, term_height: u16) {
        self.view.cursor_end(self.playlist.len(), term_height);
    }
}

pub fn run() {
    let started = daemon::start();
    if started == ExitCode::SUCCESS {
        sleep(Duration::from_millis(200));
    }
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
            restore_terminal(&mut terminal);
            return;
        }
    };

    let _ = observer.observe("playlist");
    let _ = observer.observe("pause");
    let _ = observer.observe("time-pos");
    let _ = observer.observe("duration");

    if let Ok(playlist) = control::get_playlist() {
        state.playlist = playlist;
        if let Some(pos) = state.current_index() {
            state.view.cursor = pos;
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

    restore_terminal(&mut terminal);
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) {
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();
}

fn track_name(item: &control::PlaylistItem, absolute: bool) -> String {
    control::display_name(&item.filename, absolute).to_string()
}

fn pad_to_width(text: &str, width: usize) -> String {
    let truncated: String = text.chars().take(width).collect();
    format!("{truncated:<width$}")
}

fn row_style(is_hover: bool, is_current: bool) -> Style {
    if is_hover && is_current {
        Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(Color::Green)
    } else if is_hover {
        Style::default().add_modifier(Modifier::REVERSED)
    } else if is_current {
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    }
}

fn render(f: &mut Frame, state: &PlaylistState, term_height: u16) {
    let area = f.area();
    let list_height = ListView::list_height(term_height);

    if state.playlist.is_empty() {
        let empty_msg = Line::from(Span::styled(
            "Playlist is empty. Press p to pick files.",
            Style::default().add_modifier(Modifier::ITALIC).dim(),
        ));
        f.render_widget(empty_msg, Rect::new(0, 0, area.width, list_height as u16));
    } else {
        let items: Vec<Line> = state.playlist[state.view.offset..]
            .iter()
            .take(list_height)
            .enumerate()
            .map(|(i, item)| {
                let idx = i + state.view.offset;
                let is_hover = idx == state.view.cursor;
                let is_current = item.current.unwrap_or(false);

                let index_str = format!("{:>4} ", idx + 1);
                let cursor = if is_current {
                    if state.paused { "- " } else { "* " }
                } else {
                    "  "
                };

                let name = track_name(item, state.absolute);
                let name_max = (area.width as usize).saturating_sub(index_str.len() + cursor.len());
                let name_padded = pad_to_width(&name, name_max);

                let style = row_style(is_hover, is_current);
                let index_style = if is_hover {
                    style
                } else {
                    Style::default().dim()
                };
                let cursor_style = if is_hover { style } else { Style::default() };

                Line::from(vec![
                    Span::styled(index_str, index_style),
                    Span::styled(cursor, cursor_style),
                    Span::styled(name_padded, style),
                ])
            })
            .collect();

        let list = Paragraph::new(items);
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
        let name = track_name(item, state.absolute);
        let name_max =
            (area.width as usize).saturating_sub(index_str.len() + cursor.len() + time_str.len());
        let name_padded = pad_to_width(&name, name_max);

        let status_line = Line::from(Span::raw(format!(
            "{index_str}{cursor}{name_padded}{time_str}"
        )));
        f.render_widget(status_line, Rect::new(0, list_height as u16, area.width, 1));
    }
}

fn move_track(state: &mut PlaylistState, term_height: u16, down: bool) {
    let position = state.view.cursor;
    if down {
        if position + 1 >= state.playlist.len() {
            return;
        }
        let _ = control::move_in_playlist(position + 1, position + 2);
    } else {
        if position == 0 {
            return;
        }
        let _ = control::move_in_playlist(position + 1, position);
    }
    if let Ok(playlist) = control::get_playlist() {
        state.playlist = playlist;
    }
    if down {
        state.cursor_down(term_height);
    } else {
        state.cursor_up(term_height);
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
        KeyCode::Down if has_shift => move_track(state, term_height, true),
        KeyCode::Up if has_shift => move_track(state, term_height, false),
        KeyCode::Char('J') => move_track(state, term_height, true),
        KeyCode::Char('K') => move_track(state, term_height, false),
        KeyCode::Char('g') => state.go_top(),
        KeyCode::Char('G') => state.go_bottom(term_height),
        KeyCode::Char('f') if !has_ctrl => state.absolute = !state.absolute,
        KeyCode::Char('p') => {
            disable_raw_mode().unwrap();
            execute!(std::io::stdout(), LeaveAlternateScreen).unwrap();
            pick::run(config::DEFAULT_MUSIC_DIR);
            enable_raw_mode().unwrap();
            execute!(std::io::stdout(), EnterAlternateScreen).unwrap();
            if let Ok(playlist) = control::get_playlist() {
                state.playlist = playlist;
                state.clamp_scroll(term_height);
            }
            let _ = terminal.clear();
            let term_height = terminal.size().unwrap().height;
            terminal.draw(|f| render(f, state, term_height)).unwrap();
        }
        KeyCode::Char('D') | KeyCode::Delete => {
            let _ = control::remove_from_playlist(state.view.cursor + 1);
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
            if Some(state.view.cursor) == state.current_index() {
                let _ = control::set_pause(!state.paused);
            } else {
                let _ = control::play_at_index(state.view.cursor + 1);
            }
        }
        _ => {}
    }
    false
}
