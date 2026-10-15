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
    /// Vim's ^ ignores its count and treats only spaces and tabs as indentation.
    pub(super) fn vim_first_nonblank(&self) -> usize {
        let mut position = self.beginning_of_current_line();
        let end = self.end_of_current_line();
        while position < end {
            let next = self.next_atomic_boundary(position);
            if next > end
                || !self.text[position..next]
                    .chars()
                    .all(|ch| matches!(ch, ' ' | '\t'))
            {
                return position;
            }
            position = next;
        }
        self.vim_line_end_cursor()
    }

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
