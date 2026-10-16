//! Join logical lines using Vim's default spacing without rewriting atomic elements.

use super::TextArea;
use super::vim_commands::VimAction;

impl TextArea {
    pub(super) fn join_vim_lines(&mut self, count: usize) -> bool {
        let mut joined = false;
        for _ in 1..count.max(2).min(self.text.len().saturating_add(1)) {
            let end = self.end_of_current_line();
            if end == self.text.len() {
                break;
            }
            let mut next = end + 1;
            while next < self.text.len() {
                let boundary = self.next_atomic_boundary(next);
                if !matches!(&self.text[next..boundary], " " | "\t")
                    || self
                        .elements
                        .iter()
                        .any(|element| element.range.contains(&next))
                {
                    break;
                }
                next = boundary;
            }
            // An element may itself span lines. Never consume any part of it to join.
            if self
                .elements
                .iter()
                .any(|element| element.range.start < next && element.range.end > end)
            {
                break;
            }
            let left = &self.text[self.beginning_of_current_line()..end];
            let right = &self.text[next..self.end_of_line(next)];
            let separator = if left.is_empty()
                || right.is_empty()
                || right.starts_with(')')
                || left.ends_with('\t')
            {
                ""
            } else if [". ", "! ", "? "]
                .iter()
                .any(|suffix| left.ends_with(*suffix))
            {
                " "
            } else if left.ends_with(' ') {
                ""
            } else if left.ends_with(['.', '!', '?']) {
                "  "
            } else {
                " "
            };
            self.replace_range(end..next, separator);
            self.set_cursor(end.min(self.vim_line_end_cursor()));
            joined = true;
        }
        joined
    }

    pub(super) fn join_vim_visual_lines(&mut self) {
        let Some(anchor) = self.vim_visual_anchor else {
            self.enter_vim_normal_mode();
            return;
        };
        let start = self.beginning_of_line(anchor.min(self.cursor_pos));
        let end = self.beginning_of_line(anchor.max(self.cursor_pos));
        let count = self.text[start..end]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count()
            + 1;
        self.enter_vim_normal_mode();
        self.set_cursor(start);
        self.start_vim_edit(VimAction::JoinLines { count });
    }
}
