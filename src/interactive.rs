use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::{Frame, Terminal};

use crate::control;
use crate::ipc;
use crate::list::ListView;
use crate::pick;
use crate::term::{term_alternate_raw, term_restore};
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
            view: ListView::new(0),
            playlist: Vec::new(),
            paused: false,
            time: 0.0,
            duration: 0.0,
            absolute: false,
        }
    }

    fn current_index(&self) -> Option<usize> {
        self.playlist
            .iter()
            .position(|item| item.current.unwrap_or(false))
    }

    fn render(&self, f: &mut Frame) {
        let area = f.area();

        if self.playlist.is_empty() {
            let empty_msg = Line::from(Span::styled(
                "Playlist is empty. Press p to pick files.",
                Style::default().add_modifier(Modifier::ITALIC).dim(),
            ));
            f.render_widget(
                empty_msg,
                Rect::new(0, 0, area.width, self.view.height as u16),
            );
        } else {
            let items = self.render_lines(area.width as usize);
            let list = Paragraph::new(items);
            f.render_widget(list, Rect::new(0, 0, area.width, self.view.height as u16));
        }

        if let Some(status_line) = self.render_status(area.width as usize) {
            f.render_widget(
                status_line,
                Rect::new(0, self.view.height as u16, area.width, 1),
            );
        }
    }

    fn render_lines(&self, area_width: usize) -> Vec<Line<'_>> {
        self.playlist[self.view.offset..]
            .iter()
            .take(self.view.height)
            .enumerate()
            .map(|(i, item)| {
                let idx = i + self.view.offset;
                let is_hover = idx == self.view.cursor;
                let is_current = item.current.unwrap_or(false);

                let index_str = format!("{:>4} ", idx + 1);
                let cursor = if is_current {
                    if self.paused { "- " } else { "* " }
                } else {
                    "  "
                };

                let name = track_name(item, self.absolute);
                let name_max = area_width.saturating_sub(index_str.len() + cursor.len());
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
            .collect()
    }

    fn render_status(&self, area_width: usize) -> Option<Line<'_>> {
        let current = self.current_index()?;
        let item = self.playlist.get(current)?;

        let index_str = format!("{:>4} ", current + 1);
        let cursor = if self.paused { "- " } else { "* " };
        let time_str = format!(" {}", control::format_time_string(self.time, self.duration));
        let name = track_name(item, self.absolute);
        let name_max = area_width.saturating_sub(index_str.len() + cursor.len() + time_str.len());
        let name_padded = pad_to_width(&name, name_max);

        Some(Line::from(Span::raw(format!(
            "{index_str}{cursor}{name_padded}{time_str}"
        ))))
    }

    fn move_track(&mut self, down: bool) {
        let position = self.view.cursor;
        if down {
            if position + 1 >= self.playlist.len() {
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
            self.playlist = playlist;
        }
        if down {
            self.view.cursor_down();
        } else {
            self.view.cursor_up();
        }
    }

    fn handle_input(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        key: KeyEvent,
    ) -> bool {
        let m = key.modifiers;
        let has_ctrl = m.contains(KeyModifiers::CONTROL);
        let has_shift = m.contains(KeyModifiers::SHIFT);

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return true,
            KeyCode::Char('c') if has_ctrl => return true,
            KeyCode::Char('e') if has_ctrl => self.view.scroll_down(1),
            KeyCode::Char('y') if has_ctrl => self.view.scroll_up(1),
            KeyCode::Char('d') if has_ctrl => self.view.page_down(),
            KeyCode::Char('u') if has_ctrl => self.view.page_up(),
            KeyCode::Char('H') => self.view.cursor_home(),
            KeyCode::Char('L') => self.view.cursor_end(),
            KeyCode::Char('j') if !has_shift => self.view.cursor_down(),
            KeyCode::Char('n') if has_ctrl => self.view.cursor_down(),
            KeyCode::Char('k') if !has_shift => self.view.cursor_up(),
            KeyCode::Char('p') if has_ctrl => self.view.cursor_up(),
            KeyCode::Down if !has_shift => self.view.cursor_down(),
            KeyCode::Up if !has_shift => self.view.cursor_up(),
            KeyCode::Down if has_shift => self.move_track(true),
            KeyCode::Up if has_shift => self.move_track(false),
            KeyCode::Char('J') => self.move_track(true),
            KeyCode::Char('K') => self.move_track(false),
            KeyCode::Char('g') => self.view.go_top(),
            KeyCode::Char('G') => self.view.go_bottom(),
            KeyCode::Char('f') if !has_ctrl => self.absolute = !self.absolute,
            KeyCode::Char('p') => {
                term_restore();
                pick::run(config::DEFAULT_MUSIC_DIR);
                term_alternate_raw();
                if let Ok(playlist) = control::get_playlist() {
                    self.playlist = playlist;
                    self.view.clamp_scroll();
                }
                terminal.clear().ok();
                terminal.draw(|f| self.render(f)).unwrap();
            }
            KeyCode::Char('D') | KeyCode::Delete => {
                let _ = control::remove_from_playlist(self.view.cursor + 1);
                if let Ok(playlist) = control::get_playlist() {
                    self.playlist = playlist;
                }
                self.view.clamp_scroll();
            }
            KeyCode::Char(' ') => {
                let _ = control::set_pause(!self.paused);
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
                if Some(self.view.cursor) == self.current_index() {
                    let _ = control::set_pause(!self.paused);
                } else {
                    let _ = control::play_at_index(self.view.cursor + 1);
                }
            }
            _ => {}
        }
        false
    }
}

pub fn run() {
    daemon::start();
    term_alternate_raw();
    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout())).unwrap();
    let mut state = PlaylistState::new();
    let mut observer = match ipc::Observer::connect() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            term_restore();
            return;
        }
    };

    let playlist_oid = observer.observe("playlist").unwrap();
    let pause_oid = observer.observe("pause").unwrap();
    let time_oid = observer.observe("time-pos").unwrap();
    let duration_oid = observer.observe("duration").unwrap();

    if let Ok(playlist) = control::get_playlist() {
        state.playlist = playlist;
        state.view.count = state.playlist.len();
        if let Some(pos) = state.current_index() {
            state.view.cursor = pos;
            state.view.clamp_scroll();
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
        state.view.resize();
        terminal.draw(|f| state.render(f)).unwrap();

        if event::poll(Duration::from_millis(50)).unwrap()
            && let Event::Key(key) = event::read().unwrap()
            && state.handle_input(&mut terminal, key)
        {
            break;
        }

        for (id, _name, data) in observer.poll() {
            if id == playlist_oid {
                if let Ok(playlist) = serde_json::from_value(data) {
                    state.playlist = playlist;
                    state.view.count = state.playlist.len();
                    state.view.clamp_scroll();
                }
            } else if id == pause_oid {
                state.paused = data.as_bool().unwrap_or(false);
            } else if id == time_oid {
                state.time = data.as_f64().unwrap_or(0.0);
            } else if id == duration_oid {
                state.duration = data.as_f64().unwrap_or(0.0);
            }
        }
    }

    term_restore();
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
