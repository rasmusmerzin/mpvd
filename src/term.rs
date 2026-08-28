use ratatui::{Terminal, backend::CrosstermBackend, layout::Size};

pub fn term_size() -> Option<Size> {
    let stdout = std::io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend).unwrap();
    terminal.size().ok()
}
