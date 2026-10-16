//! Resolve operator counts before mutating text, preserving one register and undo transaction.

use super::KillBufferKind;
use super::TextArea;
use super::VimMode;
use super::VimMotion;
use super::VimOperator;
use super::VimPending;
use super::vim_commands::VimAction;
use super::vim_commands::VimEditTarget;
use super::vim_counts::WordDestination;
use super::vim_counts::count_digit;
use super::vim_words::VimWordKind;
use crate::key_hint::KeyBindingListExt;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use std::ops::Range;

impl TextArea {
    pub(super) fn handle_vim_counted_operator(&mut self, op: VimOperator, event: KeyEvent) -> bool {
        if let Some(digit) = count_digit(event)
            && (self.vim_count.is_some()
                || digit != 0
                    && !self.vim_operator_keymap.motion_match_pair.is_pressed(event)
                    && self.vim_motion_for_event(event).is_none()
                    && self
                        .vim_repeat_find_for_event(event, /*operator*/ true)
                        .is_none())
        {
            self.vim_count = Some(
                self.vim_count
                    .unwrap_or(0)
                    .saturating_mul(10)
                    .saturating_add(digit),
            );
            self.vim_pending = VimPending::Operator(op);
            return true;
        }
        let counted = self.vim_operator_count.is_some() || self.vim_count.is_some();
        let count = self
            .vim_operator_count
            .take()
            .unwrap_or(1)
            .saturating_mul(self.vim_count.take().unwrap_or(1));
        if self.vim_operator_keymap.cancel.is_pressed(event) {
            return true;
        }
        if let Some(scope) = self.vim_text_object_scope_for_event(event) {
            self.vim_count = counted.then_some(count);
            self.vim_pending = VimPending::TextObject {
                operator: op,
                scope,
            };
            return true;
        }
        if let Some(reverse) = self.vim_repeat_find_for_event(event, /*operator*/ true) {
            self.repeat_vim_find(reverse, Some(op), count);
            return true;
        }
        if self.vim_operator_keymap.motion_match_pair.is_pressed(event) {
            self.start_vim_counted_operator(
                op,
                if counted {
                    VimEditTarget::PercentageLine
                } else {
                    VimEditTarget::MatchPair
                },
                count,
            );
            return true;
        }
        let motion = self.vim_motion_for_event(event);
        let line = op == VimOperator::Delete
            && self.vim_operator_keymap.delete_line.is_pressed(event)
            || op == VimOperator::Yank && self.vim_operator_keymap.yank_line.is_pressed(event)
            || op == VimOperator::Change
                && self
                    .vim_normal_keymap
                    .start_change_operator
                    .is_pressed(event);
        if line || matches!(motion, Some(VimMotion::Up | VimMotion::Down)) || counted {
            let target = if line {
                VimEditTarget::Line
            } else if let Some(motion) = motion {
                VimEditTarget::Motion(motion)
            } else {
                // Keep the count while f/F/t/T waits for its literal target, including digits.
                let keys = &self.vim_operator_keymap;
                if keys.motion_find_forward.is_pressed(event)
                    || keys.motion_find_backward.is_pressed(event)
                    || keys.motion_till_forward.is_pressed(event)
                    || keys.motion_till_backward.is_pressed(event)
                {
                    self.vim_count = Some(count);
                    return self.handle_vim_operator_command(op, event);
                }
                if keys.motion_jump_top.is_pressed(event)
                    || keys.motion_jump_bottom.is_pressed(event)
                {
                    VimEditTarget::BufferJump {
                        last: keys.motion_jump_bottom.is_pressed(event),
                    }
                } else {
                    // Unknown input cancels the operator and consumes its count.
                    return true;
                }
            };
            self.start_vim_counted_operator(op, target, count);
            return true;
        }
        false
    }

    pub(super) fn start_vim_counted_operator(
        &mut self,
        op: VimOperator,
        target: VimEditTarget,
        count: usize,
    ) {
        if op == VimOperator::Yank {
            self.apply_vim_counted_target(op, target, count);
            return;
        }
        let target = VimEditTarget::Counted {
            target: Box::new(target),
            count,
        };
        match op {
            VimOperator::Delete => {
                self.start_vim_edit(VimAction::Delete(target));
            }
            VimOperator::Change => {
                self.start_vim_edit(VimAction::Change(target));
            }
            VimOperator::Yank => unreachable!("yanks are handled without recording a change"),
        }
    }

