//! Session registers share the editor kill buffer for unnamed access. Explicit named writes
//! update that buffer, while only unnamed yanks update register zero automatically.
//! Register selection is command state; the bank survives undo and same-draft restoration.

use super::KillBufferKind;
use super::KillBufferSnapshot;
use super::TextArea;
use super::VimMode;
use super::VimPending;
use crate::key_hint::KeyBindingListExt;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub(super) struct VimRegisters {
    pub(super) values: BTreeMap<char, KillBufferSnapshot>,
    pub(super) selected: Option<char>,
    pub(super) waiting: bool,
}

impl TextArea {
    pub(super) fn handle_vim_register_key(&mut self, event: KeyEvent) -> bool {
        if self.vim_registers.waiting {
            self.vim_registers.waiting = false;
            self.vim_registers.selected = match (event.code, event.modifiers) {
                (
                    KeyCode::Char(ch @ ('0' | '_' | '"' | 'a'..='z')),
                    KeyModifiers::NONE | KeyModifiers::SHIFT,
                ) => Some(ch),
                _ => None,
            };
            if self.vim_registers.selected.is_none() {
                self.vim_count = None;
                self.vim_operator_count = None;
            }
            return true;
        }
        if matches!(
            self.vim_mode,
            VimMode::Normal | VimMode::Visual | VimMode::VisualLine
        ) && matches!(self.vim_pending, VimPending::None)
            && self.vim_query().is_none()
            && self.vim_normal_keymap.select_register.is_pressed(event)
        {
            self.vim_registers.selected = None;
            self.vim_registers.waiting = true;
            return true;
        }
        false
    }

    pub(super) fn finish_vim_register_key(&mut self) {
        if matches!(self.vim_pending, VimPending::None)
            && self.vim_count.is_none()
            && self.vim_query().is_none()
            && !self.is_vim_visual_mode()
        {
            self.clear_vim_register_selection();
        }
    }

    pub(super) fn clear_vim_register_selection(&mut self) {
        self.vim_registers.selected = None;
        self.vim_registers.waiting = false;
    }

    pub(super) fn store_vim_yank(&mut self, text: String, kind: KillBufferKind) {
        if self.vim_enabled && self.vim_registers.selected.unwrap_or('"') == '"' {
            self.vim_registers.values.insert(
                '0',
                KillBufferSnapshot {
                    text: text.clone(),
                    kind,
                },
            );
        }
        self.store_kill_buffer(text, kind);
    }

    /// Called by the central kill-buffer writer, including paragraph and Visual overrides.
    pub(super) fn store_vim_register(&mut self, text: &str, kind: KillBufferKind) -> bool {
        if !self.vim_enabled || matches!(self.vim_mode, VimMode::Insert | VimMode::Replace) {
            return true;
        }
        match self.vim_registers.selected {
            Some('_') => false,
            Some(name @ ('0' | 'a'..='z')) => {
                self.vim_registers.values.insert(
                    name,
                    KillBufferSnapshot {
                        text: text.to_owned(),
                        kind,
                    },
                );
                true
            }
            _ => true,
        }
    }

    pub(super) fn vim_paste_buffer(&self) -> Option<KillBufferSnapshot> {
        match self.vim_registers.selected {
            Some('_') => None,
            Some(name @ ('0' | 'a'..='z')) => self.vim_registers.values.get(&name).cloned(),
            _ => Some(KillBufferSnapshot {
                text: self.kill_buffer.clone(),
                kind: self.kill_buffer_kind,
            }),
        }
    }
}

#[cfg(test)]
#[path = "vim_registers_tests.rs"]
mod tests;
