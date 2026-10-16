//! Literal bracket matching and counted percentage jumps share atomic editor boundaries.

use super::TextArea;
use super::VimOperator;
use unicode_segmentation::UnicodeSegmentation;

impl TextArea {
    pub(super) fn vim_matching_pair(&self) -> Option<usize> {
        // Elements are opaque. A bracket with combining marks still occupies one grapheme.
        let brackets: Vec<(usize, char)> = self
            .text
            .grapheme_indices(true)
            .filter_map(|(position, grapheme)| {
                let ch = grapheme.chars().next()?;
                (matches!(ch, '(' | ')' | '[' | ']' | '{' | '}')
                    && !self
                        .elements
                        .iter()
                        .any(|element| element.range.contains(&position)))
                .then_some((position, ch))
            })
            .collect();
        let end = self.end_of_current_line();
        let index = brackets
            .iter()
            .position(|&(position, _)| position >= self.cursor_pos && position < end)?;
        let (_, source) = brackets[index];
        let (partner, forward) = match source {
            '(' => (')', true),
            '[' => (']', true),
            '{' => ('}', true),
            ')' => ('(', false),
            ']' => ('[', false),
            '}' => ('{', false),
            _ => unreachable!("only brackets are collected"),
        };
        let mut depth = 1;
        let mut visit = |&(position, ch): &(usize, char)| {
            if ch == source {
                depth += 1;
            } else if ch == partner {
                depth -= 1;
            }
            (depth == 0).then_some(position)
        };
        if forward {
            brackets[index + 1..].iter().find_map(&mut visit)
        } else {
            brackets[..index].iter().rev().find_map(&mut visit)
        }
    }

    pub(super) fn apply_vim_matching_pair(&mut self, operator: VimOperator) -> bool {
        let Some(target) = self.vim_matching_pair() else {
            return false;
        };
        let origin = self.cursor_pos;
        let end = self.next_atomic_boundary(origin.max(target));
        self.apply_vim_operator_to_range(operator, origin.min(target)..end);
        true
    }

    pub(super) fn vim_percentage_line(&self, percentage: usize) -> Option<usize> {
        if !(1..=100).contains(&percentage) {
            return None;
        }
        let lines = self.text.bytes().filter(|&byte| byte == b'\n').count() + 1;
        // Vim rounds up: even a small positive percentage addresses at least line one.
        let line = (lines / 100) * percentage + ((lines % 100) * percentage).div_ceil(100);
        if line == 1 {
            Some(0)
        } else {
            self.text
                .bytes()
                .enumerate()
                .filter(|&(_, byte)| byte == b'\n')
                .nth(line - 2)
                .map(|(position, _)| position + 1)
        }
    }
}

#[cfg(test)]
#[path = "vim_match_pair_tests.rs"]
mod tests;