    pub(super) fn apply_vim_counted_target(
        &mut self,
        op: VimOperator,
        target: VimEditTarget,
        count: usize,
    ) -> bool {
        let origin = self.cursor_pos;
        let preferred = self.preferred_col;
        let line_count = matches!(target, VimEditTarget::Line).then_some(count);
        let clamp_delete = matches!(
            target,
            VimEditTarget::Character | VimEditTarget::Motion(VimMotion::LineEnd)
        );
        let (range, linewise) = match target {
            VimEditTarget::MatchPair => return self.apply_vim_matching_pair(op),
            VimEditTarget::PercentageLine => {
                let Some(start) = self.vim_percentage_line(count) else {
                    return false;
                };
                let current = self.current_line_range_with_newline();
                let end = self.end_of_line(start);
                (
                    current.start.min(start)
                        ..current.end.max(end + usize::from(end < self.text.len())),
                    true,
                )
            }
            VimEditTarget::Character => {
                let (range, _) = self.vim_counted_character_range(count);
                if range.is_empty() {
                    if op == VimOperator::Change {
                        self.vim_mode = VimMode::Insert;
                        return true;
                    }
                    return false;
                }
                (range, false)
            }
            VimEditTarget::TextObject { scope, object } => {
                let Some(range) = self.vim_counted_text_object_range(object, scope, count) else {
                    return false;
                };
                self.apply_vim_operator_to_range(op, range);
                return true;
            }
            VimEditTarget::Search(query) => {
                return self.apply_vim_search_count(&query, Some(op), count);
            }
            VimEditTarget::Find { motion, target } => {
                return self.find_vim_character_count(motion, Some(op), target, count);
            }
            VimEditTarget::RepeatFind { motion, target } => {
                return self.repeat_vim_character_find(motion, Some(op), target, count);
            }
            VimEditTarget::Line => (self.vim_counted_lines(count), true),
            VimEditTarget::BufferJump { .. } => {
                let current = self.current_line_range_with_newline();
                let mut start = 0;
                for _ in 1..count.min(self.text.len().saturating_add(1)) {
                    let end = self.end_of_line(start);
                    if end == self.text.len() {
                        break;
                    }
                    start = end + 1;
                }
                let end = self.end_of_line(start);
                (
                    current.start.min(start)
                        ..current.end.max(end + usize::from(end < self.text.len())),
                    true,
                )
            }
            VimEditTarget::Motion(VimMotion::Up) => {
                let current = self.current_line_range_with_newline();
                let mut start = current.start;
                for _ in 0..count.min(self.text.len().saturating_add(1)) {
                    if start == 0 {
                        break;
                    }
                    start = self.beginning_of_line(start - 1);
                }
                if start == current.start {
                    return false;
                }
                (start..current.end, true)
            }
            VimEditTarget::Motion(VimMotion::Down) => {
                if self.end_of_current_line() == self.text.len() {
                    return false;
                }
                (self.vim_counted_lines(count.saturating_add(1)), true)
            }
            VimEditTarget::Motion(motion) => {
                let kind = if matches!(
                    motion,
                    VimMotion::BigWordForward | VimMotion::BigWordBackward | VimMotion::BigWordEnd
                ) {
                    VimWordKind::BigWord
                } else {
                    VimWordKind::Word
                };
                let changing_word = op == VimOperator::Change
                    && matches!(motion, VimMotion::WordForward | VimMotion::BigWordForward)
                    && self.text[origin..]
                        .chars()
                        .next()
                        .is_some_and(|ch| !ch.is_whitespace());
                match motion {
                    VimMotion::WordForward | VimMotion::BigWordForward if !changing_word => {
                        self.move_vim_counted_word(kind, WordDestination::Start, count);
                    }
                    VimMotion::WordBackward | VimMotion::BigWordBackward => {
                        self.move_vim_counted_word(kind, WordDestination::Previous, count);
                    }
                    VimMotion::WordEnd
                    | VimMotion::BigWordEnd
                    | VimMotion::WordForward
                    | VimMotion::BigWordForward => {
                        // The first end may be the character already under the cursor.
                        let ranges = self.vim_word_ranges(kind);
                        let target = ranges
                            .iter()
                            .filter(|range| {
                                if changing_word {
                                    range.end > origin
                                } else {
                                    self.prev_atomic_boundary(range.end) > origin
                                }
                            })
                            .nth(count - 1)
                            .map_or(self.text.len(), |range| range.end);
                        self.set_cursor(target);
                    }
                    VimMotion::LineEnd => {
                        let range = self.vim_counted_lines(count);
                        let end = range.end
                            - usize::from(
                                range.end > range.start && self.text[..range.end].ends_with('\n'),
                            );
                        self.set_cursor(end);
                    }
                    _ => {
                        for _ in 0..count.min(self.text.len().saturating_add(1)) {
                            let target = self.target_for_motion(motion);
                            if target == self.cursor_pos {
                                break;
                            }
                            self.set_cursor(target);
                        }
                    }
                }
                let destination = self.cursor_pos;
                self.cursor_pos = origin;
                self.preferred_col = preferred;
                (origin.min(destination)..origin.max(destination), false)
            }
            _ => return false,
        };
        if range.is_empty() {
            if linewise && op == VimOperator::Change {
                self.store_kill_buffer("\n".to_owned(), KillBufferKind::Linewise);
                self.vim_mode = VimMode::Insert;
                return true;
            }
            return false;
        }
        if !linewise {
            self.apply_vim_operator_to_range(op, range);
            if clamp_delete && op == VimOperator::Delete {
                self.set_cursor(self.cursor_pos.min(self.vim_line_end_cursor()));
            }
        } else {
            match op {
                VimOperator::Delete => self.kill_line_range(range),
                VimOperator::Yank => self.yank_line_range(range),
                VimOperator::Change => {
                    let removed = &self.text[range.clone()];
                    let retain_newline = removed.ends_with('\n')
                        && (range.end < self.text.len()
                            || line_count.is_some_and(|count| {
                                count <= removed.bytes().filter(|byte| *byte == b'\n').count()
                            }));
                    let start = range.start;
                    self.kill_line_range(range);
                    if retain_newline {
                        self.insert_str_at(start, "\n");
                        self.set_cursor(start);
                    }
                    self.vim_mode = VimMode::Insert;
                }
            }
        }
        true
    }

