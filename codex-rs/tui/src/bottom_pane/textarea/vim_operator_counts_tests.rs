use super::super::KillBufferKind;
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

fn keys(t: &mut TextArea, command: &str) {
    for ch in command.chars() {
        t.input(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
    }
}

#[test]
fn operator_prefixes_multiply_and_store_the_whole_deleted_range() {
    for command in ["6dw", "d6w", "2d3w"] {
        let mut t = textarea("a b c d e f g h", 0);
        keys(&mut t, command);
        assert_eq!(t.text(), "g h", "{command}");
        assert_eq!(t.kill_buffer, "a b c d e f ");
        assert_eq!(t.kill_buffer_kind, KillBufferKind::Characterwise);
        assert!(!t.is_vim_operator_pending());
    }
    let mut t = textarea("abcdefghijk", 7);
    keys(&mut t, "d0");
    assert_eq!(t.text(), "hijk");
    keys(&mut t, "d10l");
    assert_eq!(t.text(), "");
}

#[test]
fn counted_line_and_vertical_operators_keep_linewise_buffers() {
    for command in ["2dd", "d2d", "dj"] {
        let mut t = textarea("one\ntwo\nthree", 1);
        keys(&mut t, command);
        assert_eq!(t.text(), "three", "{command}");
        assert_eq!(t.kill_buffer, "one\ntwo\n");
        assert_eq!(t.kill_buffer_kind, KillBufferKind::Linewise);
    }
    let mut t = textarea("one\ntwo\nthree\nfour", 0);
    keys(&mut t, "2yy");
    assert_eq!(t.text(), "one\ntwo\nthree\nfour");
    assert_eq!(t.kill_buffer, "one\ntwo\n");
    assert_eq!(t.kill_buffer_kind, KillBufferKind::Linewise);
    keys(&mut t, "2ccX");
    t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(t.text(), "X\nthree\nfour");
    assert_eq!(t.kill_buffer, "one\ntwo\n");
}

#[test]
fn counted_word_end_change_and_character_find_record_semantic_targets() {
    let mut t = textarea("one two three four five six", 0);
    keys(&mut t, "2cwX");
    t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(t.text(), "X three four five six");
    keys(&mut t, "w.");
    assert_eq!(t.text(), "X X five six");
    let mut t = textarea("a1b1c1d1e", 0);
    keys(&mut t, "d2f1");
    assert_eq!(t.text(), "c1d1e");
    assert_eq!(t.kill_buffer, "a1b1");
    keys(&mut t, ".");
    assert_eq!(t.text(), "e");
    let mut t = textarea("a x", 0);
    keys(&mut t, "2dfx");
    assert_eq!(t.text(), "a x");
}

#[test]
fn counted_operators_cancel_and_saturate_without_splitting_atomic_items() {
    let mut t = textarea("e\u{301}👩‍💻中z", 0);
    keys(&mut t, "d2l");
    assert_eq!(t.text(), "中z");
    t.set_cursor(0);
    t.insert_element("<attachment>");
    t.set_cursor(0);
    keys(&mut t, "2dl");
    assert_eq!(t.text(), "z");
    keys(&mut t, "3d2");
    t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    keys(&mut t, "l");
    assert_eq!(t.text(), "z");
    keys(&mut t, "3dz");
    assert!(!t.is_vim_operator_pending());
    keys(&mut t, "999999999999999999999999d999999999999999999999999l");
    assert_eq!(t.text(), "");
}

#[test]
fn counted_buffer_jumps_use_absolute_lines_and_empty_change_enters_insert() {
    let mut t = textarea("one\ntwo\nthree\nfour", 4);
    keys(&mut t, "d3G");
    assert_eq!(t.text(), "one\nfour");
    assert_eq!(t.kill_buffer, "two\nthree\n");
    let mut t = textarea("", 0);
    keys(&mut t, "2ccX");
    t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(t.text(), "X");
}

#[test]
fn counted_motion_ranges_include_intermediate_words_and_logical_lines() {
    for (text, start, command, expected, killed) in [
        ("one two three four", 0, "d2e", " three four", "one two"),
        ("one two three four", 8, "d2b", "three four", "one two "),
        ("one two,three four", 0, "d2W", "four", "one two,three "),
        ("abc\ndef\nghi", 1, "d2$", "a\nghi", "bc\ndef"),
        ("a x x z", 0, "d2tx", "x z", "a x "),
        ("a x x z", 6, "d2Fx", "a z", "x x "),
    ] {
        let mut t = textarea(text, start);
        keys(&mut t, command);
        assert_eq!(t.text(), expected, "{command}");
        assert_eq!(t.kill_buffer, killed, "{command}");
    }
}
