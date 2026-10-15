//! Direct command counts are resolved once and retained by semantic replay.

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
fn direct_delete_counts_keep_one_buffer_and_stop_at_the_logical_line_end() {
    let mut t = textarea("abcdef\nnext", 1);
    keys(&mut t, "3x");
    assert_eq!(t.text(), "aef\nnext");
    assert_eq!(t.kill_buffer, "bcd");
    assert_eq!(t.cursor(), 1);
    keys(&mut t, ".");
    assert_eq!(t.text(), "a\nnext");
    assert_eq!(t.kill_buffer, "ef");
    assert_eq!(t.cursor(), 0);
    t.set_cursor(2);
    keys(&mut t, "999999999999999999999999999999999999999x");
    assert_eq!(t.text(), "a\n");
    assert_eq!(t.kill_buffer, "next");
}

#[test]
fn direct_character_counts_preserve_graphemes_and_atomic_elements() {
    let mut t = textarea("e\u{301}👩‍💻", 0);
    t.set_cursor(t.text().len());
    t.insert_element("<attachment>");
    t.insert_str("z\nnext");
    t.set_cursor(0);
    keys(&mut t, "3x");
    assert_eq!(t.text(), "z\nnext");
    assert_eq!(t.kill_buffer, "e\u{301}👩‍💻<attachment>");
    assert!(t.elements.is_empty());
    let mut t = textarea("e\u{301}👩‍💻", 0);
    t.set_cursor(t.text().len());
    t.insert_element("<attachment>");
    t.insert_str("z");
    t.set_cursor(0);
    keys(&mut t, "3r中");
    assert_eq!(t.text(), "中中中z");
    assert_eq!(t.cursor(), "中中".len());
    assert!(t.elements.is_empty());
}

#[test]
fn counted_replacement_validates_the_whole_range_and_accepts_literal_digits() {
    let mut t = textarea("abcdef\ngh", 1);
    keys(&mut t, "3r7");
    assert_eq!(t.text(), "a777ef\ngh");
    assert_eq!(t.cursor(), 3);
    keys(&mut t, "2l3rX");
    assert_eq!(t.text(), "a777ef\ngh");
    assert_eq!(t.cursor(), 5);
    keys(&mut t, "0.");
    assert_eq!(t.text(), "7777ef\ngh");
    assert_eq!(t.cursor(), 2);
    keys(&mut t, "999999999999999999999999999999999999999rX");
    assert_eq!(t.text(), "7777ef\ngh");
}

#[test]
fn counted_line_shortcuts_use_one_multiline_range_and_yank_keeps_dot() {
    let mut t = textarea("abc\ndef\nghi\njkl", 1);
    keys(&mut t, "2D");
    assert_eq!(t.text(), "a\nghi\njkl");
    assert_eq!(t.kill_buffer, "bc\ndef");
    assert_eq!(t.cursor(), 0);
    keys(&mut t, "2Y");
    assert_eq!(t.kill_buffer, "a\nghi\n");
    assert_eq!(t.kill_buffer_kind, KillBufferKind::Linewise);
    keys(&mut t, ".");
    assert_eq!(t.text(), "\njkl");
    let mut t = textarea("abc\ndef\nghi", 1);
    keys(&mut t, "2CX");
    assert_eq!(t.text(), "aX\nghi");
    assert_eq!(t.kill_buffer, "bc\ndef");
    let mut t = textarea("\nnext", 0);
    keys(&mut t, "9sX");
    assert_eq!(t.text(), "X\nnext");
}

#[test]
fn counted_paste_places_cursor_like_vim_for_character_and_line_buffers() {
    for (buffer, kind, expected, cursor) in [
        ("xy", KillBufferKind::Characterwise, "axyxyxybc", 6),
        ("x\ny", KillBufferKind::Characterwise, "ax\nyx\nyx\nybc", 1),
        (
            "  xy\n",
            KillBufferKind::Linewise,
            "abc\n  xy\n  xy\n  xy",
            6,
        ),
        ("  \n", KillBufferKind::Linewise, "abc\n  \n  \n  ", 5),
        ("\n\n", KillBufferKind::Linewise, "abc\n\n\n\n\n\n", 4),
    ] {
        let mut t = textarea("abc", 0);
        t.store_kill_buffer(buffer.to_owned(), kind);
        keys(&mut t, "3p");
        assert_eq!(t.text(), expected, "{buffer:?}");
        assert_eq!(t.cursor(), cursor, "{buffer:?}");
    }
    let mut t = textarea("abc\ndef", 0);
    t.store_kill_buffer("  xy".to_owned(), KillBufferKind::Linewise);
    keys(&mut t, "2p");
    assert_eq!(t.text(), "abc\n  xy\n  xy\ndef");
    assert_eq!(t.cursor(), 6);
    let mut t = textarea("\nnext", 0);
    t.store_kill_buffer("xy".to_owned(), KillBufferKind::Characterwise);
    keys(&mut t, "3p");
    assert_eq!(t.text(), "xyxyxy\nnext");
    assert_eq!(t.cursor(), 5);
}

#[test]
fn counted_paste_and_replace_reject_impossible_allocations_without_changes() {
    let mut t = textarea("abc", 0);
    t.store_kill_buffer("xy".to_owned(), KillBufferKind::Characterwise);
    keys(&mut t, "999999999999999999999999999999999999999p");
    assert_eq!(t.text(), "abc");
    assert_eq!(t.cursor(), 0);
    assert_eq!(t.kill_buffer, "xy");
    keys(&mut t, "2p0.");
    assert_eq!(t.text(), "axyxyxyxybc");
    assert_eq!(t.cursor(), 4);
}
