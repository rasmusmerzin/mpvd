pub struct ListView {
    pub offset: usize,
    pub cursor: usize,
}

impl ListView {
    pub fn new() -> Self {
        Self {
            offset: 0,
            cursor: 0,
        }
    }

    pub fn list_height(term_height: u16) -> usize {
        term_height.saturating_sub(1) as usize
    }

    pub fn clamp_scroll(&mut self, len: usize, term_height: u16) {
        let h = Self::list_height(term_height);
        let max_offset = len.saturating_sub(h);
        self.offset = self.offset.min(max_offset);
        let max_cursor = (self.offset + h)
            .saturating_sub(1)
            .min(len.saturating_sub(1))
            .max(self.offset);
        self.cursor = self.cursor.clamp(self.offset, max_cursor);
    }

    pub fn cursor_up(&mut self, len: usize, term_height: u16) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
        if self.cursor < self.offset {
            self.offset = self.cursor;
        }
        self.clamp_scroll(len, term_height);
    }

    pub fn cursor_down(&mut self, len: usize, term_height: u16) {
        if self.cursor + 1 < len {
            self.cursor += 1;
        }
        let h = Self::list_height(term_height);
        if self.cursor >= self.offset + h {
            self.offset = self.cursor + 1 - h;
        }
        self.clamp_scroll(len, term_height);
    }

    pub fn scroll_up(&mut self, amount: usize) {
        self.offset = self.offset.saturating_sub(amount);
    }

    pub fn scroll_down(&mut self, amount: usize, len: usize, term_height: u16) {
        self.offset += amount;
        self.clamp_scroll(len, term_height);
    }

    pub fn page_up(&mut self, len: usize, term_height: u16) {
        let delta = Self::list_height(term_height) / 2;
        self.scroll_up(delta);
        self.cursor = self.cursor.saturating_sub(delta);
        self.clamp_scroll(len, term_height);
    }

    pub fn page_down(&mut self, len: usize, term_height: u16) {
        let delta = Self::list_height(term_height) / 2;
        self.offset += delta;
        self.cursor = self.cursor.saturating_add(delta);
        self.clamp_scroll(len, term_height);
    }

    pub fn go_top(&mut self) {
        self.offset = 0;
        self.cursor = 0;
    }

    pub fn go_bottom(&mut self, len: usize, term_height: u16) {
        self.offset = len.saturating_sub(Self::list_height(term_height));
        self.cursor = len.saturating_sub(1);
        self.clamp_scroll(len, term_height);
    }

    pub fn cursor_home(&mut self) {
        self.cursor = self.offset;
    }

    pub fn cursor_end(&mut self, len: usize, term_height: u16) {
        let h = Self::list_height(term_height);
        self.cursor = (self.offset + h)
            .saturating_sub(1)
            .min(len.saturating_sub(1));
    }
}
