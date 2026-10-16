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
