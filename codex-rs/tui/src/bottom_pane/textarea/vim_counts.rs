//! Numeric prefixes belong to Normal-mode commands, including pending character searches.

use super::TextArea;
use super::VimOperator;
use super::VimPending;
use super::vim_words::VimWordKind;
use crate::key_hint::KeyBindingListExt;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;

impl TextArea {
    pub(crate) fn wants_vim_count_key(&self, event: KeyEvent) -> bool {
        self.is_vim_normal_mode()
            && matches!(self.vim_pending, VimPending::None)
            && count_digit(event).is_some_and(|digit| {
                if digit == 0 {
                    self.vim_count.is_some()
                } else {
                    !self.has_vim_normal_digit_binding(event)
                }
            })
    }

    fn has_vim_normal_digit_binding(&self, event: KeyEvent) -> bool {
        // Existing bindings take precedence when a user assigns an action to a digit.
        let keys = &self.vim_normal_keymap;
        [
            &keys.enter_insert,
            &keys.append_after_cursor,
            &keys.append_line_end,
            &keys.insert_line_start,
            &keys.open_line_below,
            &keys.open_line_above,
            &keys.enter_replace_mode,
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
            &keys.move_line_end,
            &keys.find_forward,
            &keys.find_backward,
            &keys.till_forward,
            &keys.till_backward,
            &keys.repeat_find,
            &keys.reverse_find,
            &keys.jump_top,
            &keys.jump_bottom,
            &keys.delete_char,
            &keys.replace_char,
            &keys.repeat_last_change,
            &keys.substitute_char,
            &keys.delete_to_line_end,
            &keys.change_to_line_end,
            &keys.yank_line,
            &keys.paste_after,
            &keys.start_delete_operator,
            &keys.start_yank_operator,
            &keys.start_change_operator,
            &keys.undo,
            &keys.redo,
            &keys.cancel_operator,
        ]
        .into_iter()
        .any(|bindings| bindings.is_pressed(event))
    }

    pub(super) fn handle_vim_counted_navigation(&mut self, event: KeyEvent) -> bool {
        if self.wants_vim_count_key(event) {
            let digit = count_digit(event).expect("count digit");
            self.vim_count = Some(
                self.vim_count
                    .unwrap_or(0)
                    .saturating_mul(10)
                    .saturating_add(digit),
            );
            return true;
        }
        let Some(count) = self.vim_count.take() else {
            return false;
        };
        if self.handle_vim_counted_direct_edit(event, count) {
            return true;
        }
        let keys = &self.vim_normal_keymap;
        let operator = [
            (&keys.start_delete_operator, VimOperator::Delete),
            (&keys.start_change_operator, VimOperator::Change),
            (&keys.start_yank_operator, VimOperator::Yank),
        ]
        .into_iter()
        .find_map(|(keys, op)| keys.is_pressed(event).then_some(op));
        if let Some(operator) = operator {
            self.vim_operator_count = Some(count);
            self.vim_pending = VimPending::Operator(operator);
            return true;
        }
        if let Some(reverse) = self.vim_repeat_find_for_event(event, /*operator*/ false) {
            self.repeat_vim_find(reverse, None, count);
            return true;
        }
        if keys.find_forward.is_pressed(event)
            || keys.find_backward.is_pressed(event)
            || keys.till_forward.is_pressed(event)
            || keys.till_backward.is_pressed(event)
        {
            self.vim_count = Some(count);
            self.handle_vim_extra_command(event);
            return true;
        }
        if keys.jump_top.is_pressed(event) || keys.jump_bottom.is_pressed(event) {
            self.jump_to_vim_counted_line(count);
            return true;
        }
        if keys.move_line_end.is_pressed(event) {
            for _ in 1..count.min(self.text.len().saturating_add(1)) {
                let end = self.end_of_current_line();
                if end == self.text.len() {
                    break;
                }
                self.set_cursor(end + 1);
            }
            self.set_cursor(self.vim_line_end_cursor());
            return true;
        }
        let word = [
            (
                &keys.move_word_forward,
                VimWordKind::Word,
                WordDestination::Start,
            ),
            (
                &keys.move_word_backward,
                VimWordKind::Word,
                WordDestination::Previous,
            ),
            (&keys.move_word_end, VimWordKind::Word, WordDestination::End),
            (
                &keys.move_big_word_forward,
                VimWordKind::BigWord,
                WordDestination::Start,
            ),
            (
                &keys.move_big_word_backward,
                VimWordKind::BigWord,
                WordDestination::Previous,
            ),
            (
                &keys.move_big_word_end,
                VimWordKind::BigWord,
                WordDestination::End,
            ),
        ]
        .into_iter()
        .find_map(|(bindings, kind, destination)| {
            bindings.is_pressed(event).then_some((kind, destination))
        });
        if let Some((kind, destination)) = word {
            self.move_vim_counted_word(kind, destination, count);
            return true;
        }
        if keys.move_left.is_pressed(event)
            || keys.move_right.is_pressed(event)
            || keys.move_up.is_pressed(event)
            || keys.move_down.is_pressed(event)
        {
            // Each successful step crosses at least one byte or visual row; stop at the edge.
            for _ in 0..count.min(self.text.len().saturating_add(1)) {
                let before = self.cursor_pos;
                self.handle_vim_normal(event);
                if self.cursor_pos == before {
                    break;
                }
            }
            return true;
        }
        // Mode transitions and line-start retain their existing behavior. Unsupported counts
        // are consumed rather than silently turning a counted edit into a single edit.
        let uncounted = keys.move_line_start.is_pressed(event)
            || keys.enter_insert.is_pressed(event)
            || keys.append_after_cursor.is_pressed(event)
            || keys.append_line_end.is_pressed(event)
            || keys.insert_line_start.is_pressed(event)
            || keys.enter_replace_mode.is_pressed(event);
        !uncounted
    }

