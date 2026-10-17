//! Return-to-latest affordance in the owned transcript's existing composer gap.
//! The control is hidden whenever the current final transcript row is visible.
//! Rendering owns the hit rectangle; hidden controls never intercept pointer input.

use super::*;
use crossterm::event::MouseButton;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use ratatui::layout::Position as ScreenPosition;
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;

#[derive(Default)]
pub(super) struct FollowControl {
    area: Option<Rect>,
    pointer: Option<ScreenPosition>,
    pressed: bool,
}

impl TranscriptView {
    pub(crate) fn render_follow_control(&mut self, area: Option<Rect>, buf: &mut Buffer) {
        let Some(area) = area.filter(|area| !area.is_empty()) else {
            self.follow_control = FollowControl::default();
            return;
        };
        let Some(label) = self.follow_control_label(area.width) else {
            self.follow_control = FollowControl::default();
            return;
        };
        let width = label.width() as u16;
        let target = Rect::new(
            area.x + (area.width - width) / 2,
            area.y,
            width,
            /*height*/ 1,
        );
        let hovered = self
            .follow_control
            .pointer
            .is_some_and(|point| target.contains(point));
        self.follow_control.area = Some(target);
        let style =
            crate::style::user_message_style().fg(crate::style::user_message_accent_color());
        let style = if hovered {
            style.reversed().bold()
        } else {
            style
        };
        Paragraph::new(label).style(style).render(target, buf);
    }

    pub(super) fn follow_control_label(&self, width: u16) -> Option<String> {
        if self.area.is_empty()
            || self.tail_visible
            || self.highlight.is_some()
            || self.selection.is_some()
            || self.is_search_editing()
        {
            return None;
        }
        let latest_key =
            crate::key_hint::ctrl_alt(crossterm::event::KeyCode::Char('e')).display_label();
        let labels = if self.unseen_activity {
            [
                format!(" New activity · ↓ Back to bottom · {latest_key} "),
                " New activity · ↓ Bottom ".to_owned(),
                " New · ↓ Bottom ".to_owned(),
                " ↓ Bottom ".to_owned(),
                " ↓ ".to_owned(),
            ]
        } else {
            [
                format!(" ↓ Back to bottom · {latest_key} "),
                " ↓ Back to bottom ".to_owned(),
                format!(" ↓ Bottom · {latest_key} "),
                " ↓ Bottom ".to_owned(),
                " ↓ ".to_owned(),
            ]
        };
        labels
            .into_iter()
            .find(|label| label.width() <= usize::from(width))
    }

    pub(super) fn handle_follow_control_mouse(&mut self, event: MouseEvent) -> Option<ViewAction> {
        let control = &mut self.follow_control;
        let area = control.area?;
        let point = ScreenPosition::new(event.column, event.row);
        let was_hovered = control.pointer.is_some_and(|point| area.contains(point));
        let inside = area.contains(point);
        control.pointer = Some(point);
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) if inside => {
                control.pressed = true;
                Some(ViewAction::Changed)
            }
            MouseEventKind::Up(MouseButton::Left) if control.pressed => {
                control.pressed = false;
                if inside {
                    self.jump_to_latest();
                }
                Some(ViewAction::Changed)
            }
            MouseEventKind::Drag(MouseButton::Left) if control.pressed => Some(ViewAction::Changed),
            MouseEventKind::Moved if inside != was_hovered => Some(ViewAction::Changed),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "follow_control_tests.rs"]
mod tests;
