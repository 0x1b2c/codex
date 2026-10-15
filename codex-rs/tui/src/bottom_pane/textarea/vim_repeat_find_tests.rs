//! Character-search repeats keep their original direction and semantic edit target.

use super::TextArea;
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
fn repeats_keep_the_original_find_direction_after_comma() {
    let mut t = textarea("xa1a2a3a4a", 0);
    keys(&mut t, "fa2;");
    assert_eq!(t.cursor(), 5);
    keys(&mut t, ",");
    assert_eq!(t.cursor(), 3);
    keys(&mut t, ";");
    assert_eq!(t.cursor(), 5);
    keys(&mut t, "Fa;");
    assert_eq!(t.cursor(), 1);
    keys(&mut t, ",");
    assert_eq!(t.cursor(), 3);
}

#[test]
fn till_repeats_skip_an_adjacent_match_only_for_a_single_repeat() {
    for (command, expected) in [("ta;", 2), ("ta2;", 2), ("ta3;", 4)] {
        let mut t = textarea("xa1a2a3a4a", 0);
        keys(&mut t, command);
        assert_eq!(t.cursor(), expected, "{command}");
    }
    let mut t = textarea("xa1a2a3a4a", 9);
    keys(&mut t, "Ta;");
    assert_eq!(t.cursor(), 6);
    keys(&mut t, ",");
    assert_eq!(t.cursor(), 8);
    keys(&mut t, ";");
    assert_eq!(t.cursor(), 6);
}

#[test]
fn repeat_operators_multiply_counts_and_dot_replays_the_saved_target() {
    let mut t = textarea("xa1a2a3a4a5a6a7a8a9a", 0);
    keys(&mut t, "fa2d2;");
    assert_eq!(t.text(), "x5a6a7a8a9a");
    assert_eq!(t.kill_buffer, "a1a2a3a4a");
    keys(&mut t, "f8");
    t.set_cursor(1);
    keys(&mut t, ".");
    assert_eq!(t.text(), "x9a");
    let mut t = textarea("xa1a2a3a4a", 0);
    keys(&mut t, "tad;");
    assert_eq!(t.text(), "a2a3a4a");
    assert_eq!(t.kill_buffer, "xa1");
    let mut t = textarea("xa1a2a3a4a", 0);
    keys(&mut t, "fa;y,");
    assert_eq!(t.kill_buffer, "a1");
    assert_eq!(t.text(), "xa1a2a3a4a");
    keys(&mut t, "c;X");
    t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(t.text(), "xX2a3a4a");
}

#[test]
fn failed_searches_update_the_target_but_canceled_searches_do_not() {
    let mut t = textarea("xa1a2a", 0);
    keys(&mut t, ";,d;");
    assert_eq!(t.text(), "xa1a2a");
    assert!(t.vim_commands.last_change.is_empty());
    keys(&mut t, "faFz0;");
    assert_eq!(t.cursor(), 0);
    keys(&mut t, "fa");
    keys(&mut t, "f");
    t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    keys(&mut t, ";");
    assert_eq!(t.cursor(), 3);
    keys(&mut t, "dFz;");
    assert_eq!(t.cursor(), 3);
    assert!(t.vim_commands.last_change.is_empty());
}

#[test]
fn repeat_find_stays_on_the_line_and_uses_graphemes() {
    let mut t = textarea("xé e\u{301} é\n é", 0);
    keys(&mut t, "fé;");
    assert_eq!(t.cursor(), "xé e\u{301} ".len());
    keys(&mut t, ";");
    assert_eq!(t.cursor(), "xé e\u{301} ".len());
    let mut t = textarea("xe\u{301} e\u{301}", 0);
    keys(&mut t, "fe;");
    assert_eq!(t.cursor(), "xe\u{301} ".len());
}

#[test]
fn repeat_find_respects_digit_rebindings_and_atomic_elements() {
    let mut t = textarea("xa1a2a3a", 0);
    t.vim_normal_keymap.repeat_find = vec![crate::key_hint::plain(KeyCode::Char('2'))];
    t.vim_operator_keymap.motion_repeat_find = vec![crate::key_hint::plain(KeyCode::Char('3'))];
    keys(&mut t, "fa2");
    assert_eq!(t.cursor(), 3);
    keys(&mut t, "d3");
    assert_eq!(t.text(), "xa13a");
    let mut t = textarea("x", 1);
    t.insert_element("<a>");
    t.insert_str(" a a");
    t.set_cursor(0);
    keys(&mut t, "ta");
    assert_eq!(t.cursor(), 4);
    keys(&mut t, ";");
    assert_eq!(t.cursor(), 6);
    assert!(t.elements.iter().all(|e| !e.range.contains(&t.cursor())));
}
