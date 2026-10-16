//! Paragraph motions use empty logical lines; whitespace and opaque elements remain content.

use super::TextArea;
use super::VimOperator;
use crate::key_hint::KeyBindingListExt;
use crossterm::event::KeyEvent;
use std::ops::Range;

impl TextArea {
    pub(super) fn vim_paragraph_for_event(&self, event: KeyEvent, operator: bool) -> Option<bool> {
        let (forward, backward) = if operator {
            (
                &self.vim_operator_keymap.motion_paragraph_forward,
                &self.vim_operator_keymap.motion_paragraph_backward,
            )
        } else {
            (
                &self.vim_normal_keymap.move_paragraph_forward,
                &self.vim_normal_keymap.move_paragraph_backward,
            )
        };
        if forward.is_pressed(event) {
            Some(true)
        } else {
            backward.is_pressed(event).then_some(false)
        }
    }

    pub(super) fn vim_paragraph_target(&self, forward: bool, count: usize) -> Option<usize> {
        let lines = self.vim_paragraph_lines();
        let mut position = self.cursor_pos;
        // Each step must consume a logical line or reach a buffer edge.
        if count > lines.len().saturating_add(1) {
            return None;
        }
        for _ in 0..count {
            let mut row = lines.partition_point(|line| line.start <= position) - 1;
            let target = if forward {
                while row < lines.len() && lines[row].is_empty() {
                    row += 1;
                }
                while row < lines.len() && !lines[row].is_empty() {
                    row += 1;
                }
                lines.get(row).map_or_else(
                    || {
                        if lines.last().is_some_and(Range::is_empty) {
                            self.text.len()
                        } else {
                            self.prev_atomic_boundary(self.text.len())
                        }
                    },
                    |line| line.start,
                )
            } else {
                while row > 0 && lines[row].is_empty() {
                    row -= 1;
                }
                while row > 0 && !lines[row].is_empty() {
                    row -= 1;
                }
                lines[row].start
            };
            if target == position {
                return None;
            }
            position = target;
        }
        Some(position)
    }

    pub(super) fn vim_paragraph_operator_range(
        &self,
        forward: bool,
        count: usize,
        operator: VimOperator,
    ) -> Option<(Range<usize>, bool)> {
        let origin = self.cursor_pos;
        let target = self.vim_paragraph_target(forward, count).or_else(|| {
            (forward
                && count == 1
                && origin < self.text.len()
                && self.next_atomic_boundary(origin) == self.text.len())
            .then_some(origin)
        })?;
        let start = origin.min(target);
        let mut end = origin.max(target);
        let eof = forward
            && target < self.text.len()
            && self.next_atomic_boundary(target) == self.text.len()
            && self
                .vim_paragraph_lines()
                .last()
                .is_some_and(|line| !line.is_empty());
        if eof {
            // The last character is inclusive when a paragraph reaches EOF.
            end = self.text.len();
        }
        let end_at_line_start = end == self.beginning_of_line(end);
        let start_line = self.beginning_of_line(start);
        let start_before_text = self.text[start_line..start]
            .chars()
            .all(char::is_whitespace);
        if end_at_line_start && start_line < end {
            if start_before_text {
                return Some((start_line..end, true));
            }
            // Exclusive motions ending in column zero preserve the previous newline.
            end = self.prev_atomic_boundary(end);
        }
        if eof
            && operator == VimOperator::Delete
            && start_before_text
            && start_line < self.beginning_of_line(end)
        {
            return Some((start_line..end, true));
        }
        Some((start..end, false))
    }

    fn vim_paragraph_lines(&self) -> Vec<Range<usize>> {
        let mut lines = Vec::new();
        let mut start = 0;
        for (position, byte) in self.text.bytes().enumerate() {
            if byte == b'\n'
                && !self
                    .elements
                    .iter()
                    .any(|element| element.range.contains(&position))
            {
                lines.push(start..position);
                start = position + 1;
            }
        }
        lines.push(start..self.text.len());
        lines
    }
}

#[cfg(test)]
#[path = "vim_paragraph_tests.rs"]
mod tests;
