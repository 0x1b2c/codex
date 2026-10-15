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
    t.kill_buffer = "previous yank".into();
    t
}

fn keys(t: &mut TextArea, command: &str) {
    for ch in command.chars() {
        t.input(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
    }
}

#[test]
fn inner_counts_include_alternating_words_and_whitespace() {
    for (cursor, command, expected) in [
        (0, "y2iw", "one   "),
        (0, "2yiw", "one   "),
        (0, "2y2iw", "one   two "),
        (0, "y3iw", "one   two"),
        (4, "y2iw", "   two"),
        (4, "y3iw", "   two "),
    ] {
        let mut t = textarea("one   two three", cursor);
        keys(&mut t, command);
        assert_eq!(t.kill_buffer, expected, "{command}");
        assert_eq!(t.text(), "one   two three");
    }
}

#[test]
fn around_counts_select_words_and_the_appropriate_whitespace() {
    for (cursor, command, expected) in [
        (0, "y2aw", "one   two "),
        (0, "y3aw", "one   two three"),
        (4, "y2aw", "   two three"),
        (7, "y2aw", "   two three"),
    ] {
        let mut t = textarea("one   two three", cursor);
        keys(&mut t, command);
        assert_eq!(t.kill_buffer, expected, "{command}");
    }
    let mut t = textarea("one,two three four", 0);
    keys(&mut t, "y2iW");
    assert_eq!(t.kill_buffer, "one,two ");
    keys(&mut t, "y2aW");
    assert_eq!(t.kill_buffer, "one,two three ");
    let mut t = textarea("one\n\ntwo", 0);
    keys(&mut t, "y2iw");
    assert_eq!(t.kill_buffer, "one\n\n");
    let mut t = textarea("one\n  two three", 0);
    keys(&mut t, "y2aw");
    assert_eq!(t.kill_buffer, "one\n  two");
    let mut t = textarea("one,two", 3);
    keys(&mut t, "y2aw");
    assert_eq!(t.kill_buffer, ",two");
}

#[test]
fn brackets_select_the_requested_enclosing_level() {
    for (text, cursor, command, expected) in [
        ("a ((one)) b", 5, "y2i(", "(one)"),
        ("a [[one]] b", 5, "2ya[", "[[one]]"),
        ("a {{{one}}} b", 6, "y3i{", "{{one}}"),
        ("a ((one)) b", 5, "y3i(", "previous yank"),
    ] {
        let mut t = textarea(text, cursor);
        keys(&mut t, command);
        assert_eq!(t.kill_buffer, expected, "{command}");
    }
}

#[test]
fn quote_inner_counts_include_delimiters_without_selecting_the_next_quote() {
    for quote in ['"', '\'', '`'] {
        let text = format!("x {quote}one{quote} y {quote}two{quote}");
        for count in [2, 3, 99] {
            let mut t = textarea(&text, 4);
            keys(&mut t, &format!("y{count}i{quote}"));
            assert_eq!(t.kill_buffer, format!("{quote}one{quote}"));
            keys(&mut t, &format!("y{count}a{quote}"));
            assert_eq!(t.kill_buffer, format!("{quote}one{quote} "));
        }
    }
}

#[test]
fn failed_or_cancelled_objects_leave_counts_and_registers_untouched() {
    for command in ["y99iw", "y99aw", "yi2w", "2yi2w"] {
        let mut t = textarea("one two three", 0);
        keys(&mut t, command);
        assert_eq!(t.kill_buffer, "previous yank", "{command}");
        assert!(!t.is_vim_operator_pending());
        assert_eq!(t.vim_count, None);
        t.set_cursor(0);
        keys(&mut t, "yiw");
        assert_eq!(t.kill_buffer, "one");
    }
    let mut t = textarea("one two three", 0);
    keys(&mut t, "2di");
    t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    keys(&mut t, "diw");
    assert_eq!(t.text(), " two three");
    let mut t = textarea("one two", 0);
    keys(&mut t, "999999999999999999999d999999999999999999999iw");
    assert_eq!(t.text(), "one two");
    assert_eq!(t.kill_buffer, "previous yank");
}

#[test]
fn counted_changes_replay_the_object_and_complete_inserted_text() {
    let mut t = textarea("one two three four five six", 0);
    keys(&mut t, "c3iwXY");
    t.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(t.text(), "XY three four five six");
    keys(&mut t, "w.");
    assert_eq!(t.text(), "XY XY five six");
    assert_eq!(t.kill_buffer, "three four");
}

#[test]
fn counted_objects_preserve_unicode_graphemes_and_atomic_elements() {
    let mut t = textarea("中文 e\u{301} 👩‍💻 last", 0);
    keys(&mut t, "d3iw");
    assert_eq!(t.text(), " 👩‍💻 last");
    t.set_cursor(0);
    t.insert_element("<attachment>");
    t.set_cursor(0);
    keys(&mut t, "d2aw");
    assert_eq!(t.text(), "last");
    assert!(t.elements.is_empty());
}