    fn jump_to_vim_counted_line(&mut self, count: usize) {
        let mut start = 0;
        for _ in 1..count.min(self.text.len().saturating_add(1)) {
            let end = self.end_of_line(start);
            if end == self.text.len() {
                break;
            }
            start = end + 1;
        }
        self.set_cursor(start);
        self.set_cursor(self.first_non_blank_of_current_line());
    }

    pub(super) fn move_vim_counted_word(
        &mut self,
        kind: VimWordKind,
        destination: WordDestination,
        count: usize,
    ) {
        // Classify the buffer once, even for a saturated numeric prefix.
        let ranges = self.vim_word_ranges(kind);
        let target = match destination {
            WordDestination::Start => ranges
                .iter()
                .filter(|range| range.start > self.cursor_pos)
                .nth(count - 1)
                .map_or(self.text.len(), |range| range.start),
            WordDestination::Previous => ranges
                .iter()
                .rev()
                .filter(|range| range.start < self.cursor_pos)
                .nth(count - 1)
                .map_or(0, |range| range.start),
            WordDestination::End => {
                let target = ranges
                    .iter()
                    .map(|range| self.prev_atomic_boundary(range.end))
                    .filter(|&end| end > self.cursor_pos)
                    .nth(count - 1);
                target.unwrap_or_else(|| {
                    // Preserve the existing e/E fallback at the final grapheme.
                    if self.cursor_pos < self.text.len() {
                        self.prev_atomic_boundary(self.text.len())
                    } else {
                        self.text.len()
                    }
                })
            }
        };
        self.set_cursor(target);
    }
}

#[derive(Clone, Copy)]
pub(super) enum WordDestination {
    Start,
    Previous,
    End,
}

pub(super) fn count_digit(event: KeyEvent) -> Option<usize> {
    match (event.code, event.modifiers) {
        (KeyCode::Char(ch @ '0'..='9'), KeyModifiers::NONE) => Some((ch as u8 - b'0') as usize),
        _ => None,
    }
}

#[cfg(test)]
#[path = "vim_counts_tests.rs"]
mod tests;
