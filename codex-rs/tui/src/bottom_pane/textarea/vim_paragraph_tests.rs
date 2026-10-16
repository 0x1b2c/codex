//! Paragraph navigation follows logical empty lines and preserves editing transactions.

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

fn keys(t: &mut TextArea, keys: &str) {
    for ch in keys.chars() {
        t.input(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
    }
}

#[test]
fn paragraph_motions_skip_blank_runs_and_keep_whitespace_only_lines_in_the_paragraph() {
    let text = "aaa\nbbb\n\n\nccc\n   \nddd\n\neee";
    for (start, command, target) in [
        (1, "}", 8),
        (1, "2}", 22),
        (8, "}", 22),
        (9, "{", 0),
        (10, "{", 9),
        (23, "{", 22),
        (23, "}", 25),
        (25, "}", 25),
        (1, "2{", 1),
        (22, "2}", 22),
    ] {
        let mut t = textarea(text, start);
        keys(&mut t, command);
        assert_eq!(t.cursor(), target, "{start}: {command}");
        assert!(t.vim_commands.last_change.is_empty());
    }
    let mut t = textarea("aaa\n\n", 0);
    keys(&mut t, "2}");
    assert_eq!(t.cursor(), 5);
    keys(&mut t, "{");
    assert_eq!(t.cursor(), 0);
    let mut t = textarea("", 0);
    keys(&mut t, "{}d}");
    assert_eq!(t.text(), "");
}

#[test]
fn paragraph_operators_resolve_counts_and_preserve_exclusive_line_boundaries() {
    for command in ["2d}", "d2}"] {
        let mut t = textarea("a\n\nb\n\nc", 0);
        keys(&mut t, command);
        assert_eq!(t.text(), "\nc");
        assert_eq!(t.kill_buffer, "a\n\nb\n");
        assert_eq!(t.kill_buffer_kind, KillBufferKind::Linewise);
    }
    let mut t = textarea("a\n\nb\n\nc\n\nd\n\ne", 0);
    keys(&mut t, "2d2}");
    assert_eq!(t.text(), "\ne");
    let mut t = textarea("aaa\nbbb\n\nccc", 1);
    keys(&mut t, "d}");
    assert_eq!(t.text(), "a\n\nccc");
    assert_eq!(t.kill_buffer, "aa\nbbb");
    assert_eq!(t.kill_buffer_kind, KillBufferKind::Characterwise);
    let mut t = textarea("aaa\nbbb\n\nccc", 4);
    keys(&mut t, "y{");
    assert_eq!(t.kill_buffer, "aaa\n");
    assert_eq!(t.kill_buffer_kind, KillBufferKind::Linewise);
    assert_eq!(t.cursor(), 4);
    let mut t = textarea("aaa\nbbb", 1);
    keys(&mut t, "d}");
    assert_eq!(t.text(), "a");
    assert_eq!(t.kill_buffer, "aa\nbbb");
    let mut t = textarea("a\n\nb", 3);
    keys(&mut t, "y}");
    assert_eq!(t.kill_buffer, "b");
}

#[test]
fn paragraph_changes_retain_unselected_blank_lines_and_dot_resolves_new_paragraphs() {
    let mut t = textarea("aaa\nbbb\n\nccc", 0);
    keys(&mut t, "c}X");
    t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(t.text(), "X\n\nccc");
    let mut t = textarea("aaa\n\n", 0);
    keys(&mut t, "d}");
    assert_eq!(t.text(), "\n");
    let mut t = textarea("aaa\n\n", 4);
    keys(&mut t, "c}X");
    t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(t.text(), "aaa\nX\n");
    let mut t = textarea("a\n\nb\n\nc", 0);
    keys(&mut t, "d}.");
    assert_eq!(t.text(), "\nc");
}

#[test]
fn paragraph_motions_extend_both_visual_selection_kinds() {
    let mut t = textarea("aaa\nbbb\n\nccc", 0);
    keys(&mut t, "v}");
    assert_eq!(t.vim_visual_range(), Some(0..9));
    keys(&mut t, "{");
    assert_eq!(t.vim_visual_range(), Some(0..1));
    keys(&mut t, "vV}");
    assert_eq!(t.vim_visual_range(), Some(0..9));
}

#[test]
fn paragraph_navigation_preserves_graphemes_and_treats_elements_as_opaque() {
    let mut t = textarea("aaa\n\ne\u{301}", 0);
    keys(&mut t, "2}");
    assert_eq!(t.cursor(), 5);
    let mut t = textarea("a", 1);
    t.insert_element("x\n\ny");
    t.insert_str("\n\nb");
    t.set_cursor(0);
    keys(&mut t, "}");
    assert_eq!(t.cursor(), 6);
    keys(&mut t, "d{");
    assert_eq!(t.text(), "\nb");
    assert_eq!(t.kill_buffer, "ax\n\ny\n");
}

#[test]
fn paragraph_digit_bindings_take_precedence_in_normal_visual_and_operator_modes() {
    let mut t = textarea("a\n\nb\n\nc", 0);
    t.vim_normal_keymap.move_paragraph_forward = vec![crate::key_hint::plain(KeyCode::Char('2'))];
    t.vim_operator_keymap.motion_paragraph_backward =
        vec![crate::key_hint::plain(KeyCode::Char('3'))];
    keys(&mut t, "2");
    assert_eq!(t.cursor(), 2);
    keys(&mut t, "d3");
    assert_eq!(t.text(), "\nb\n\nc");
    keys(&mut t, "v2");
    assert_eq!(t.vim_visual_range(), Some(0..4));
}

#[test]
fn paragraph_objects_select_content_and_blank_runs_as_complete_lines() {
    let text = "aaa\nbbb\n\n\nccc\n   \nddd\n\neee";
    for (cursor, command, expected) in [
        (1, "yip", "aaa\nbbb\n"),
        (1, "yap", "aaa\nbbb\n\n\n"),
        (8, "yip", "\n\n"),
        (8, "yap", "\n\nccc\n"),
        (10, "yip", "ccc\n"),
        (14, "yip", "   \n"),
        (14, "yap", "   \nddd\n"),
        (23, "yap", "\neee\n"),
    ] {
        let mut t = textarea(text, cursor);
        keys(&mut t, command);
        assert_eq!(t.kill_buffer, expected, "{cursor}: {command}");
        assert_eq!(t.kill_buffer_kind, KillBufferKind::Linewise);
        assert_eq!(t.text(), text);
        assert_eq!(t.cursor(), cursor);
    }
}

#[test]
fn paragraph_object_counts_multiply_and_resolve_runs_before_editing() {
    for (cursor, command, expected) in [
        (0, "y2ip", "a\n\n"),
        (0, "2y2ip", "a\n\nb\n\n"),
        (0, "y2ap", "a\n\nb\n\n"),
        (2, "y2ip", "\nb\n"),
        (2, "y2ap", "\nb\n\nc\n"),
        (6, "y2ap", "\nc\n\nd\n"),
    ] {
        let mut t = textarea("a\n\nb\n\nc\n\nd", cursor);
        keys(&mut t, command);
        assert_eq!(t.kill_buffer, expected, "{cursor}: {command}");
    }
    let mut t = textarea("a\n\nb\n\nc\n\nd\n\ne", 0);
    keys(&mut t, "2d2ap");
    assert_eq!(t.text(), "e");
    assert_eq!(t.kill_buffer, "a\n\nb\n\nc\n\nd\n\n");
}

#[test]
fn paragraph_objects_include_the_final_logical_empty_line_in_the_register() {
    for (text, cursor, command, expected) in [
        ("a\n\n", 0, "yap", "a\n\n\n"),
        ("a\n\n", 2, "yip", "\n\n"),
        ("a\n", 0, "yip", "a\n"),
        ("a\n", 2, "yip", "\n"),
        ("a", 0, "yip", "a\n"),
        ("", 0, "yip", "\n"),
    ] {
        let mut t = textarea(text, cursor);
        keys(&mut t, command);
        assert_eq!(t.kill_buffer, expected, "{text:?}: {command}");
        assert_eq!(t.kill_buffer_kind, KillBufferKind::Linewise);
    }
    let mut t = textarea("a\n\n", 0);
    keys(&mut t, "yapP");
    assert_eq!(t.text(), "a\n\n\na\n\n");
    let mut t = textarea("a\n\n", 2);
    keys(&mut t, "dip");
    assert_eq!(t.text(), "a");
    assert_eq!(t.kill_buffer, "\n\n");
    let mut t = textarea("a\n\nb", 3);
    keys(&mut t, "dap");
    assert_eq!(t.text(), "a");
    assert_eq!(t.kill_buffer, "\nb\n");
}

#[test]
fn paragraph_changes_preserve_unselected_newlines_and_replay_complete_insertions() {
    for (text, cursor, command, expected) in [
        ("a\n\nb", 0, "cipXY", "XY\n\nb"),
        ("a\n\nb", 0, "capXY", "XY\nb"),
        ("a\n\nb", 3, "cipXY", "a\n\nXY"),
        ("a\n\nb", 3, "capXY", "a\nXY"),
        ("a\n\n", 2, "cipXY", "a\nXY"),
        ("", 0, "cipXY", "XY"),
    ] {
        let mut t = textarea(text, cursor);
        keys(&mut t, command);
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(t.text(), expected, "{text:?}: {command}");
    }
    let mut t = textarea("a\n\nb\nbb\n\nc", 0);
    keys(&mut t, "cipXY");
    t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    t.set_cursor(4);
    keys(&mut t, ".");
    assert_eq!(t.text(), "XY\n\nXY\n\nc");
    assert_eq!(t.kill_buffer, "b\nbb\n");
    let mut t = textarea("a\n\nb\nbb\n\nc", 0);
    keys(&mut t, "dip.");
    assert_eq!(t.text(), "b\nbb\n\nc");
    assert_eq!(t.kill_buffer, "\n");
}

#[test]
fn failed_and_cancelled_paragraph_objects_preserve_registers_and_last_change() {
    for (text, cursor, command) in [
        ("a", 0, "d2ip"),
        ("a\n\n", 2, "cap"),
        ("a\n\nb", 2, "y2ap"),
        ("a\n\nb", 0, "99999999999999999999d99999999999999999999ap"),
        ("a\n\nb", 0, "di2"),
    ] {
        let mut t = textarea(text, cursor);
        t.kill_buffer = "previous".into();
        let last_change = format!("{:?}", t.vim_commands.last_change);
        keys(&mut t, command);
        assert_eq!(t.text(), text, "{command}");
        assert_eq!(t.kill_buffer, "previous");
        assert_eq!(format!("{:?}", t.vim_commands.last_change), last_change);
        assert_eq!(t.vim_count, None);
        assert!(!t.is_vim_operator_pending());
        assert!(t.is_vim_normal_mode());
    }
    let mut t = textarea("a\n\nb", 0);
    keys(&mut t, "2di");
    t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    keys(&mut t, "dip");
    assert_eq!(t.text(), "\nb");
}

#[test]
fn paragraph_objects_keep_graphemes_and_opaque_elements_intact_and_can_be_remapped() {
    let mut t = textarea("e\u{301}👩‍💻\n\nb", 0);
    keys(&mut t, "dip");
    assert_eq!(t.text(), "\nb");
    assert_eq!(t.kill_buffer, "e\u{301}👩‍💻\n");
    let mut t = textarea("a", 1);
    t.insert_element("x\n\ny");
    t.insert_str("\n\nb");
    t.set_cursor(0);
    keys(&mut t, "dip");
    assert_eq!(t.text(), "\nb");
    assert_eq!(t.kill_buffer, "ax\n\ny\n");
    assert!(t.elements.is_empty());
    let mut t = textarea("a\n\nb", 0);
    t.vim_text_object_keymap.paragraph = vec![crate::key_hint::plain(KeyCode::Char('2'))];
    keys(&mut t, "yi2");
    assert_eq!(t.kill_buffer, "a\n");
    keys(&mut t, "p");
    assert_eq!(t.text(), "a\na\n\nb");
}
