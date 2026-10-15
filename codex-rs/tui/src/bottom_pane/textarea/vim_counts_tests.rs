use super::TextArea;
use crate::key_hint::KeyBinding;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;

fn textarea(text: &str, cursor: usize) -> TextArea {
    let mut t = TextArea::new();
    t.insert_str(text);
    t.set_vim_enabled(true);
    t.set_cursor(cursor);
    t
}

fn keys(t: &mut TextArea, keys: &str) {
    for ch in keys.chars() {
        t.input(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
    }
}

#[test]
fn normal_counts_select_word_destinations_without_editing() {
    for (start, command, expected) in [
        (0, "3w", 8),
        (14, "2b", 7),
        (0, "2e", 6),
        (0, "2W", 14),
        (14, "2B", 0),
        (0, "2E", 12),
    ] {
        let mut t = textarea("one two,three four", start);
        keys(&mut t, command);
        assert_eq!(t.cursor(), expected, "{command}");
        assert_eq!(t.text(), "one two,three four");
        assert!(!t.is_vim_operator_pending());
    }
    let mut t = textarea("a3b3c", 0);
    keys(&mut t, "2f3");
    assert_eq!(t.cursor(), 3);
}

#[test]
fn decimal_counts_preserve_zero_as_a_motion_and_stop_at_line_boundaries() {
    let mut t = textarea("abcdefghijklmnop\nz", 0);
    keys(&mut t, "12l");
    assert_eq!(t.cursor(), 12);
    keys(&mut t, "10h");
    assert_eq!(t.cursor(), 2);
    keys(&mut t, "0");
    assert_eq!(t.cursor(), 0);
    keys(&mut t, "10l");
    assert_eq!(t.cursor(), 10);
    keys(&mut t, "99999999999999999999999999999999999999l");
    assert_eq!(t.cursor(), 15);
    keys(&mut t, "99999999999999999999999999999999999999h");
    assert_eq!(t.cursor(), 0);
}

#[test]
fn counted_vertical_line_end_and_buffer_jumps_use_the_requested_line() {
    let mut t = textarea("abc\ndefg\n  hi\njkl", 1);
    keys(&mut t, "2j");
    assert_eq!(t.cursor(), 10);
    keys(&mut t, "2k");
    assert_eq!(t.cursor(), 1);
    keys(&mut t, "3$");
    assert_eq!(t.cursor(), 12);
    keys(&mut t, "2G");
    assert_eq!(t.cursor(), 4);
    keys(&mut t, "3");
    let (code, modifiers) = t.vim_normal_keymap.jump_top[0].parts();
    t.input(KeyEvent::new(code, modifiers));
    assert_eq!(t.cursor(), 11);
    keys(&mut t, "99999999999999999999999999999G");
    assert_eq!(t.cursor(), 14);
}

#[test]
fn counted_find_and_till_select_the_nth_match_and_accept_digit_targets() {
    for (start, command, expected) in [
        (0, "2fx", 4),
        (0, "2tx", 3),
        (6, "2Fx", 2),
        (6, "2Tx", 3),
        (0, "2f1", 7),
        (0, "9fx", 0),
    ] {
        let mut t = textarea("a x x 11", start);
        keys(&mut t, command);
        assert_eq!(t.cursor(), expected, "{command}");
        assert!(!t.is_vim_operator_pending());
    }
}

#[test]
fn count_prefix_is_pending_and_cancels_on_escape_invalid_keys_and_mode_changes() {
    let mut t = textarea("abcdef", 0);
    keys(&mut t, "3");
    assert!(t.is_vim_operator_pending());
    let escape = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    assert!(t.should_handle_vim_insert_escape(escape));
    t.input(escape);
    keys(&mut t, "l");
    assert_eq!(t.cursor(), 1);
    keys(&mut t, "3zl");
    assert_eq!(t.cursor(), 2);
    keys(&mut t, "3f");
    t.input(escape);
    keys(&mut t, "l");
    assert_eq!(t.cursor(), 3);
    t.set_cursor(2);
    keys(&mut t, "3i");
    t.input(escape);
    keys(&mut t, "l");
    assert_eq!(t.cursor(), 2);
    keys(&mut t, "3x");
    assert_eq!(t.text(), "abcdef");
    assert!(!t.is_vim_operator_pending());
    keys(&mut t, "3d");
    assert!(t.is_vim_operator_pending());
    t.input(escape);
    assert!(!t.is_vim_operator_pending());
}

#[test]
fn counted_navigation_respects_graphemes_atomic_elements_and_remapped_bindings() {
    let mut t = textarea("e\u{301}👩‍💻中", 0);
    t.set_cursor(t.text().len());
    t.insert_element("<attachment>");
    t.insert_str("z");
    t.set_cursor(0);
    keys(&mut t, "3l");
    assert_eq!(t.cursor(), "e\u{301}👩‍💻中".len());
    keys(&mut t, "2h");
    assert_eq!(t.cursor(), "e\u{301}".len());
    t.vim_normal_keymap.move_right = vec![KeyBinding::new(KeyCode::Char('q'), KeyModifiers::NONE)];
    keys(&mut t, "2q");
    assert_eq!(t.cursor(), "e\u{301}👩‍💻中".len());
}

#[test]
fn existing_digit_bindings_take_precedence_over_new_count_prefixes() {
    let mut t = textarea("abcdefghijklmnop", 0);
    t.vim_normal_keymap.move_right = vec![KeyBinding::new(KeyCode::Char('2'), KeyModifiers::NONE)];
    keys(&mut t, "2");
    assert_eq!(t.cursor(), 1);
    assert!(!t.is_vim_operator_pending());
    keys(&mut t, "32");
    assert_eq!(t.cursor(), 4);
    t.vim_normal_keymap.move_right = vec![KeyBinding::new(KeyCode::Char('l'), KeyModifiers::NONE)];
    keys(&mut t, "10l");
    assert_eq!(t.cursor(), 14);
}

#[test]
fn search_repeat_count_wraps_without_iterating_the_numeric_prefix() {
    let mut t = textarea("a x a x a x", 0);
    t.enable_vim_search();
    keys(&mut t, "/x");
    t.input(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    keys(&mut t, "2n");
    assert_eq!(t.cursor(), 10);
    keys(&mut t, "2N");
    assert_eq!(t.cursor(), 2);
}
