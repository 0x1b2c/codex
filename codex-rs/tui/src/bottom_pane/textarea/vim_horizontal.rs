use super::TextArea;

pub(super) enum VimHorizontalDirection {
    Left,
    Right,
}

pub(super) enum VimHorizontalBoundary {
    Normal,
    /// Operators use an exclusive end so `dl` can include the last item in a line.
    Operator,
}

impl TextArea {
    pub(super) fn vim_horizontal_target(
        &self,
        direction: VimHorizontalDirection,
        boundary: VimHorizontalBoundary,
    ) -> usize {
        let start = self.beginning_of_current_line();
        let end = match boundary {
            VimHorizontalBoundary::Normal => self.vim_line_end_cursor(),
            VimHorizontalBoundary::Operator => self.end_of_current_line(),
        };
        let target = match direction {
            VimHorizontalDirection::Left => self.prev_atomic_boundary(self.cursor_pos),
            VimHorizontalDirection::Right => self.next_atomic_boundary(self.cursor_pos),
        };
        target.clamp(start, end)
    }
}
