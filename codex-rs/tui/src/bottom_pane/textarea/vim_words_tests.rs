use super::super::TextArea;
use crate::keymap::RuntimeKeymap;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;

fn textarea(text: &str) -> TextArea {
    let mut textarea = TextArea::new();
    textarea.insert_str(text);
    textarea.set_cursor(/*pos*/ 0);
    textarea.set_vim_enabled(/*enabled*/ true);
    textarea
}

fn keys(textarea: &mut TextArea, keys: &str) {
    for key in keys.chars() {
        let code = if key == '\x1b' {
            KeyCode::Esc
        } else {
            KeyCode::Char(key)
        };
        textarea.input(KeyEvent::new(code, KeyModifiers::NONE));
    }
}

#[test]
fn vim_words_stop_at_han_punctuation_and_keyword_boundaries() {
    let mut t = textarea("修改这里，保留那里。abc_def42!=next");
    let mut states = Vec::new();
    for key in "wwwwwwbbbbbb".chars() {
        keys(&mut t, &key.to_string());
        states.push(format!(
            "{key}: {}|{}",
            &t.text()[..t.cursor()],
            &t.text()[t.cursor()..]
        ));
    }
    insta::assert_snapshot!(states.join("\n"), @r"
w: 修改这里|，保留那里。abc_def42!=next
w: 修改这里，|保留那里。abc_def42!=next
w: 修改这里，保留那里|。abc_def42!=next
w: 修改这里，保留那里。|abc_def42!=next
w: 修改这里，保留那里。abc_def42|!=next
w: 修改这里，保留那里。abc_def42!=|next
b: 修改这里，保留那里。abc_def42|!=next
b: 修改这里，保留那里。|abc_def42!=next
b: 修改这里，保留那里|。abc_def42!=next
b: 修改这里，|保留那里。abc_def42!=next
b: 修改这里|，保留那里。abc_def42!=next
b: |修改这里，保留那里。abc_def42!=next
");
    keys(&mut t, "e");
    assert_eq!(t.cursor(), "修改这".len());
    keys(&mut t, "el");
    assert_eq!(t.cursor(), "修改这里，".len());
    keys(&mut t, "h");
    assert_eq!(t.cursor(), "修改这里".len());
}

#[test]
fn vim_big_words_ignore_punctuation_and_support_shift_events() {
    let mut t = textarea("中文，abc_def! next\n末尾。");
    keys(&mut t, "W");
    assert_eq!(t.cursor(), "中文，abc_def! ".len());
    keys(&mut t, "E");
    assert_eq!(t.cursor(), "中文，abc_def! nex".len());
    keys(&mut t, "EB");
    assert_eq!(t.cursor(), "中文，abc_def! next\n".len());
    t.input(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::SHIFT));
    assert_eq!(t.cursor(), "中文，abc_def! ".len());
    t.input(KeyEvent::new(KeyCode::Char('W'), KeyModifiers::SHIFT));
    assert_eq!(t.cursor(), "中文，abc_def! next\n".len());
}

#[test]
fn vim_operators_and_text_objects_share_word_boundaries() {
    for (command, remaining, killed) in [
        ("dw", "，保留那里", "修改这里"),
        ("de", "，保留那里", "修改这里"),
        ("diw", "，保留那里", "修改这里"),
        ("ciw", "，保留那里", "修改这里"),
        ("cw", "，保留那里", "修改这里"),
        ("yiw", "修改这里，保留那里", "修改这里"),
        ("dW", "", "修改这里，保留那里"),
        ("dE", "", "修改这里，保留那里"),
        ("diW", "", "修改这里，保留那里"),
        ("ciW", "", "修改这里，保留那里"),
        ("yiW", "修改这里，保留那里", "修改这里，保留那里"),
    ] {
        let mut t = textarea("修改这里，保留那里");
        keys(&mut t, command);
        assert_eq!(
            (t.text(), t.kill_buffer.as_str()),
            (remaining, killed),
            "{command}"
        );
    }
    for command in ["cw", "cW"] {
        let mut t = textarea("中文   next");
        keys(&mut t, command);
        assert_eq!((t.text(), t.vim_mode_label()), ("   next", Some("Insert")));
        let mut t = textarea("   \nnext");
        keys(&mut t, command);
        assert_eq!(t.text(), "\nnext");
    }
}

#[test]
fn vim_word_edits_repeat_as_complete_changes() {
    let mut t = textarea("修改这里 保留那里");
    keys(&mut t, "cwX\x1bw.");
    assert_eq!(t.text(), "X X");
}