    fn vim_counted_lines(&self, count: usize) -> Range<usize> {
        let start = self.beginning_of_current_line();
        let mut end = start;
        for _ in 0..count.min(self.text.len().saturating_add(1)) {
            let eol = self.end_of_line(end);
            end = eol + usize::from(eol < self.text.len());
            if end == self.text.len() {
                break;
            }
        }
        start..end
    }

    /// Numeric/operator prefixes and yanks never begin an undo snapshot.
    pub(crate) fn vim_pending_key_may_edit(&self, event: KeyEvent) -> bool {
        if self.vim_query().is_some() {
            return event.code == KeyCode::Enter
                && matches!(
                    self.vim_pending,
                    VimPending::Operator(VimOperator::Delete | VimOperator::Change)
                );
        }
        match self.vim_pending {
            VimPending::Operator(VimOperator::Yank) => false,
            VimPending::None => self.vim_counted_direct_key_may_edit(event),
            VimPending::Operator(_) => {
                !self.vim_operator_keymap.cancel.is_pressed(event)
                    && !(count_digit(event).is_some_and(|digit| {
                        self.vim_count.is_some()
                            || digit != 0
                                && !self.vim_operator_keymap.motion_match_pair.is_pressed(event)
                                && self
                                    .vim_repeat_find_for_event(event, /*operator*/ true)
                                    .is_none()
                    }))
                    && self.vim_text_object_scope_for_event(event).is_none()
                    && !self.vim_search_keymap.forward.is_pressed(event)
                    && !self.vim_search_keymap.backward.is_pressed(event)
                    && !self
                        .vim_operator_keymap
                        .motion_find_forward
                        .is_pressed(event)
                    && !self
                        .vim_operator_keymap
                        .motion_find_backward
                        .is_pressed(event)
                    && !self
                        .vim_operator_keymap
                        .motion_till_forward
                        .is_pressed(event)
                    && !self
                        .vim_operator_keymap
                        .motion_till_backward
                        .is_pressed(event)
            }
            VimPending::TextObject { operator, .. } => {
                operator != VimOperator::Yank
                    && !self.vim_text_object_keymap.cancel.is_pressed(event)
                    && self.vim_text_object_for_event(event).is_some()
            }
            VimPending::Find {
                operator: Some(operator),
                ..
            } => operator != VimOperator::Yank,
            VimPending::Replace => true,
            VimPending::Find { operator: None, .. } => false,
        }
    }
}

#[cfg(test)]
#[path = "vim_operator_counts_tests.rs"]
mod tests;
