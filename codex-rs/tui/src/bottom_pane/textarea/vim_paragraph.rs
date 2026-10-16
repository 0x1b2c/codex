//! Paragraph motions use empty logical lines; objects also recognize whitespace-only lines.

use super::KillBufferKind;
use super::TextArea;
use super::VimMode;
use super::VimOperator;
use super::VimTextObjectScope;
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

    pub(super) fn vim_paragraph_text_object_range(
        &self,
        scope: VimTextObjectScope,
        count: usize,
    ) -> Option<Range<usize>> {
        self.vim_paragraph_object_selection(scope, count)
            .map(|(range, _)| range)
    }

    fn vim_paragraph_object_selection(
        &self,
        scope: VimTextObjectScope,
        count: usize,
    ) -> Option<(Range<usize>, bool)> {
        let lines = self.vim_paragraph_lines();
        let mut runs: Vec<(Range<usize>, bool)> = Vec::new();
        for (row, line) in lines.iter().enumerate() {
            let blank = self.text[line.clone()].chars().all(char::is_whitespace)
                && !self.elements.iter().any(|element| {
                    element.range.start < line.end && element.range.end > line.start
                });
            if let Some((run, prior_blank)) = runs.last_mut()
                && *prior_blank == blank
            {
                run.end = row + 1;
            } else {
                runs.push((row..row + 1, blank));
            }
        }
        let row = lines.partition_point(|line| line.start <= self.cursor_pos) - 1;
        let current = runs.iter().position(|(run, _)| run.contains(&row))?;
        let mut first = current;
        let last = if scope == VimTextObjectScope::Inner {
            current.checked_add(count.checked_sub(1)?)?
        } else {
            let content = current.checked_add(usize::from(runs[current].1))?;
            let last = content.checked_add(count.checked_sub(1)?.checked_mul(2)?)?;
            // A blank run needs a following content run, including at EOF.
            runs.get(last)?;
            if !runs[current].1 {
                if runs.get(last + 1).is_some() {
                    last + 1
                } else {
                    first = first.saturating_sub(1);
                    last
                }
            } else {
                last
            }
        };
        let end_row = runs.get(last)?.0.end;
        let start = lines[runs[first].0.start].start;
        let end = lines
            .get(end_row)
            .map_or(self.text.len(), |line| line.start);
        Some((start..end, end_row == lines.len()))
    }

    pub(super) fn apply_vim_paragraph_object(
        &mut self,
        operator: VimOperator,
        scope: VimTextObjectScope,
        count: usize,
    ) -> bool {
        let Some((mut range, includes_final_line)) =
            self.vim_paragraph_object_selection(scope, count)
        else {
            return false;
        };
        let mut selected = self.text[range.clone()].to_owned();
        // The final logical line needs a register terminator even when it is empty.
        if includes_final_line {
            selected.push('\n');
        }
        if operator == VimOperator::Yank {
            self.store_kill_buffer(selected, KillBufferKind::Linewise);
            return true;
        }
        let replacement = if operator == VimOperator::Change && !includes_final_line {
            "\n"
        } else {
            ""
        };
        // Deleting the last lines also consumes their preceding separator. A change
        // retains it so the replacement stays on its own logical line.
        if operator == VimOperator::Delete && includes_final_line && range.start > 0 {
            range.start -= 1;
        }
        let start = range.start;
        self.kill_line_range(range);
        self.store_kill_buffer(selected, KillBufferKind::Linewise);
        if !replacement.is_empty() {
            self.insert_str_at(start, replacement);
        }
        self.set_cursor(start);
        if operator == VimOperator::Change {
            self.vim_mode = VimMode::Insert;
        }
        true
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
