//! Direct edit prefixes resolve to one bounded transaction, including paste and replacement.

use super::KillBufferKind;
use super::TextArea;
use super::VimMotion;
use super::VimOperator;
use super::VimPending;
use super::vim_commands::VimAction;
use super::vim_commands::VimEditTarget;
use crate::key_hint::KeyBindingListExt;
use codex_protocol::user_input::MAX_USER_INPUT_TEXT_CHARS;
use crossterm::event::KeyEvent;
use std::ops::Range;

impl TextArea {
    pub(super) fn handle_vim_counted_direct_edit(&mut self, event: KeyEvent, count: usize) -> bool {
        let keys = &self.vim_normal_keymap;
        let edit = [
            (
                &keys.delete_char,
                VimOperator::Delete,
                VimEditTarget::Character,
            ),
            (
                &keys.substitute_char,
                VimOperator::Change,
                VimEditTarget::Character,
            ),
            (
                &keys.delete_to_line_end,
                VimOperator::Delete,
                VimEditTarget::Motion(VimMotion::LineEnd),
            ),
            (
                &keys.change_to_line_end,
                VimOperator::Change,
                VimEditTarget::Motion(VimMotion::LineEnd),
            ),
            (&keys.yank_line, VimOperator::Yank, VimEditTarget::Line),
        ]
        .into_iter()
        .find_map(|(keys, operator, target)| keys.is_pressed(event).then_some((operator, target)));
        if let Some((operator, target)) = edit {
            self.start_vim_counted_operator(operator, target, count);
            return true;
        }
        if keys.replace_char.is_pressed(event) {
            if self.cursor_pos < self.end_of_current_line() {
                self.vim_count = Some(count);
                self.vim_pending = VimPending::Replace;
            }
            return true;
        }
        if keys.paste_after.is_pressed(event) {
            self.start_vim_edit(VimAction::PasteAfter { count });
            return true;
        }
        if keys.repeat_last_change.is_pressed(event) {
            self.repeat_vim_change(Some(count));
            return true;
        }
        false
    }

    pub(super) fn vim_counted_direct_key_may_edit(&self, event: KeyEvent) -> bool {
        let keys = &self.vim_normal_keymap;
        keys.delete_char.is_pressed(event)
            || keys.substitute_char.is_pressed(event)
            || keys.delete_to_line_end.is_pressed(event)
            || keys.change_to_line_end.is_pressed(event)
            || keys.paste_after.is_pressed(event)
            || keys.repeat_last_change.is_pressed(event)
    }

    pub(super) fn vim_counted_character_range(&self, count: usize) -> (Range<usize>, usize) {
        let start = self.cursor_pos;
        let eol = self.end_of_current_line();
        let mut end = start;
        let mut found = 0;
        while found < count && end < eol {
            let next = self.next_atomic_boundary(end);
            // An element spanning lines is never split by a line-local command.
            if next > eol || next <= end {
                break;
            }
            end = next;
            found += 1;
        }
        (start..end, found)
    }

    pub(super) fn replace_vim_characters(&mut self, ch: char, count: usize) -> bool {
        let (range, found) = self.vim_counted_character_range(count);
        if found != count || range.is_empty() {
            return false;
        }
        // Vim replaces a counted range with one line break when the target is Enter.
        let repetitions = if ch == '\n' { 1 } else { count };
        let Some(bytes) = repetitions.checked_mul(ch.len_utf8()) else {
            return false;
        };
        if bytes > MAX_USER_INPUT_TEXT_CHARS * 4 {
            return false;
        }
        let replacement = ch.to_string().repeat(repetitions);
        let start = range.start;
        self.replace_range(range, &replacement);
        self.set_cursor(if ch == '\n' {
            start + 1
        } else {
            self.prev_atomic_boundary(start + replacement.len())
        });
        true
    }

    pub(super) fn paste_vim_after(&mut self, count: usize) -> bool {
        if self.kill_buffer.is_empty() {
            return false;
        }
        let linewise = self.kill_buffer_kind == KillBufferKind::Linewise && !self.single_line;
        let mut unit = self.filter_line_breaks(&self.kill_buffer).into_owned();
        if unit.is_empty() {
            return false;
        }
        if linewise && !unit.ends_with('\n') {
            unit.push('\n');
        }
        let Some(bytes) = unit.len().checked_mul(count) else {
            return false;
        };
        // Bound allocation before repeat; saturated counts cannot consume unbounded memory.
        if self.text.len().saturating_add(bytes) > MAX_USER_INPUT_TEXT_CHARS * 4
            || self
                .text
                .chars()
                .count()
                .saturating_add(unit.chars().count().saturating_mul(count))
                > MAX_USER_INPUT_TEXT_CHARS
        {
            return false;
        }
        let mut text = unit.repeat(count);
        if linewise {
            let eol = self.end_of_current_line();
            let insert_at = if eol < self.text.len() { eol + 1 } else { eol };
            let start = if eol < self.text.len() {
                insert_at
            } else {
                // Move exactly one terminal newline before the pasted lines, retaining blanks.
                text.pop();
                text.insert(0, '\n');
                insert_at + 1
            };
            self.insert_str_at(insert_at, &text);
            self.set_cursor(start);
            self.set_cursor(
                self.first_non_blank_of_current_line()
                    .min(self.vim_line_end_cursor()),
            );
        } else {
            let insert_at = if self.cursor_pos < self.end_of_current_line() {
                self.next_atomic_boundary(self.cursor_pos)
            } else {
                self.cursor_pos
            };
            self.insert_str_at(insert_at, &text);
            self.set_cursor(if text.contains('\n') {
                insert_at
            } else {
                self.prev_atomic_boundary(insert_at + text.len())
            });
        }
        true
    }
}

#[cfg(test)]
#[path = "vim_direct_counts_tests.rs"]
mod tests;
