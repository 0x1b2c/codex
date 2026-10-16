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
    keys(&mut t, "ipPxu.");
    t.input(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
    assert_eq!(t.text(), "abc");
    assert!(!t.is_vim_operator_pending());
    assert!(t.is_vim_visual_mode());
    t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(t.vim_visual_range(), None);
}

#[test]
fn visual_delete_and_yank_use_the_inclusive_range_in_both_directions() {
    for (cursor, sequence) in [(1, "vjld"), (7, "vkhd")] {
        let mut t = textarea("abcd\nefgh\nlast", cursor);
        keys(&mut t, sequence);
        assert_eq!(t.text(), "ah\nlast");
        assert_eq!(t.kill_buffer, "bcd\nefg");
        assert_eq!(
            t.kill_buffer_kind,
            super::super::KillBufferKind::Characterwise
        );
        assert_eq!(t.cursor(), 1);
        assert!(t.is_vim_normal_mode());
        assert!(t.vim_visual_range().is_none());
    }
    let mut t = textarea("abcd\nefgh", 7);
    keys(&mut t, "vkhy");
    assert_eq!(t.text(), "abcd\nefgh");
    assert_eq!(t.kill_buffer, "bcd\nefg");
    assert_eq!(t.cursor(), 1);
    assert!(t.vim_commands.last_change.is_empty());
    assert!(t.is_vim_normal_mode());
}

#[test]
fn visual_delete_repeats_atomic_width_and_ignores_dot_count_like_vim() {
    let mut t = textarea("e\u{301}👩‍💻中abcd", 0);
    keys(&mut t, "vld");
    assert_eq!(t.text(), "中abcd");
    keys(&mut t, "3.");
    assert_eq!(t.text(), "bcd");
    assert_eq!(t.kill_buffer, "中a");
    t.set_cursor(t.text().len() - 1);
    keys(&mut t, ".");
    assert_eq!(t.text(), "bc");
    assert_eq!(t.cursor(), 1);
}

#[test]
fn visual_multiline_dot_uses_last_line_column_and_clamps_short_lines() {
    let mut t = textarea("abcd\nefgh\ni\njklmn\nop", 1);
    keys(&mut t, "vjld");
    assert_eq!(t.text(), "ah\ni\njklmn\nop");
    t.set_cursor(3);
    keys(&mut t, ".");
    assert_eq!(t.text(), "ah\nmn\nop");
    t.set_cursor(4);
    keys(&mut t, ".");
    assert_eq!(t.text(), "ah\nm");
    assert_eq!(t.kill_buffer, "n\nop");
}

#[test]
fn visual_change_records_complete_insert_session_and_keeps_atomic_elements_whole() {
    let mut t = textarea("ab", 2);
    t.insert_element("<element>");
    t.insert_str("cd");
    t.set_cursor(0);
    keys(&mut t, "v2lcXY");
    assert_eq!(t.text(), "XYcd");
    assert!(t.elements.is_empty());
    assert_eq!(t.kill_buffer, "ab<element>");
    assert_eq!(t.vim_mode_label(), Some("Insert"));
    t.input(KeyEvent::from(KeyCode::Esc));
    t.set_cursor(2);
    keys(&mut t, ".");
    assert_eq!(t.text(), "XYXY");
    assert!(t.is_vim_normal_mode());
}

#[test]
fn visual_empty_edits_and_cancel_preserve_last_change_and_clear_selection() {
    for operator in ['d', 'y', 'c'] {
        let mut t = textarea("x", 0);
        keys(&mut t, "xv");
        let before = format!("{:?}", t.vim_commands.last_change);
        keys(&mut t, &operator.to_string());
        if operator == 'c' {
            assert_eq!(t.vim_mode_label(), Some("Insert"));
            t.input(KeyEvent::from(KeyCode::Esc));
        }
        assert_eq!(t.text(), "");
        assert!(t.vim_visual_range().is_none());
        assert_eq!(format!("{:?}", t.vim_commands.last_change), before);
    }
    let mut t = textarea("\nz", 0);
    keys(&mut t, "vd");
    assert_eq!(t.text(), "z");
    assert_eq!(t.kill_buffer, "\n");
}

