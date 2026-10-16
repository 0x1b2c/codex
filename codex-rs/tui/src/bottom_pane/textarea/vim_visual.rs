//! Visual selections keep a fixed atomic anchor and an inclusive moving end.
//! Navigation reuses Normal-mode motions; d/c/y consume characterwise or whole-line ranges.
//! Dot stores atomic dimensions or selected line count, independent of byte offsets.
//! J joins every touched logical line and returns to Normal mode.

use super::KillBufferKind;
use super::TextArea;
use super::VimMode;
use super::VimOperator;
use super::VimPending;
use super::vim_commands::VimAction;
use super::vim_commands::VimEditTarget;
use crate::key_hint::KeyBindingListExt;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

/// Selection dimensions measured in atomic items or whole lines.
#[derive(Clone, Debug)]
pub(super) enum VimVisualShape {
    SingleLine { width: usize },
    MultiLine { lines: usize, column: usize },
    Linewise { lines: usize },
}

impl TextArea {
    pub(crate) fn is_vim_visual_mode(&self) -> bool {
        self.vim_enabled && matches!(self.vim_mode, VimMode::Visual | VimMode::VisualLine)
    }

    pub(super) fn enter_vim_visual_mode(&mut self, mode: VimMode) {
        self.mouse_selection = None;
        self.vim_pending = VimPending::None;
        self.vim_count = None;
        self.vim_operator_count = None;
        self.cancel_vim_search();
        if !self.is_vim_visual_mode() {
            let anchor = self.vim_visual_item_start(self.cursor_pos);
            self.set_cursor(anchor);
            self.vim_visual_anchor = Some(anchor);
        }
        self.vim_mode = mode;
    }

    /// Content replacement and explicit mode transitions must never retain a stale anchor.
    pub(super) fn clear_vim_visual(&mut self) {
        self.vim_visual_anchor = None;
        if matches!(self.vim_mode, VimMode::Visual | VimMode::VisualLine) {
            self.vim_mode = VimMode::Normal;
        }
    }

    /// Return an exclusive byte range containing both selected endpoints, including elements.
    pub(super) fn vim_visual_range(&self) -> Option<Range<usize>> {
        if !self.is_vim_visual_mode() {
            return None;
        }
        let anchor = self.vim_visual_item_start(self.vim_visual_anchor?);
        let cursor = self.vim_visual_item_start(self.cursor_pos);
        if self.vim_mode == VimMode::VisualLine {
            let start = self.beginning_of_line(anchor.min(cursor));
            let end = self.end_of_line(anchor.max(cursor));
            return Some(self.expand_range_to_element_boundaries(
                start..end + usize::from(end < self.text.len()),
            ));
        }
        Some(anchor.min(cursor)..self.next_atomic_boundary(anchor.max(cursor)))
    }

    fn vim_visual_item_start(&self, position: usize) -> usize {
        let position = position.min(self.text.len());
        if let Some(element) = self
            .elements
            .iter()
            .find(|element| element.range.contains(&position))
        {
            return element.range.start;
        }
        if position == self.text.len() {
            return if self.beginning_of_line(position) < position {
                self.prev_atomic_boundary(position)
            } else {
                position
            };
        }
        self.text
            .grapheme_indices(true)
            .take_while(|(start, _)| *start <= position)
            .last()
            .map_or(0, |(start, _)| start)
    }

    pub(super) fn handle_vim_visual(&mut self, event: KeyEvent) {
        if (event.code == KeyCode::Esc && event.modifiers == KeyModifiers::NONE)
            || self.vim_normal_keymap.cancel_operator.is_pressed(event)
        {
            self.enter_vim_normal_mode();
            return;
        }
        // A find target is literal input, even when it is also a mode or navigation binding.
        if matches!(self.vim_pending, VimPending::Find { .. }) {
            let pending = std::mem::replace(&mut self.vim_pending, VimPending::None);
            self.handle_vim_pending_command(pending, event);
            return;
        }
        let mode = if self.vim_normal_keymap.enter_visual.is_pressed(event) {
            Some(VimMode::Visual)
        } else if self.vim_normal_keymap.enter_visual_line.is_pressed(event) {
            Some(VimMode::VisualLine)
        } else {
            None
        };
        if let Some(mode) = mode {
            if self.vim_mode == mode {
                self.enter_vim_normal_mode();
            } else {
                self.enter_vim_visual_mode(mode);
            }
            return;
        }
        if let Some(operator) = self.vim_visual_operator(event) {
            self.apply_vim_visual_selection(operator);
            return;
        }
        if self.vim_normal_keymap.join_lines.is_pressed(event) {
            self.join_vim_visual_lines();
            return;
        }
        if self.wants_vim_count_key(event) || self.is_vim_visual_navigation(event) {
            self.handle_vim_normal(event);
            // Normal word motions may reach EOF; keep the selected endpoint on its item.
            self.cursor_pos = self.vim_visual_item_start(self.cursor_pos);
            return;
        }
        // Unsupported edits, text objects, and submission keys cannot become Normal commands.
        self.vim_count = None;
        self.vim_operator_count = None;
        self.vim_pending = VimPending::None;
    }

