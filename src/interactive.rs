use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::{Backend, CrosstermBackend};
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

    fn handle_input<B: Backend>(&mut self, terminal: &mut Terminal<B>, key: KeyEvent) -> bool {
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
                terminal.clear().ok();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control;
    use crate::test_util::{FakeServer, handler_fn, mpv_ok};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::backend::TestBackend;
    use serde_json::{Value, json};

    fn item(name: &str, current: bool) -> control::PlaylistItem {
        control::PlaylistItem {
            filename: name.to_string(),
            current: Some(current),
        }
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    fn shift(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::SHIFT)
    }

    fn term() -> Terminal<TestBackend> {
        Terminal::new(TestBackend::new(80, 24)).unwrap()
    }

    fn three_track() -> Value {
        json!([
            { "filename": "/m/a.mp3", "current": true },
            { "filename": "/m/b.mp3", "current": false },
            { "filename": "/m/c.mp3", "current": false },
        ])
    }

    fn default_handler() -> crate::test_util::MpvHandler {
        handler_fn(|req| {
            let cmd = req
                .get("command")
                .and_then(|c| c.as_array())
                .cloned()
                .unwrap_or_default();
            match cmd.first().and_then(|v| v.as_str()).unwrap_or("") {
                "get_property" => match cmd.get(1).and_then(|v| v.as_str()).unwrap_or("") {
                    "playlist" => mpv_ok(three_track()),
                    "pause" => mpv_ok(json!(false)),
                    "time-pos" => mpv_ok(json!(61.5)),
                    "duration" => mpv_ok(json!(200.0)),
                    _ => mpv_ok(Value::Null),
                },
                _ => mpv_ok(Value::Null),
            }
        })
    }

    fn state_basic() -> PlaylistState {
        let mut s = PlaylistState::new();
        s.playlist = vec![
            item("/m/a.mp3", true),
            item("/m/b.mp3", false),
            item("/m/c.mp3", false),
        ];
        s.view.count = 3;
        s.view.height = 10;
        s
    }

    fn buffer_str(s: &PlaylistState, width: u16, height: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(width, height)).unwrap();
        t.draw(|f| s.render(f)).unwrap();
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
    fn new_state_has_sane_defaults() {
        let s = PlaylistState::new();
        assert!(s.playlist.is_empty());
        assert!(!s.paused);
        assert!(!s.absolute);
        assert_eq!(s.time, 0.0);
        assert_eq!(s.duration, 0.0);
    }

    #[test]
    fn current_index_finds_current_track() {
        let mut s = PlaylistState::new();
        assert_eq!(s.current_index(), None);
        s.playlist = vec![item("/a", false), item("/b", true)];
        assert_eq!(s.current_index(), Some(1));
    }

    #[test]
    fn render_lines_marks_current_and_hover() {
        let mut s = state_basic();
        s.paused = true;
        s.view.cursor = 1;
        let lines = s.render_lines(40);
        let text: Vec<String> = lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|sp| sp.content.as_ref())
                    .collect::<String>()
            })
            .collect();
        assert_eq!(text.len(), 3);
        assert!(text[0].contains("a.mp3"));
        assert!(text[0].contains("-"));
        assert!(text[1].contains("b.mp3"));
    }

    #[test]
    fn render_status_shows_current_track_and_time() {
        let mut s = state_basic();
        s.time = 61.5;
        s.duration = 200.0;
        let line = s.render_status(40).unwrap();
        let text: String = line.spans.iter().map(|sp| sp.content.as_ref()).collect();
        assert!(text.contains("a.mp3"));
        assert!(text.contains("01:01/03:20"));
    }

    #[test]
    fn render_status_none_when_playlist_empty() {
        let s = PlaylistState::new();
        assert!(s.render_status(40).is_none());
    }

    #[test]
    fn render_prompt_when_empty() {
        let mut s = PlaylistState::new();
        s.view.height = 4;
        assert!(buffer_str(&s, 40, 5).contains("Playlist is empty"));
    }

    #[test]
    fn render_lists_tracks() {
        let mut s = state_basic();
        s.view.height = 3;
        let out = buffer_str(&s, 40, 5);
        assert!(out.contains("a.mp3"));
        assert!(out.contains("b.mp3"));
        assert!(out.contains("c.mp3"));
    }

    #[test]
    fn pad_to_width_pads_and_truncates() {
        assert_eq!(pad_to_width("ab", 5), "ab   ");
        assert_eq!(pad_to_width("abcdef", 3), "abc");
        assert_eq!(pad_to_width("", 2), "  ");
    }

    #[test]
    fn row_style_combinations() {
        let s = row_style(true, true);
        assert!(s.add_modifier.contains(Modifier::REVERSED));
        assert_eq!(s.fg, Some(Color::Green));
        let s = row_style(true, false);
        assert!(s.add_modifier.contains(Modifier::REVERSED));
        assert_eq!(s.fg, None);
        let s = row_style(false, true);
        assert_eq!(s.fg, Some(Color::Green));
        let s = row_style(false, false);
        assert_eq!(s.fg, None);
        assert!(!s.add_modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn track_name_resolves_display_name() {
        let it = item("/m/a.mp3", false);
        assert_eq!(track_name(&it, false), "a.mp3");
        assert_eq!(track_name(&it, true), "/m/a.mp3");
    }

    #[test]
    fn handle_input_quit_keys() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            let mut t = term();
            let mut s = state_basic();
            assert!(s.handle_input(&mut t, key(KeyCode::Char('q'))));
            assert!(s.handle_input(&mut t, key(KeyCode::Esc)));
            assert!(s.handle_input(&mut t, ctrl(KeyCode::Char('c'))));
        });
    }

    #[test]
    fn handle_input_navigation() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            let mut t = term();
            let mut s = state_basic();
            s.view.count = 10;
            s.view.height = 3;
            s.handle_input(&mut t, ctrl(KeyCode::Char('e')));
            assert_eq!(s.view.offset, 1);
            s.handle_input(&mut t, ctrl(KeyCode::Char('y')));
            assert_eq!(s.view.offset, 0);
            s.view.cursor = 6;
            s.view.offset = 6;
            s.handle_input(&mut t, ctrl(KeyCode::Char('u')));
            assert_eq!(s.view.cursor, 6 - (s.view.height / 2));
            s.handle_input(&mut t, ctrl(KeyCode::Char('d')));
            assert!(s.view.cursor >= 6 - (s.view.height / 2));
            s.view.go_top();
            s.handle_input(&mut t, key(KeyCode::Char('g')));
            assert_eq!((s.view.offset, s.view.cursor), (0, 0));
            s.handle_input(&mut t, key(KeyCode::Char('G')));
            assert_eq!(s.view.cursor, 9);
            s.handle_input(&mut t, key(KeyCode::Char('H')));
            assert_eq!(s.view.cursor, 7);
            s.handle_input(&mut t, key(KeyCode::Char('L')));
            assert_eq!(s.view.cursor, 9);
        });
    }

    #[test]
    fn handle_input_cursor_moves() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            let mut t = term();
            let mut s = state_basic();
            assert!(!s.handle_input(&mut t, key(KeyCode::Char('j'))));
            assert_eq!(s.view.cursor, 1);
            s.handle_input(&mut t, key(KeyCode::Down));
            assert_eq!(s.view.cursor, 2);
            s.handle_input(&mut t, ctrl(KeyCode::Char('n')));
            assert_eq!(s.view.cursor, 2);
            s.handle_input(&mut t, key(KeyCode::Char('k')));
            assert_eq!(s.view.cursor, 1);
            s.handle_input(&mut t, key(KeyCode::Up));
            assert_eq!(s.view.cursor, 0);
            s.handle_input(&mut t, ctrl(KeyCode::Char('p')));
            assert_eq!(s.view.cursor, 0);
        });
    }

    #[test]
    fn handle_input_f_toggles_absolute() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            let mut t = term();
            let mut s = state_basic();
            s.handle_input(&mut t, key(KeyCode::Char('f')));
            assert!(s.absolute);
        });
    }

    #[test]
    fn handle_input_space_sends_pause() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            let mut t = term();
            let mut s = state_basic();
            s.handle_input(&mut t, key(KeyCode::Char(' ')));
            let cmds: Vec<Value> = server
                .received
                .try_iter()
                .filter_map(|r| r.get("command").cloned())
                .collect();
            assert!(
                cmds.iter()
                    .any(|c| c.get(0) == Some(&json!("set_property")))
            );
        });
    }

    #[test]
    fn handle_input_move_track_down_and_up() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            let mut t = term();
            let mut s = state_basic();
            s.view.cursor = 1;
            s.handle_input(&mut t, shift(KeyCode::Down));
            assert_eq!(s.view.cursor, 2);
            s.handle_input(&mut t, shift(KeyCode::Up));
            assert_eq!(s.view.cursor, 1);
            s.handle_input(&mut t, key(KeyCode::Char('J')));
            assert_eq!(s.view.cursor, 2);
            s.handle_input(&mut t, key(KeyCode::Char('K')));
            assert_eq!(s.view.cursor, 1);
        });
    }

    #[test]
    fn handle_input_move_track_boundaries() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            let mut t = term();
            let mut s = state_basic();
            s.view.cursor = 0;
            s.handle_input(&mut t, shift(KeyCode::Up));
            assert_eq!(s.view.cursor, 0);
            s.view.cursor = 2;
            s.handle_input(&mut t, shift(KeyCode::Down));
            assert_eq!(s.view.cursor, 2);
        });
    }

    #[test]
    fn handle_input_enter_plays_or_toggles() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            let mut t = term();
            let mut s = state_basic();
            s.view.cursor = 0;
            s.handle_input(&mut t, key(KeyCode::Enter));
            s.view.cursor = 1;
            s.handle_input(&mut t, key(KeyCode::Enter));
            let cmds: Vec<Value> = server
                .received
                .try_iter()
                .filter_map(|r| r.get("command").cloned())
                .collect();
            assert!(
                cmds.iter()
                    .any(|c| c.get(0) == Some(&json!("set_property")))
            );
            assert!(
                cmds.iter()
                    .any(|c| c.get(0) == Some(&json!("playlist-play-index")))
            );
        });
    }

    #[test]
    fn handle_input_seek_keys() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            let mut t = term();
            let mut s = state_basic();
            s.handle_input(&mut t, ctrl(KeyCode::Left));
            s.handle_input(&mut t, ctrl(KeyCode::Right));
            s.handle_input(&mut t, ctrl(KeyCode::Char('b')));
            s.handle_input(&mut t, ctrl(KeyCode::Char('f')));
            let cmds: Vec<Value> = server
                .received
                .try_iter()
                .filter_map(|r| r.get("command").cloned())
                .collect();
            let seeks: Vec<f64> = cmds
                .iter()
                .filter(|c| c.get(0) == Some(&json!("seek")))
                .filter_map(|c| c.get(1).and_then(|v| v.as_f64()))
                .collect();
            assert_eq!(seeks, vec![-5.0, 5.0, -5.0, 5.0]);
        });
    }

    #[test]
    fn handle_input_remove_track() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            let mut t = term();
            let mut s = state_basic();
            s.view.cursor = 1;
            s.handle_input(&mut t, key(KeyCode::Char('D')));
            let cmds: Vec<Value> = server
                .received
                .try_iter()
                .filter_map(|r| r.get("command").cloned())
                .collect();
            assert!(
                cmds.iter()
                    .any(|c| c.get(0) == Some(&json!("playlist-remove")))
            );
        });
    }

    #[test]
    fn handle_input_unknown_key_is_harmless() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            let mut t = term();
            let mut s = state_basic();
            assert!(!s.handle_input(&mut t, key(KeyCode::Char('x'))));
        });
    }

    #[test]
    fn handle_input_delete_keycode_removes() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            let mut t = term();
            let mut s = state_basic();
            s.handle_input(&mut t, key(KeyCode::Delete));
            let cmds: Vec<Value> = server
                .received
                .try_iter()
                .filter_map(|r| r.get("command").cloned())
                .collect();
            assert!(
                cmds.iter()
                    .any(|c| c.get(0) == Some(&json!("playlist-remove")))
            );
        });
    }
}
