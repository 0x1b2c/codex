//! Text object counts resolve a single range before editing or recording dot replay.

use super::TextArea;
use super::VimTextObject;
use super::VimTextObjectScope;
use super::vim_words::VimWordKind;
use std::ops::Range;

impl TextArea {
    pub(super) fn vim_counted_text_object_range(
        &self,
        object: VimTextObject,
        scope: VimTextObjectScope,
        count: usize,
    ) -> Option<Range<usize>> {
        if count == 1 {
            return self.text_object_range(object, scope);
        }
        match object {
            VimTextObject::Word | VimTextObject::BigWord => {
                let kind = if object == VimTextObject::Word {
                    VimWordKind::Word
                } else {
                    VimWordKind::BigWord
                };
                self.vim_counted_word_object_range(scope, kind, count)
            }
            VimTextObject::Parentheses => {
                self.paired_text_object_range_count(scope, '(', ')', count)
            }
            VimTextObject::Brackets => self.paired_text_object_range_count(scope, '[', ']', count),
            VimTextObject::Braces => self.paired_text_object_range_count(scope, '{', '}', count),
            // Vim's counted inner quote includes the delimiters; a larger count does
            // not advance to another pair. Around quotes also include whitespace.
            _ => {
                let pair = self.text_object_range(object, VimTextObjectScope::Around)?;
                Some(if scope == VimTextObjectScope::Around {
                    self.expand_word_around(pair)
                } else {
                    pair
                })
            }
        }
    }

    fn vim_counted_word_object_range(
        &self,
        scope: VimTextObjectScope,
        kind: VimWordKind,
        count: usize,
    ) -> Option<Range<usize>> {
        let words = self.vim_word_ranges(kind);
        let whitespace = self.vim_whitespace_range();
        if scope == VimTextObjectScope::Around {
            let index = if let Some(space) = &whitespace {
                words.iter().position(|word| word.start >= space.end)
            } else {
                words
                    .iter()
                    .position(|word| word.contains(&self.cursor_pos))
                    .or_else(|| words.iter().position(|word| word.end == self.cursor_pos))
            }?;
            let last = words.get(index.checked_add(count.checked_sub(1)?)?)?;
            let first = &words[index];
            if let Some(whitespace) = whitespace {
                return Some(whitespace.start..last.end);
            }
            let following = if self.text[first.start..last.end].contains('\n') {
                last.end
            } else {
                self.following_whitespace_end(last.end)
            };
            let start = if following > last.end {
                first.start
            } else {
                self.preceding_whitespace_start(first.start)
            };
            return Some(start..following);
        }

        // Each nonempty whitespace run is one inner object, just like each word
        // class. Build the units once rather than rescanning for every count.
        let mut units = Vec::with_capacity(words.len().saturating_mul(2).saturating_add(1));
        let mut end = 0;
        for word in words {
            if end < word.start {
                units.push(end..word.start);
            }
            end = word.end;
            units.push(word);
        }
        if end < self.text.len() {
            units.push(end..self.text.len());
        }
        let index = units
            .iter()
            .position(|unit| unit.contains(&self.cursor_pos))?;
        let last = units.get(index.checked_add(count.checked_sub(1)?)?)?;
        let start = whitespace.map_or(units[index].start, |space| space.start);
        Some(start..last.end)
    }
}

#[cfg(test)]
#[path = "vim_text_object_counts_tests.rs"]
mod tests;