#[test]
fn vim_words_preserve_graphemes_and_atomic_elements() {
    let mut t = textarea("a\u{301}_b👩‍💻!?中文");
    keys(&mut t, "e");
    assert_eq!(t.cursor(), "a\u{301}_".len());
    keys(&mut t, "wl");
    assert_eq!(t.cursor(), "a\u{301}_b👩‍💻".len());
    let mut t = textarea("中文 ");
    t.set_cursor(t.text().len());
    t.insert_element("<image，中文>");
    t.insert_str(" next");
    t.set_cursor(/*pos*/ 0);
    keys(&mut t, "we");
    assert_eq!(t.cursor(), "中文 <image，中文> nex".len());
    t.set_cursor("中文 ".len());
    keys(&mut t, "diw");
    assert_eq!(
        (t.text(), t.element_payloads()),
        ("中文  next", Vec::<String>::new())
    );
}

#[test]
fn big_word_bindings_can_be_reassigned() {
    let mut config = codex_config::types::TuiKeymap::default();
    config.vim_normal.move_big_word_forward = Some(codex_config::types::KeybindingsSpec::One(
        codex_config::types::KeybindingSpec("q".to_owned()),
    ));
    config.vim_operator.motion_big_word_end = config.vim_normal.move_big_word_forward.clone();
    let keymap = RuntimeKeymap::from_config(&config).expect("reassign WORD bindings");
    let mut t = textarea("中文，abc next!");
    t.set_keymap_bindings(&keymap);
    keys(&mut t, "q");
    assert_eq!(t.cursor(), "中文，abc ".len());
    keys(&mut t, "dq");
    assert_eq!(t.text(), "中文，abc ");
}

#[test]
fn vim_words_separate_adjacent_han_and_latin_and_group_punctuation() {
    let mut t = textarea("hello你好world？！，再见");
    for prefix in [
        "hello",
        "hello你好",
        "hello你好world",
        "hello你好world？！，",
    ] {
        keys(&mut t, "w");
        assert_eq!(t.cursor(), prefix.len());
    }
    t.set_cursor("hello你好world".len());
    keys(&mut t, "yiw");
    assert_eq!(t.kill_buffer, "？！，");
    keys(&mut t, "diw");
    assert_eq!(t.text(), "hello你好world再见");
    keys(&mut t, "db");
    assert_eq!(t.text(), "hello你好再见");
    keys(&mut t, "dB");
    assert_eq!(t.text(), "再见");
}

#[test]
fn vim_words_skip_whitespace_and_stop_at_buffer_boundaries() {
    let mut t = textarea(" \t中文\n\n下一段  ");
    keys(&mut t, "b");
    assert_eq!(t.cursor(), 0);
    for prefix in [" \t", " \t中文\n\n", " \t中文\n\n下一段  "] {
        keys(&mut t, "w");
        assert_eq!(t.cursor(), prefix.len());
    }
    keys(&mut t, "b");
    assert_eq!(t.cursor(), " \t中文\n\n".len());
    keys(&mut t, "e");
    assert_eq!(t.cursor(), " \t中文\n\n下一".len());
    let mut t = textarea("");
    keys(&mut t, "wbeWBE");
    assert_eq!((t.text(), t.cursor()), ("", 0));
}

#[test]
fn regular_editor_keeps_existing_chinese_word_navigation() {
    let mut t = textarea("你好世界");
    t.set_vim_enabled(/*enabled*/ false);
    assert_eq!((t.end_of_next_word(), t.beginning_of_next_word()), (3, 3));
    t.set_cursor(t.text().len());
    assert_eq!(t.beginning_of_previous_word(), 9);
}

#[test]
fn vim_atomic_elements_form_words_separate_from_adjacent_punctuation() {
    let mut t = textarea("中文，");
    t.set_cursor(t.text().len());
    t.insert_element("<image，中文>");
    t.insert_str("！后续");
    t.set_cursor("中文".len());
    keys(&mut t, "w");
    assert_eq!(t.cursor(), "中文，".len());
    keys(&mut t, "w");
    assert_eq!(t.cursor(), "中文，<image，中文>".len());
    keys(&mut t, "bdiw");
    assert_eq!(
        (t.text(), t.element_payloads()),
        ("中文，！后续", Vec::<String>::new())
    );
}

#[test]
fn vim_inner_word_objects_select_the_current_whitespace_run() {
    for (text, cursors, expected) in [
        ("one   two", vec![3, 4, 5], "   "),
        (" \t two", vec![0, 1, 2], " \t "),
        ("one   ", vec![3, 4, 5], "   "),
        ("   ", vec![0, 1, 2], "   "),
        ("one\n  two", vec![4, 5], "  "),
        ("one  \n  two", vec![3, 4], "  "),
    ] {
        for cursor in cursors {
            for command in ["yiw", "yiW"] {
                let mut t = textarea(text);
                t.set_cursor(cursor);
                keys(&mut t, command);
                assert_eq!(t.kill_buffer, expected, "{command} at {cursor} in {text:?}");
                assert_eq!(t.text(), text);
            }
        }
    }
}