#[test]
fn visual_operators_use_configured_normal_bindings_including_digits() {
    for (operator, mode) in [('d', "Normal"), ('c', "Insert"), ('y', "Normal")] {
        let mut t = textarea("abc", 0);
        let mut runtime = RuntimeKeymap::defaults();
        let bindings = match operator {
            'd' => &mut runtime.vim_normal.start_delete_operator,
            'c' => &mut runtime.vim_normal.start_change_operator,
            _ => &mut runtime.vim_normal.start_yank_operator,
        };
        *bindings = vec![crate::key_hint::plain(KeyCode::Char('2'))];
        t.set_keymap_bindings(&runtime);
        keys(&mut t, "vl2");
        assert_eq!(t.text(), if operator == 'y' { "abc" } else { "c" });
        assert_eq!(t.kill_buffer, "ab");
        assert_eq!(t.vim_mode_label(), Some(mode));
    }
}

#[test]
fn visual_operator_binding_is_literal_during_a_pending_find() {
    let mut t = textarea("abcd", 0);
    keys(&mut t, "vfd");
    assert_eq!(t.text(), "abcd");
    assert!(t.is_vim_visual_mode());
    assert_eq!(t.cursor(), 3);
    assert!(t.vim_commands.last_change.is_empty());
    t.input(KeyEvent::from(KeyCode::Esc));
    assert!(t.vim_visual_range().is_none());
}

#[test]
fn visual_line_selection_tracks_whole_lines_in_both_directions_and_switches_shape() {
    let mut t = textarea("abc\ndef\nghi", 5);
    keys(&mut t, "V");
    assert_eq!(t.vim_visual_range(), Some(4..8));
    assert_eq!(t.vim_mode_label(), Some("Visual Line"));
    keys(&mut t, "j");
    assert_eq!(t.vim_visual_range(), Some(4..11));
    keys(&mut t, "kk");
    assert_eq!(t.vim_visual_range(), Some(0..8));
    keys(&mut t, "v");
    assert_eq!(t.vim_visual_range(), Some(1..6));
    keys(&mut t, "V");
    assert_eq!(t.vim_visual_range(), Some(0..8));
    keys(&mut t, "V");
    assert!(t.is_vim_normal_mode());
    assert!(t.vim_visual_range().is_none());
    assert_eq!(t.text(), "abc\ndef\nghi");
}

#[test]
fn visual_line_operators_use_linewise_ranges_and_dot_keeps_the_line_count() {
    for (cursor, sequence) in [(1, "Vjd"), (5, "Vkd")] {
        let mut t = textarea("abc\ndef\nghi\njkl\nmno", cursor);
        keys(&mut t, sequence);
        assert_eq!(t.text(), "ghi\njkl\nmno");
        assert_eq!(t.kill_buffer, "abc\ndef\n");
        assert_eq!(t.kill_buffer_kind, super::super::KillBufferKind::Linewise);
        keys(&mut t, "3.");
        assert_eq!(t.text(), "mno");
        assert_eq!(t.kill_buffer, "ghi\njkl\n");
        assert!(t.is_vim_normal_mode());
    }
    let mut t = textarea("abc\ndef\nghi", 5);
    keys(&mut t, "Vjy");
    assert_eq!(t.text(), "abc\ndef\nghi");
    assert_eq!(t.kill_buffer, "def\nghi");
    assert_eq!(t.kill_buffer_kind, super::super::KillBufferKind::Linewise);
    assert!(t.vim_commands.last_change.is_empty());
    keys(&mut t, "p");
    assert_eq!(t.text(), "abc\ndef\ndef\nghi\nghi");
}

