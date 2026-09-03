use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::{Terminal, backend::CrosstermBackend, layout::Size};

pub fn term_size() -> Option<Size> {
    let stdout = std::io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend).unwrap();
    terminal.size().ok()
}

pub fn term_alternate_raw() {
    enable_raw_mode().ok();
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen).ok();
}

pub fn term_restore() {
    disable_raw_mode().ok();
    let mut stdout = std::io::stdout();
    execute!(stdout, LeaveAlternateScreen).ok();
}