#[test]
fn vim_around_word_objects_include_whitespace_and_the_next_word() {
    for (text, cursor, word, big_word) in [
        ("one   two!  three", 3, "   two", "   two!"),
        ("one   two!  three", 4, "   two", "   two!"),
        ("one   two!  three", 5, "   two", "   two!"),
        ("   !two", 0, "   !", "   !two"),
        ("one  \n  two!", 4, "  \n  two", "  \n  two!"),
        ("one\n\ntwo!", 4, "\ntwo", "\ntwo!"),
        ("one  \n\ntwo", 3, "  \n", "  \n"),
        ("one\n\n\ntwo", 4, "\n", "\n"),
    ] {
        for (command, expected) in [("yaw", word), ("yaW", big_word)] {
            let mut t = textarea(text);
            t.set_cursor(cursor);
            keys(&mut t, command);
            assert_eq!(t.kill_buffer, expected, "{command} at {cursor} in {text:?}");
            assert_eq!(t.text(), text);
        }
    }
}

#[test]
fn vim_around_trailing_whitespace_does_not_edit_or_replace_the_register() {
    for (text, cursor) in [("one   ", 4), ("   ", 1), ("one\n  ", 4)] {
        for command in ["daw", "daW", "caw", "caW", "yaw", "yaW"] {
            let mut t = textarea(text);
            t.kill_buffer = "keep".to_owned();
            t.set_cursor(cursor);
            keys(&mut t, command);
            assert_eq!(
                (t.text(), t.kill_buffer.as_str(), t.vim_mode_label()),
                (text, "keep", Some("Normal")),
                "{command} at {cursor} in {text:?}"
            );
        }
    }
    for command in ["diw", "diW", "ciw", "ciW", "yiw", "yiW"] {
        let mut t = textarea("one\n\ntwo");
        t.kill_buffer = "keep".to_owned();
        t.set_cursor(/*pos*/ 4);
        keys(&mut t, command);
        assert_eq!((t.text(), t.kill_buffer.as_str()), ("one\n\ntwo", "keep"));
        assert_eq!(t.vim_mode_label(), Some("Normal"));
    }
}

#[test]
fn vim_whitespace_edits_repeat_the_resolved_text_object() {
    let mut t = textarea("one   two  three");
    t.set_cursor(/*pos*/ 4);
    keys(&mut t, "diw");
    t.set_cursor(/*pos*/ 6);
    keys(&mut t, ".");
    assert_eq!(t.text(), "onetwothree");

    let mut t = textarea("one   two  three");
    t.set_cursor(/*pos*/ 4);
    keys(&mut t, "ciWX\x1b");
    t.set_cursor(/*pos*/ 7);
    keys(&mut t, ".");
    assert_eq!(t.text(), "oneXtwoXthree");

    let mut t = textarea("one   two!  three");
    t.set_cursor(/*pos*/ 4);
    keys(&mut t, "daW");
    insta::assert_snapshot!(format!("{}|{}", &t.text()[..t.cursor()], &t.text()[t.cursor()..]), @"one|  three");
}

#[test]
fn vim_delete_change_and_yank_share_whitespace_text_object_ranges() {
    for (object, killed, remaining) in [
        ("iw", "   ", "onetwo!  three"),
        ("iW", "   ", "onetwo!  three"),
        ("aw", "   two", "one!  three"),
        ("aW", "   two!", "one  three"),
    ] {
        for operator in ['d', 'c', 'y'] {
            let mut t = textarea("one   two!  three");
            t.set_cursor(/*pos*/ 4);
            keys(&mut t, &format!("{operator}{object}"));
            assert_eq!(t.kill_buffer, killed);
            assert_eq!(
                t.text(),
                if operator == 'y' {
                    "one   two!  three"
                } else {
                    remaining
                }
            );
            assert_eq!(
                t.vim_mode_label(),
                Some(if operator == 'c' { "Insert" } else { "Normal" })
            );
        }
    }
}

#[test]
fn vim_whitespace_objects_preserve_graphemes_and_atomic_elements() {
    let mut t = textarea("  e\u{301}👩‍💻 next");
    keys(&mut t, "yaw");
    assert_eq!(t.kill_buffer, "  e\u{301}");
    keys(&mut t, "yaW");
    assert_eq!(t.kill_buffer, "  e\u{301}👩‍💻");

    let mut t = textarea("  ");
    t.set_cursor(t.text().len());
    t.insert_element("<image with spaces>");
    t.insert_str(" next");
    t.set_cursor(/*pos*/ 1);
    keys(&mut t, "daw");
    assert_eq!(
        (t.text(), t.kill_buffer.as_str()),
        (" next", "  <image with spaces>")
    );
    assert!(t.element_payloads().is_empty());
}
