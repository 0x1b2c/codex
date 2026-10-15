//! Vim word classes, independent of the regular editor's Unicode word segmentation.

use super::TextArea;
use std::ops::Range;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum VimWordKind {
    Word,
    BigWord,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Space,
    Han,
    Keyword,
    Punctuation,
    Element,
}

fn word_class(ch: char) -> Class {
    match ch {
        ch if ch.is_whitespace() => Class::Space,
        // Unified ideographs, compatibility ideographs, and supplementary Han extensions.
        '\u{3400}'..='\u{4dbf}'
        | '\u{4e00}'..='\u{9fff}'
        | '\u{f900}'..='\u{faff}'
        | '\u{20000}'..='\u{2ffff}'
        | '\u{30000}'..='\u{323af}' => Class::Han,
        ch if ch.is_alphanumeric() || ch == '_' => Class::Keyword,
        _ => Class::Punctuation,
    }
}

impl TextArea {
    fn vim_word_ranges(&self, kind: VimWordKind) -> Vec<Range<usize>> {
        let mut ranges: Vec<Range<usize>> = Vec::new();
        let mut previous = Class::Space;
        let mut start = 0;
        while start < self.text.len() {
            let end = self.next_atomic_boundary(start);
            let class = if self
                .elements
                .iter()
                .any(|element| element.range.contains(&start))
            {
                Class::Element
            } else {
                word_class(
                    self.text[start..end]
                        .chars()
                        .next()
                        .expect("nonempty grapheme"),
                )
            };
            let class = if kind == VimWordKind::BigWord && class != Class::Space {
                Class::Keyword
            } else {
                class
            };
            if class != Class::Space {
                if class == previous && class != Class::Element {
                    ranges.last_mut().expect("previous word").end = end;
                } else {
                    ranges.push(start..end);
                }
            }
            previous = class;
            start = end;
        }
        ranges
    }

    pub(super) fn vim_next_word_start(&self, kind: VimWordKind) -> usize {
        self.vim_word_ranges(kind)
            .into_iter()
            .find(|range| range.start > self.cursor_pos)
            .map_or(self.text.len(), |range| range.start)
    }

    pub(super) fn vim_previous_word_start(&self, kind: VimWordKind) -> usize {
        self.vim_word_ranges(kind)
            .into_iter()
            .rev()
            .find(|range| range.start < self.cursor_pos)
            .map_or(0, |range| range.start)
    }

    pub(super) fn vim_word_end_exclusive(&self, kind: VimWordKind) -> usize {
        self.vim_word_ranges(kind)
            .into_iter()
            .find(|range| self.prev_atomic_boundary(range.end) > self.cursor_pos)
            .map_or(self.text.len(), |range| range.end)
    }

    pub(super) fn vim_word_end_cursor(&self, kind: VimWordKind) -> usize {
        let end = self.vim_word_end_exclusive(kind);
        if end > self.cursor_pos {
            self.prev_atomic_boundary(end)
        } else {
            end
        }
    }

    pub(super) fn vim_word_range(&self, kind: VimWordKind) -> Option<Range<usize>> {
        let ranges = self.vim_word_ranges(kind);
        ranges
            .iter()
            .find(|range| range.contains(&self.cursor_pos))
            .cloned()
            .or_else(|| {
                ranges
                    .into_iter()
                    .find(|range| range.end == self.cursor_pos)
            })
    }
}

#[cfg(test)]
#[path = "vim_words_tests.rs"]
mod tests;
