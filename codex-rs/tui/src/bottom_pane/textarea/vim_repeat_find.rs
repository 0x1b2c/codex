//! Repeat line-local character searches without changing the remembered direction.

use super::TextArea;
use super::VimOperator;
use super::vim::VimFindMotion;
use super::vim_commands::VimEditTarget;
use crate::key_hint::KeyBindingListExt;
use crossterm::event::KeyEvent;

impl TextArea {
    pub(super) fn vim_repeat_find_for_event(
        &self,
        event: KeyEvent,
        operator: bool,
    ) -> Option<bool> {
        let (same, reverse) = if operator {
            (
                &self.vim_operator_keymap.motion_repeat_find,
                &self.vim_operator_keymap.motion_reverse_find,
            )
        } else {
            (
                &self.vim_normal_keymap.repeat_find,
                &self.vim_normal_keymap.reverse_find,
            )
        };
        if same.is_pressed(event) {
            Some(false)
        } else if reverse.is_pressed(event) {
            Some(true)
        } else {
            None
        }
    }

    pub(super) fn repeat_vim_find(
        &mut self,
        reverse: bool,
        operator: Option<VimOperator>,
        count: usize,
    ) {
        let Some((mut motion, target)) = self.vim_commands.last_find else {
            return;
        };
        if reverse {
            motion = match motion {
                VimFindMotion::Forward => VimFindMotion::Backward,
                VimFindMotion::Backward => VimFindMotion::Forward,
                VimFindMotion::TillForward => VimFindMotion::TillBackward,
                VimFindMotion::TillBackward => VimFindMotion::TillForward,
            };
        }
        if let Some(operator) = operator {
            self.start_vim_counted_operator(
                operator,
                VimEditTarget::RepeatFind { motion, target },
                count,
            );
        } else {
            self.repeat_vim_character_find(motion, None, target, count);
        }
    }

    pub(super) fn repeat_vim_character_find(
        &mut self,
        motion: VimFindMotion,
        operator: Option<VimOperator>,
        target: char,
        count: usize,
    ) -> bool {
        self.find_vim_character_impl(motion, operator, target, count, /*repeated*/ true)
    }
}

#[cfg(test)]
#[path = "vim_repeat_find_tests.rs"]
mod tests;
