use super::TextArea;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;

fn textarea(text: &str) -> TextArea {
    let mut textarea = TextArea::new();
    textarea.insert_str(text);
    textarea.set_vim_enabled(/*enabled*/ true);
    textarea
}

fn keys(textarea: &mut TextArea, keys: &str) {
    for key in keys.chars() {
        textarea.input(KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE));
    }
}

#[test]
fn vim_horizontal_navigation_stays_on_editable_items_in_the_logical_line() {
    let mut t = textarea("ab\n\nc\nlong");
    let mut states = Vec::new();
    for (start, key, expected) in [
        (0, "h", 0),
        (1, "l", 1),
        (2, "l", 1),
        (3, "h", 3),
        (3, "l", 3),
        (4, "l", 4),
        (5, "l", 4),
        (6, "h", 6),
        (10, "l", 9),
    ] {
        t.set_cursor(start);
        keys(&mut t, key);
        assert_eq!(t.cursor(), expected, "{key} from byte {start}");
        states.push(format!(
            "{key} from {start}: {}|{}",
            &t.text()[..t.cursor()].replace('\n', "\\n"),
            &t.text()[t.cursor()..].replace('\n', "\\n")
        ));
    }
    insta::assert_snapshot!(states.join("\n"), @r"
h from 0: |ab\n\nc\nlong
l from 1: a|b\n\nc\nlong
l from 2: a|b\n\nc\nlong
h from 3: ab\n|\nc\nlong
l from 3: ab\n|\nc\nlong
l from 4: ab\n\n|c\nlong
l from 5: ab\n\n|c\nlong
h from 6: ab\n\nc\n|long
l from 10: ab\n\nc\nlon|g
");

    for text in ["", "\n", "a\n"] {
        let mut t = textarea(text);
        let end = t.text().len();
        t.set_cursor(end);
        keys(&mut t, "hl");
        assert_eq!(t.cursor(), end);
    }
}

#[test]
fn vim_horizontal_navigation_preserves_graphemes_and_atomic_elements() {
    let mut t = textarea("e\u{301}👩‍💻中");
    t.insert_element("<element>");
    t.insert_str("\nz");
    t.set_cursor(/*pos*/ 0);
    let mut positions = Vec::new();
    for key in "llllhhhh".chars() {
        keys(&mut t, &key.to_string());
        positions.push(t.cursor());
    }
    let accent_end = "e\u{301}".len();
    let emoji_end = "e\u{301}👩‍💻".len();
    let han_end = "e\u{301}👩‍💻中".len();
    assert_eq!(
        positions,
        vec![
            accent_end, emoji_end, han_end, han_end, emoji_end, accent_end, 0, 0
        ]
    );
}

#[test]
fn vim_horizontal_operators_stop_at_newlines_but_can_delete_the_last_item() {
    for (text, start, command, remaining, killed) in [
        ("ab\ncd", 3, "dh", "ab\ncd", ""),
        ("ab\ncd", 1, "dl", "a\ncd", "b"),
        ("ab\ncd", 4, "dl", "ab\nc", "d"),
        ("ab\n\ncd", 3, "dl", "ab\n\ncd", ""),
        ("a\nb", 0, "dl", "\nb", "a"),
        ("a\nb", 2, "cl", "a\n", "b"),
        ("ab\ncd", 1, "yl", "ab\ncd", "b"),
    ] {
        let mut t = textarea(text);
        t.set_cursor(start);
        keys(&mut t, command);
        assert_eq!((t.text(), t.kill_buffer.as_str()), (remaining, killed));
    }

    let mut t = textarea("a");
    t.insert_element("<element>");
    t.insert_str("\nb");
    t.set_cursor(/*pos*/ 1);
    keys(&mut t, "dl");
    assert_eq!((t.text(), t.kill_buffer.as_str()), ("a\nb", "<element>"));
}

#[test]
fn insert_mode_horizontal_arrows_can_still_cross_newlines() {
    let mut t = textarea("a\nb");
    t.enter_vim_insert_mode();
    t.set_cursor(/*pos*/ 1);
    t.input(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    assert_eq!(t.cursor(), 2);
    t.input(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    assert_eq!(t.cursor(), 1);
}