#[test]
fn visual_line_change_preserves_following_empty_line_and_replays_insert_session() {
    for (text, command, expected) in [
        ("one\ntwo\nthree", "VjcXY", "XY\nthree"),
        ("one\n", "VcXY", "XY\n"),
        ("one\n", "VjcXY", "XY"),
        ("one", "VcXY", "XY"),
        ("\n\nlast", "VjcXY", "XY\nlast"),
        ("", "VcXY", "XY"),
    ] {
        let mut t = textarea(text, 0);
        keys(&mut t, command);
        assert_eq!(t.text(), expected);
        assert_eq!(t.kill_buffer_kind, super::super::KillBufferKind::Linewise);
        assert_eq!(t.vim_mode_label(), Some("Insert"));
        t.input(KeyEvent::from(KeyCode::Esc));
        assert!(t.is_vim_normal_mode());
    }
    let mut t = textarea("one\ntwo\nthree\nfour\nfive", 1);
    keys(&mut t, "VjcXY");
    t.input(KeyEvent::from(KeyCode::Esc));
    t.set_cursor(4);
    keys(&mut t, ".");
    assert_eq!(t.text(), "XY\nXY\nfive");
    assert_eq!(t.kill_buffer, "three\nfour\n");
}

#[test]
fn visual_line_empty_and_final_lines_keep_valid_ranges_and_linewise_registers() {
    for (text, cursor, range, register) in [
        ("", 0, 0..0, "\n"),
        ("\n", 0, 0..1, "\n"),
        ("a\n", 2, 2..2, "\n"),
        ("a\nlast", 3, 2..6, "last"),
        ("a\n\nz", 2, 2..3, "\n"),
    ] {
        let mut t = textarea(text, cursor);
        keys(&mut t, "V");
        assert_eq!(t.vim_visual_range(), Some(range));
        keys(&mut t, "y");
        assert_eq!(t.kill_buffer, register);
        assert_eq!(t.kill_buffer_kind, super::super::KillBufferKind::Linewise);
        assert_eq!(t.text(), text);
    }
    let mut t = textarea("a\nlast", 3);
    keys(&mut t, "Vd");
    assert_eq!(t.text(), "a\n");
    assert_eq!(t.kill_buffer, "last");
    let mut t = textarea("a\n", 0);
    keys(&mut t, "Vjy");
    assert_eq!(t.kill_buffer, "a\n\n");
    keys(&mut t, "p");
    assert_eq!(t.text(), "a\na\n\n");
}

#[test]
fn visual_line_render_highlights_full_content_and_newlines_including_empty_eof() {
    let mut t = textarea("abc\ndef\n", 1);
    keys(&mut t, "V2j");
    let area = Rect::new(0, 0, 5, 3);
    let mut buf = Buffer::empty(area);
    StatefulWidgetRef::render_ref(&&t, area, &mut buf, &mut TextAreaState::default());
    for (y, width) in [(0, 4), (1, 4), (2, 1)] {
        for x in 0..width {
            assert!(
                buf[(x, y)]
                    .style()
                    .add_modifier
                    .contains(Modifier::REVERSED)
            );
        }
    }
    t.input(KeyEvent::from(KeyCode::Esc));
    assert!(t.vim_visual_range().is_none());
}

#[test]
fn visual_line_entry_is_configurable_including_digits_and_shift_v() {
    let mut t = textarea("abc\ndef\nghi", 1);
    t.input(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::SHIFT));
    assert_eq!(t.vim_mode_label(), Some("Visual Line"));
    t.input(KeyEvent::from(KeyCode::Esc));
    let mut config = codex_config::types::TuiKeymap::default();
    config.vim_normal.enter_visual_line = Some(codex_config::types::KeybindingsSpec::One(
        codex_config::types::KeybindingSpec("2".to_owned()),
    ));
    t.set_keymap_bindings(&RuntimeKeymap::from_config(&config).unwrap());
    keys(&mut t, "2j");
    assert_eq!(t.vim_visual_range(), Some(0..8));
    keys(&mut t, "2");
    assert!(t.is_vim_normal_mode());
    keys(&mut t, "v2");
    assert_eq!(t.vim_mode_label(), Some("Visual Line"));
}

#[test]
fn visual_line_navigation_and_deletion_preserve_graphemes_and_atomic_elements() {
    let mut t = textarea("e\u{301}", "e\u{301}".len());
    t.insert_element("<element>");
    t.insert_str("\n\nlast");
    t.set_cursor(0);
    keys(&mut t, "Vjld");
    assert_eq!(t.text(), "last");
    assert_eq!(t.kill_buffer, "e\u{301}<element>\n\n");
    assert!(t.elements.is_empty());
}
