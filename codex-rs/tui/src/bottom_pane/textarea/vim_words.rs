//! Vim word classes, independent of the regular editor's Unicode word segmentation.

use super::TextArea;
use super::vim::VimTextObjectScope;
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
    fn vim_whitespace_at(&self, pos: usize) -> bool {
        !self
            .elements
            .iter()
            .any(|element| element.range.contains(&pos))
            && self.text[pos..]
                .chars()
                .next()
                .is_some_and(char::is_whitespace)
    }

    /// Inner whitespace objects stay within the current logical line.
    pub(super) fn vim_whitespace_range(&self) -> Option<Range<usize>> {
        let line_start = self.beginning_of_current_line();
        let line_end = self.end_of_current_line();
        if self.cursor_pos < line_end && !self.vim_whitespace_at(self.cursor_pos) {
            return None;
        }
        let mut start = self.cursor_pos;
        while start > line_start {
            let previous = self.prev_atomic_boundary(start);
            if !self.vim_whitespace_at(previous) {
                break;
            }
            start = previous;
        }
        let mut end = self.cursor_pos;
        while end < line_end && self.vim_whitespace_at(end) {
            end = self.next_atomic_boundary(end);
        }
        (start < end || line_start == line_end).then_some(start..end)
    }

    pub(super) fn vim_whitespace_text_object_range(
        &self,
        whitespace: Range<usize>,
        scope: VimTextObjectScope,
        kind: VimWordKind,
    ) -> Option<Range<usize>> {
        if scope == VimTextObjectScope::Inner {
            return (!whitespace.is_empty()).then_some(whitespace);
        }
        let next_word = self
            .vim_word_ranges(kind)
            .into_iter()
            .find(|range| range.start >= whitespace.end);
        let limit = next_word
            .as_ref()
            .map_or(self.text.len(), |range| range.start);
        let mut pos = whitespace.end;
        while pos < limit {
            if self.text[pos..].starts_with('\n') {
                let next_line = pos + 1;
                // Empty lines count as words, including an empty final line.
                if next_line == self.text.len() || self.text[next_line..].starts_with('\n') {
                    return Some(whitespace.start..next_line);
                }
            }
            pos = self.next_atomic_boundary(pos);
        }
        next_word.map(|range| whitespace.start..range.end)
    }

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
