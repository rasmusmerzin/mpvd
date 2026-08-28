use crate::term::term_size;

pub struct ListView {
    /// Offset position.
    pub offset: usize,
    /// Cursor position starting from 0.
    pub cursor: usize,
    /// Render height of view.
    pub height: usize,
    /// Total number of items in list.
    pub count: usize,
}

impl ListView {
    pub fn new(count: usize) -> Self {
        Self {
            offset: 0,
            cursor: 0,
            height: term_size()
                .map(|s| s.height.saturating_sub(1).max(1))
                .unwrap_or(1)
                .into(),
            count,
        }
    }

    pub fn resize(&mut self) {
        self.height = term_size()
            .map(|s| s.height.saturating_sub(1).max(1))
            .unwrap_or(1)
            .into();
    }

    pub fn clamp_scroll(&mut self) {
        if self.count == 0 {
            self.offset = 0;
            self.cursor = 0;
            return;
        }

        let height = self.height.max(1);
        let max_offset = self.count.saturating_sub(height);
        self.offset = self.offset.min(max_offset);
        let max_cursor = self
            .offset
            .saturating_add(height - 1)
            .min(self.count - 1)
            .max(self.offset);
        self.cursor = self.cursor.clamp(self.offset, max_cursor);
    }

    pub fn cursor_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
        if self.cursor < self.offset {
            self.offset = self.cursor;
        }
        self.clamp_scroll();
    }

    pub fn cursor_down(&mut self) {
        if self.cursor < self.count.saturating_sub(1) {
            self.cursor += 1;
        }
        let height = self.height.max(1);
        if self.cursor >= self.offset.saturating_add(height) {
            self.offset = self.cursor + 1 - height;
        }
        self.clamp_scroll();
    }

    pub fn scroll_up(&mut self, amount: usize) {
        self.offset = self.offset.saturating_sub(amount);
        self.clamp_scroll();
    }

    pub fn scroll_down(&mut self, amount: usize) {
        self.offset = self.offset.saturating_add(amount);
        self.clamp_scroll();
    }

    pub fn page_up(&mut self) {
        let delta = self.height / 2;
        self.scroll_up(delta);
        self.cursor = self.cursor.saturating_sub(delta);
        self.clamp_scroll();
    }

    pub fn page_down(&mut self) {
        let delta = self.height / 2;
        self.offset = self.offset.saturating_add(delta);
        self.cursor = self.cursor.saturating_add(delta);
        self.clamp_scroll();
    }

    pub fn go_top(&mut self) {
        self.offset = 0;
        self.cursor = 0;
    }

    pub fn go_bottom(&mut self) {
        if self.count == 0 {
            self.offset = 0;
            self.cursor = 0;
            return;
        }

        self.offset = self.count.saturating_sub(self.height.max(1));
        self.cursor = self.count - 1;
        self.clamp_scroll();
    }

    pub fn cursor_home(&mut self) {
        self.cursor = self.offset;
    }

    pub fn cursor_end(&mut self) {
        if self.count == 0 {
            self.offset = 0;
            self.cursor = 0;
            return;
        }

        self.cursor = self
            .offset
            .saturating_add(self.height.max(1) - 1)
            .min(self.count - 1);
    }
}

#[cfg(test)]
mod tests {
    use super::ListView;

    #[test]
    fn empty_list_navigation_is_safe() {
        let mut view = ListView {
            offset: 0,
            cursor: 0,
            height: 5,
            count: 0,
        };

        view.clamp_scroll();
        view.cursor_up();
        view.cursor_down();
        view.scroll_up(1);
        view.scroll_down(1);
        view.page_up();
        view.page_down();
        view.go_bottom();
        view.cursor_end();

        assert_eq!((view.offset, view.cursor), (0, 0));
    }

    #[test]
    fn list_shorter_than_viewport_stays_at_origin() {
        let mut view = ListView {
            offset: 8,
            cursor: 8,
            height: 10,
            count: 3,
        };

        view.clamp_scroll();

        assert_eq!(view.offset, 0);
        assert_eq!(view.cursor, 2);
    }

    #[test]
    fn page_down_advances_the_cursor_and_keeps_it_visible() {
        let mut view = ListView {
            offset: 0,
            cursor: 0,
            height: 4,
            count: 10,
        };

        view.page_down();

        assert_eq!((view.offset, view.cursor), (2, 2));
    }

    #[test]
    fn scroll_up_at_top_does_not_underflow() {
        let mut view = ListView {
            offset: 0,
            cursor: 0,
            height: 4,
            count: 10,
        };

        view.scroll_up(1);

        assert_eq!((view.offset, view.cursor), (0, 0));
    }
}
