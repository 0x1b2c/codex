//! Matching-pair motions keep semantic targets and atomic editor boundaries.

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
fn matching_pairs_search_the_current_line_and_match_nested_pairs_across_lines() {
    for (text, start, end) in [
        ("prefix (a(b)c) tail", 0, 13),
        ("[a\n[b]\nc]", 0, 8),
        ("{a{b}c}", 6, 0),
        ("(x[y]z)", 2, 4),
    ] {
        let mut t = textarea(text, start);
        keys(&mut t, "%");
        assert_eq!(t.cursor(), end, "{text}");
    }
    for text in ["text\n(x)", "prefix (missing", "no pair", ") ("] {
        let mut t = textarea(text, 0);
        keys(&mut t, "%d%");
        assert_eq!(t.text(), text);
        assert_eq!(t.cursor(), 0);
        assert!(t.vim_commands.last_change.is_empty());
    }
}

#[test]
fn pair_operators_include_both_endpoints_and_dot_resolves_a_new_pair() {
    let mut t = textarea("x(a) y(long)", 0);
    keys(&mut t, "d%");
    assert_eq!(t.text(), " y(long)");
    assert_eq!(t.kill_buffer, "x(a)");
    keys(&mut t, ".");
    assert_eq!(t.text(), "");
    let mut t = textarea("(abc)z", 4);
    keys(&mut t, "y%");
    assert_eq!(t.kill_buffer, "(abc)");
    assert_eq!(t.text(), "(abc)z");
    let mut t = textarea("x(a) y(bb)", 0);
    keys(&mut t, "c%NEW");
    t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(t.text(), "NEW y(bb)");
    t.set_cursor(4);
    keys(&mut t, ".");
    assert_eq!(t.text(), "NEW NEW");
}

#[test]
fn counted_percent_jumps_to_a_percentage_line_and_operators_are_linewise() {
    let text = "  one\n  two\n  three\n  four";
    let mut t = textarea(text, 0);
    keys(&mut t, "50%");
    assert_eq!(t.cursor(), "  one\n  ".len());
    keys(&mut t, "100%");
    assert_eq!(t.cursor(), "  one\n  two\n  three\n  ".len());
    keys(&mut t, "101%");
    assert_eq!(t.cursor(), "  one\n  two\n  three\n  ".len());
    for command in ["d50%", "2d25%", "d2%"] {
        let mut t = textarea(text, 0);
        keys(&mut t, command);
        let expected = if command == "d2%" {
            "  two\n  three\n  four"
        } else {
            "  three\n  four"
        };
        assert_eq!(t.text(), expected, "{command}");
        assert_eq!(t.kill_buffer_kind, super::super::KillBufferKind::Linewise);
    }
    let mut t = textarea(text, text.len() - 1);
    keys(&mut t, "y50%");
    assert_eq!(t.kill_buffer, "  two\n  three\n  four");
    assert_eq!(t.text(), text);
    let mut t = textarea(text, 0);
    keys(&mut t, "c50%X");
    t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(t.text(), "X\n  three\n  four");
    let mut t = textarea(text, 0);
    keys(&mut t, "d50%.");
    assert_eq!(t.text(), "  four");
    let mut t = textarea(text, 0);
    keys(&mut t, "d101%");
    assert_eq!(t.text(), text);
    assert!(t.vim_commands.last_change.is_empty());
}

#[test]
fn pair_navigation_updates_visual_character_and_line_selections() {
    let mut t = textarea("(a\nb) tail", 0);
    keys(&mut t, "v%");
    assert_eq!(t.vim_visual_range(), Some(0..5));
    keys(&mut t, "%");
    assert_eq!(t.vim_visual_range(), Some(0..1));
    keys(&mut t, "vV%");
    assert_eq!(t.vim_visual_range(), Some(0..10));
}

#[test]
fn pair_matching_skips_elements_and_keeps_combining_graphemes_whole() {
    let mut t = textarea("(e\u{301})\u{301} tail", 0);
    keys(&mut t, "d%");
    assert_eq!(t.text(), " tail");
    assert_eq!(t.kill_buffer, "(e\u{301})\u{301}");
    let mut t = textarea("x", 1);
    t.insert_element("[hidden)]");
    t.insert_str(" (ok)");
    t.set_cursor(0);
    keys(&mut t, "%");
    assert_eq!(t.cursor(), "x[hidden)] (ok".len());
    let mut t = textarea("(", 1);
    t.insert_element(")");
    t.insert_str(")");
    t.set_cursor(0);
    keys(&mut t, "%");
    assert_eq!(t.cursor(), 2);
}

#[test]
fn matching_pairs_respect_digit_bindings_in_both_modes() {
    let mut t = textarea("x(a)z", 0);
    t.vim_normal_keymap.match_pair = vec![crate::key_hint::plain(KeyCode::Char('2'))];
    t.vim_operator_keymap.motion_match_pair = vec![crate::key_hint::plain(KeyCode::Char('3'))];
    keys(&mut t, "2");
    assert_eq!(t.cursor(), 3);
    keys(&mut t, "d3");
    assert_eq!(t.text(), "xz");
}
