//! Characterwise Visual selections keep a fixed atomic anchor and an inclusive moving end.
//! Navigation reuses Normal-mode motions; edits are consumed until selection operators exist.

use super::TextArea;
use super::VimMode;
use super::VimPending;
use crate::key_hint::KeyBindingListExt;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

impl TextArea {
    pub(crate) fn is_vim_visual_mode(&self) -> bool {
        self.vim_enabled && self.vim_mode == VimMode::Visual
    }

    pub(super) fn enter_vim_visual_mode(&mut self) {
        self.mouse_selection = None;
        self.vim_pending = VimPending::None;
        self.vim_count = None;
        self.vim_operator_count = None;
        self.cancel_vim_search();
        let anchor = self.vim_visual_item_start(self.cursor_pos);
        self.set_cursor(anchor);
        self.vim_visual_anchor = Some(anchor);
        self.vim_mode = VimMode::Visual;
    }

    /// Content replacement and explicit mode transitions must never retain a stale anchor.
    pub(super) fn clear_vim_visual(&mut self) {
        self.vim_visual_anchor = None;
        if self.vim_mode == VimMode::Visual {
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
        if self.vim_normal_keymap.enter_visual.is_pressed(event) {
            self.enter_vim_normal_mode();
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
