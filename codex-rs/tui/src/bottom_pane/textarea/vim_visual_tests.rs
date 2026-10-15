use super::super::TextAreaState;
use super::TextArea;
use crate::keymap::RuntimeKeymap;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::StatefulWidgetRef;

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
fn visual_character_selection_is_inclusive_and_can_cross_its_anchor() {
    let mut t = textarea("abc\ndef\nghi", 5);
    keys(&mut t, "v");
    assert_eq!(t.vim_visual_range(), Some(5..6));
    assert_eq!(t.vim_mode_label(), Some("Visual"));
    assert!(t.mouse_selection_range().is_none());
    keys(&mut t, "j");
    assert_eq!(t.vim_visual_range(), Some(5..10));
    keys(&mut t, "kk");
    assert_eq!(t.vim_visual_range(), Some(1..6));
    keys(&mut t, "v");
    assert_eq!(t.vim_visual_range(), None);
    assert_eq!(t.vim_mode_label(), Some("Normal"));
    assert_eq!(t.text(), "abc\ndef\nghi");
}

#[test]
fn visual_navigation_uses_counts_words_lines_and_find_repeats() {
    let mut t = textarea("xa1a2a3a\none two three\nlast", 0);
    keys(&mut t, "v2fa");
    assert_eq!(t.vim_visual_range(), Some(0..4));
    keys(&mut t, ";,");
    assert_eq!(t.cursor(), 3);
    keys(&mut t, "Ta");
    assert_eq!(t.cursor(), 2);
    keys(&mut t, "G0^$2b");
    assert_eq!(t.vim_visual_range(), Some(0..18));
    keys(&mut t, "2w");
    assert_eq!(t.vim_visual_range(), Some(0..27));
    let runtime = RuntimeKeymap::from_config(&codex_config::types::TuiKeymap::default()).unwrap();
    t.set_keymap_bindings(&runtime);
    let mut matcher = crate::keymap::KeyChordMatcher::default();
    let g = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
    assert!(matches!(
        matcher.advance(g, &runtime.chords, t.keymap_contexts()),
        crate::keymap::KeyChordMatch::Pending(_)
    ));
    let crate::keymap::KeyChordMatch::Completed(event) =
        matcher.advance(g, &runtime.chords, t.keymap_contexts())
    else {
        panic!("gg must retain Visual navigation context");
    };
    t.input(event);
    assert_eq!(t.vim_visual_range(), Some(0..1));
}

#[test]
fn visual_selection_preserves_graphemes_atomic_elements_and_empty_lines() {
    let mut inside_grapheme = textarea("e\u{301}x", 1);
    keys(&mut inside_grapheme, "v");
    assert_eq!(inside_grapheme.cursor(), 0);
    assert_eq!(
        inside_grapheme.vim_visual_range(),
        Some(0.."e\u{301}".len())
    );
    let mut t = textarea("e\u{301}👩‍💻中", 0);
    t.set_cursor(t.text().len());
    t.insert_element("<element>");
    t.insert_str("\n\nz");
    t.set_cursor(0);
    keys(&mut t, "v3l");
    assert_eq!(t.vim_visual_range(), Some(0.."e\u{301}👩‍💻中<element>".len()));
    keys(&mut t, "3h");
    assert_eq!(t.vim_visual_range(), Some(0.."e\u{301}".len()));
    for text in ["", "\n", "a\n"] {
        let mut t = textarea(text, text.len());
        keys(&mut t, "vhljk0$");
        let range = t.vim_visual_range().expect("visual range");
        assert!(range.end <= text.len());
        t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(t.vim_mode_label(), Some("Normal"));
        assert_eq!(t.text(), text);
    }
}

#[test]
fn visual_render_highlights_selection_without_mouse_drag_state() {
    let mut t = textarea("abcd\nefgh", 1);
    keys(&mut t, "vj");
    let area = Rect::new(0, 0, 6, 2);
    let mut buf = Buffer::empty(area);
    StatefulWidgetRef::render_ref(&&t, area, &mut buf, &mut TextAreaState::default());
    let rows = (0..2)
        .map(|y| {
            (0..6)
                .map(|x| {
                    if buf[(x, y)]
                        .style()
                        .add_modifier
                        .contains(Modifier::REVERSED)
                    {
                        '#'
                    } else {
                        '.'
                    }
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(rows, @r"
.####.
##....
");
    assert!(t.mouse_selection_range().is_none());
}

#[test]
fn visual_empty_line_newline_is_highlighted_and_escape_clears_find_and_count() {
    let mut t = textarea("a\n\nz", 2);
    keys(&mut t, "v2f");
    let escape = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    assert!(t.should_handle_vim_insert_escape(escape));
    let area = Rect::new(0, 0, 4, 3);
    let mut buf = Buffer::empty(area);
    StatefulWidgetRef::render_ref(&&t, area, &mut buf, &mut TextAreaState::default());
    assert!(
        buf[(0, 1)]
            .style()
            .add_modifier
            .contains(Modifier::REVERSED)
    );
    t.input(escape);
    assert!(!t.is_vim_operator_pending());
    assert_eq!(t.vim_visual_range(), None);
    assert_eq!(t.cursor(), 2);
}

#[test]
fn visual_foundation_consumes_unsupported_edits_and_supports_a_digit_binding() {
    let mut t = textarea("abc", 0);
    let mut runtime = RuntimeKeymap::defaults();
    runtime.vim_normal.enter_visual = vec![crate::key_hint::plain(KeyCode::Char('2'))];
    t.set_keymap_bindings(&runtime);
    keys(&mut t, "2");
    assert!(t.is_vim_visual_mode());
    keys(&mut t, "dcyipPxu.");
    t.input(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
    assert_eq!(t.text(), "abc");
    assert!(!t.is_vim_operator_pending());
    assert!(t.is_vim_visual_mode());
    t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(t.vim_visual_range(), None);
}