    /// Selection deletion, change, and joining can start a composer undo transaction.
    pub(crate) fn vim_visual_key_may_edit(&self, event: KeyEvent) -> bool {
        matches!(self.vim_pending, VimPending::None)
            && event.code != KeyCode::Esc
            && !self.vim_normal_keymap.cancel_operator.is_pressed(event)
            && !self.vim_normal_keymap.enter_visual.is_pressed(event)
            && !self.vim_normal_keymap.enter_visual_line.is_pressed(event)
            && (self.vim_normal_keymap.join_lines.is_pressed(event)
                || matches!(
                    self.vim_visual_operator(event),
                    Some(VimOperator::Delete | VimOperator::Change)
                ))
    }

    fn vim_visual_operator(&self, event: KeyEvent) -> Option<VimOperator> {
        let keys = &self.vim_normal_keymap;
        [
            (&keys.start_delete_operator, VimOperator::Delete),
            (&keys.start_yank_operator, VimOperator::Yank),
            (&keys.start_change_operator, VimOperator::Change),
        ]
        .into_iter()
        .find_map(|(bindings, operator)| bindings.is_pressed(event).then_some(operator))
    }

    fn apply_vim_visual_selection(&mut self, operator: VimOperator) {
        let Some(range) = self.vim_visual_range() else {
            self.enter_vim_normal_mode();
            return;
        };
        let endpoint = self.vim_visual_item_start(
            self.vim_visual_anchor
                .unwrap_or(self.cursor_pos)
                .max(self.cursor_pos),
        );
        let lines = self.text[range.start..endpoint]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count();
        let shape = if self.vim_mode == VimMode::VisualLine {
            VimVisualShape::Linewise { lines: lines + 1 }
        } else if lines == 0 {
            VimVisualShape::SingleLine {
                width: self.vim_visual_atomic_width(range.start..range.end),
            }
        } else {
            VimVisualShape::MultiLine {
                lines: lines + 1,
                column: self.vim_visual_atomic_width(self.beginning_of_line(endpoint)..range.end),
            }
        };
        self.enter_vim_normal_mode();
        self.set_cursor(range.start);
        match operator {
            VimOperator::Yank => {
                if matches!(shape, VimVisualShape::Linewise { .. }) {
                    let mut selected = self.text[range].to_owned();
                    // The final empty line has no byte of its own but belongs in the register.
                    if endpoint == self.text.len() && self.beginning_of_line(endpoint) == endpoint {
                        selected.push('\n');
                    }
                    self.store_kill_buffer(selected, KillBufferKind::Linewise);
                } else {
                    self.yank_range(range);
                }
            }
            VimOperator::Delete => {
                self.start_vim_edit(VimAction::Delete(VimEditTarget::Visual(shape)));
            }
            VimOperator::Change => {
                self.start_vim_edit(VimAction::Change(VimEditTarget::Visual(shape)));
            }
        }
    }

    fn vim_visual_atomic_width(&self, range: Range<usize>) -> usize {
        let mut position = range.start;
        let mut width = 0;
        while position < range.end {
            position = self.next_atomic_boundary(position);
            width += 1;
        }
        width
    }

    /// Resolve a saved selection at the new cursor, clipping each endpoint to its line.
    pub(super) fn apply_vim_visual_shape(
        &mut self,
        operator: VimOperator,
        shape: VimVisualShape,
    ) -> bool {
        let start = self.vim_visual_item_start(self.cursor_pos);
        let (mut end, width) = match shape {
            VimVisualShape::Linewise { lines } => {
                self.set_cursor(self.beginning_of_line(start));
                return self.apply_vim_counted_target(operator, VimEditTarget::Line, lines);
            }
            VimVisualShape::SingleLine { width } => (start, width),
            VimVisualShape::MultiLine { lines, column } => {
                let mut line = self.beginning_of_line(start);
                for _ in 1..lines.min(self.text.len().saturating_add(1)) {
                    let end = self.end_of_line(line);
                    if end == self.text.len() {
                        // The saved final line is beyond EOF: consume the remaining text.
                        line = end;
                        break;
                    }
                    line = end + 1;
                }
                (line, column)
            }
        };
        let line_end = self.end_of_line(end);
        // An empty line selects its newline, while nonempty lines stop before it.
        let limit = if self.beginning_of_line(end) == line_end {
            self.next_atomic_boundary(line_end)
        } else {
            line_end
        };
        for _ in 0..width.min(self.text.len().saturating_add(1)) {
            if end >= limit {
                break;
            }
            end = self.next_atomic_boundary(end);
        }
        self.set_cursor(start);
        self.apply_vim_operator_to_range(operator, start..end);
        if operator == VimOperator::Delete {
            self.set_cursor(self.cursor_pos.min(self.vim_line_end_cursor()));
        }
        start < end || operator == VimOperator::Change
    }

    fn is_vim_visual_navigation(&self, event: KeyEvent) -> bool {
        let keys = &self.vim_normal_keymap;
        [
            &keys.move_left,
            &keys.move_right,
            &keys.move_up,
            &keys.move_down,
            &keys.move_word_forward,
            &keys.move_word_backward,
            &keys.move_word_end,
            &keys.move_big_word_forward,
            &keys.move_big_word_backward,
            &keys.move_big_word_end,
            &keys.move_line_start,
            &keys.move_first_nonblank,
            &keys.move_line_end,
            &keys.find_forward,
            &keys.find_backward,
            &keys.till_forward,
            &keys.till_backward,
            &keys.repeat_find,
            &keys.reverse_find,
            &keys.match_pair,
            &keys.jump_top,
            &keys.jump_bottom,
        ]
        .into_iter()
        .any(|bindings| bindings.is_pressed(event))
    }
}

#[cfg(test)]
#[path = "vim_visual_tests.rs"]
mod tests;
