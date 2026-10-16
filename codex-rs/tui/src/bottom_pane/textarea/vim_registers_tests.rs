//! Register selection is literal input and survives semantic replay and draft restoration.

use super::super::KillBufferKind;
use super::super::TextArea;
use super::super::VimPersistentState;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;

fn textarea(text: &str) -> TextArea {
    let mut t = TextArea::new();
    t.insert_str(text);
    t.set_vim_enabled(true);
    t.set_cursor(0);
    t
}

fn keys(t: &mut TextArea, command: &str) {
    for ch in command.chars() {
        t.input(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
    }
}

#[test]
fn yank_zero_survives_deletes_and_black_hole_commands() {
    let mut t = textarea("one two three four");
    keys(&mut t, "yw");
    keys(&mut t, "dw");
    assert_eq!(t.kill_buffer, "one ");
    keys(&mut t, "dw");
    assert_eq!(t.kill_buffer, "two ");
    keys(&mut t, "\"_yw\"_dw");
    assert_eq!(t.kill_buffer, "two ");
    keys(&mut t, "\"0P");
    assert_eq!(t.text(), "one four");
    assert_eq!(t.kill_buffer, "two ");
    keys(&mut t, "\"_p");
    assert_eq!(t.text(), "one four");
}

#[test]
fn explicit_zero_writes_and_named_yanks_follow_vim_register_rules() {
    let mut t = textarea("one two three");
    keys(&mut t, "yy\"ayw");
    assert_eq!(t.vim_registers.values[&'0'].text, "one two three");
    assert_eq!(t.vim_registers.values[&'a'].text, "one ");
    keys(&mut t, "w\"0dw");
    assert_eq!(t.vim_registers.values[&'0'].text, "two ");
    keys(&mut t, "\"aP");
    assert_eq!(t.text(), "one one three");
    assert_eq!(t.kill_buffer, "two ");
}

#[test]
fn register_counts_visual_linewise_and_dot_preserve_the_selected_buffer() {
    let mut t = textarea("one two three four five six");
    keys(&mut t, "yw\"_2dw2.");
    assert_eq!(t.text(), "five six");
    assert_eq!(t.kill_buffer, "one ");
    keys(&mut t, "\"02P");
    assert_eq!(t.text(), "one one five six");
    let mut t = textarea("first\nsecond\nthird");
    keys(&mut t, "yyV\"_d");
    assert_eq!(t.text(), "second\nthird");
    assert_eq!(t.kill_buffer, "first\n");
    keys(&mut t, "\"0P");
    assert_eq!(t.text(), "first\nsecond\nthird");
    assert_eq!(t.kill_buffer_kind, KillBufferKind::Linewise);
}

#[test]
fn named_change_replays_insertion_and_keeps_yank_zero() {
    let mut t = textarea("one two three");
    keys(&mut t, "yw\"acwX");
    t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    keys(&mut t, "w.");
    assert_eq!(t.text(), "X X three");
    assert_eq!(t.vim_registers.values[&'a'].text, "two");
    assert_eq!(t.vim_registers.values[&'0'].text, "one ");
}

#[test]
fn register_literal_is_not_a_count_and_cancel_or_failure_clears_selection() {
    let mut t = textarea("one two three");
    keys(&mut t, "yw\"");
    assert_eq!(t.keymap_context(), crate::keymap::KeymapContext::Editor);
    assert!(!t.wants_vim_count_key(KeyEvent::new(KeyCode::Char('0'), KeyModifiers::NONE)));
    keys(&mut t, "0");
    assert!(t.is_vim_operator_pending());
    assert!(t.wants_vim_count_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE)));
    t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    keys(&mut t, "w\"_dq");
    keys(&mut t, "dw");
    assert_eq!(t.kill_buffer, "two ");
    keys(&mut t, "\"9x");
    assert_eq!(t.text(), "one hree");
    keys(&mut t, "\"_");
    t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    keys(&mut t, "x");
    assert_eq!(t.kill_buffer, "h");
}

#[test]
fn register_prefix_remapping_respects_digits_and_literal_find_and_quotes() {
    let mut t = textarea("\"one\" two");
    let mut keymap = crate::keymap::RuntimeKeymap::defaults();
    keymap.vim_normal.select_register = vec![crate::key_hint::plain(KeyCode::Char('2'))];
    t.set_keymap_bindings(&keymap);
    keys(&mut t, "2_yi\"");
    assert!(t.kill_buffer.is_empty());
    keys(&mut t, "f2");
    assert_eq!(t.cursor(), 0);
    keys(&mut t, "yi\"");
    assert_eq!(t.kill_buffer, "one");
}

#[test]
fn black_hole_visual_delete_treats_graphemes_and_elements_atomically() {
    let mut t = textarea("e\u{301}👩‍💻");
    t.set_cursor(t.text().len());
    t.insert_element("<attachment>");
    t.insert_str("tail");
    t.set_cursor(0);
    keys(&mut t, "v2l\"_d");
    assert_eq!(t.text(), "tail");
    assert!(t.elements.is_empty());
    assert!(t.kill_buffer.is_empty());
}

#[test]
fn register_bank_survives_persistent_state_swap_and_non_vim_kills() {
    let mut t = textarea("one two");
    keys(&mut t, "yw\"adw");
    let mut state = VimPersistentState::default();
    t.swap_vim_persistent_state(&mut state);
    t.set_text_clearing_elements("restored");
    t.swap_vim_persistent_state(&mut state);
    keys(&mut t, "\"0P");
    assert_eq!(t.text(), "one restored");
    keys(&mut t, "\"aP");
    assert_eq!(t.kill_buffer, "one ");
    t.set_vim_enabled(false);
    t.set_cursor(0);
    t.kill_to_end_of_line();
    assert_eq!(t.vim_registers.values[&'0'].text, "one ");
}

#[test]
fn black_hole_covers_change_line_shortcuts_and_paragraph_yanks() {
    for command in [
        "\"_cw", "\"_S", "\"_C", "v\"_c", "\"_dip", "\"_yip", "V\"_y", "\"_Y",
    ] {
        let mut t = textarea("one two\n\nthree");
        keys(&mut t, "yw");
        keys(&mut t, command);
        assert_eq!(t.kill_buffer, "one ", "{command}");
        assert_eq!(t.vim_registers.values[&'0'].text, "one ", "{command}");
    }
}

#[test]
fn counts_before_register_and_visual_entry_keep_selection_until_the_edit() {
    let mut t = textarea("one two three four");
    keys(&mut t, "yw2\"_dw");
    assert_eq!(t.text(), "three four");
    assert_eq!(t.kill_buffer, "one ");
    keys(&mut t, "\"_vld");
    assert_eq!(t.text(), "ree four");
    assert_eq!(t.kill_buffer, "one ");
}

#[test]
fn named_paste_and_dot_read_the_bank_without_changing_unnamed() {
    let mut t = textarea("one two three");
    keys(&mut t, "\"aywdwdw\"aP");
    assert_eq!(t.text(), "one three");
    assert_eq!(t.kill_buffer, "two ");
    keys(&mut t, "2.");
    assert_eq!(t.text(), "oneone one  three");
    assert_eq!(t.kill_buffer, "two ");
}
