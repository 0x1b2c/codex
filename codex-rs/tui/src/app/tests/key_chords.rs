use super::App;
use super::AppServerSession;
use super::Result;
use super::RuntimeKeymap;
use super::TuiEvent;
use super::make_test_app;
use super::make_test_app_with_channels;
use super::start_config_write_test_app_server;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::chatwidget::tests::helpers::render_bottom_popup;
use crate::chatwidget::tests::helpers::set_active_cell;
use crate::keymap::KeymapContext;
use crate::pager_overlay::TranscriptHistoryState;
use crate::test_support::test_path_display;
use crate::tui::Tui;
use codex_app_server_protocol::ToolRequestUserInputOption;
use codex_app_server_protocol::ToolRequestUserInputParams;
use codex_app_server_protocol::ToolRequestUserInputQuestion;
use codex_config::types::KeybindingSpec;
use codex_config::types::KeybindingsSpec;
use codex_config::types::TuiKeymap;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;

async fn chord_app() -> Result<(Box<App>, Tui, AppServerSession)> {
    let mut app = make_test_app().await;
    let mut config = TuiKeymap::default();
    config.global.open_transcript = Some(KeybindingsSpec::One(KeybindingSpec(
        "ctrl-x ctrl-t".to_string(),
    )));
    config.chat.interrupt_turn = Some(KeybindingsSpec::One(KeybindingSpec(
        "ctrl-x ctrl-u".to_string(),
    )));
    config.list.move_down = Some(KeybindingsSpec::One(KeybindingSpec("ctrl-x j".to_string())));
    config.list.accept = Some(KeybindingsSpec::One(KeybindingSpec(
        "ctrl-x enter".to_string(),
    )));
    let runtime =
        RuntimeKeymap::from_config(&config).map_err(|error| color_eyre::eyre::eyre!(error))?;
    app.chat_widget.apply_keymap_update(config, &runtime);
    app.keymap = runtime;

    let app_server = start_config_write_test_app_server(&app).await?;
    let tui = crate::tui::test_support::make_test_tui()?;
    Ok((app, tui, app_server))
}

async fn press(
    app: &mut App,
    tui: &mut Tui,
    app_server: &mut AppServerSession,
    key: KeyEvent,
) -> Result<()> {
    app.handle_tui_event(tui, app_server, TuiEvent::Key(key))
        .await?;
    Ok(())
}

fn ctrl(ch: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(ch), KeyModifiers::CONTROL)
}

async fn vim_paging_app() -> Result<(
    Box<App>,
    Tui,
    AppServerSession,
    tokio::sync::mpsc::UnboundedReceiver<super::AppEvent>,
)> {
    let (mut app, events, _operations) = super::make_test_app_with_channels().await;
    let server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    app.chat_widget.toggle_vim_mode_and_notify();
    app.transcript_cells = vec![std::sync::Arc::new(
        crate::history_cell::PlainHistoryCell::new(
            (0..200)
                .map(|row| ratatui::text::Line::from(format!("row {row}")))
                .collect(),
        ),
    )];
    let size = tui.prepare_draw_size()?;
    app.render_owned_transcript(&mut tui, size)?;
    app.transcript_view.jump_to_beginning(&app.transcript_cells);
    app.transcript_view.scroll(&app.transcript_cells, 60);
    Ok((app, tui, server, events))
}

fn paging_top_row(app: &mut App, tui: &mut Tui) -> Result<(isize, isize)> {
    let size = tui.prepare_draw_size()?;
    app.render_owned_transcript(tui, size)?;
    let remaining = app.transcript_view.remaining_area();
    let area = ratatui::layout::Rect::new(0, 0, remaining.width, remaining.y);
    let mut buffer = ratatui::buffer::Buffer::empty(area);
    app.transcript_view
        .render(area, &mut buffer, &app.transcript_cells);
    let first_row = buffer
        .content
        .iter()
        .take(usize::from(area.width))
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();
    let row = first_row
        .trim()
        .strip_prefix("row ")
        .expect("numbered history row")
        .parse()?;
    Ok((row, isize::from(area.height)))
}

fn paging_draft_cursor(app: &App) -> Option<(u16, u16)> {
    use crate::render::renderable::Renderable;

    app.chat_widget
        .bottom_pane_renderable(crate::bottom_pane::ComposerRenderOptions::default())
        .cursor_pos(ratatui::layout::Rect::new(0, 0, 80, 12))
}

#[tokio::test]
async fn latest_shortcut_preserves_every_composer_mode_while_work_is_running() -> Result<()> {
    for (mode, enhanced) in ["normal", "insert", "visual", "non-vim"]
        .into_iter()
        .flat_map(|mode| [false, true].map(|enhanced| (mode, enhanced)))
    {
        let (mut app, mut tui, mut server, mut events) = vim_paging_app().await?;
        app.enhanced_keys_supported = enhanced;
        crate::app::owned_transcript::tests::attach_thread(
            &mut app,
            codex_protocol::ThreadId::new(),
        );
        app.chat_widget
            .apply_external_edit("preserved draft".into());
        press(&mut app, &mut tui, &mut server, KeyCode::Esc.into()).await?;
        match mode {
            "insert" => press(&mut app, &mut tui, &mut server, KeyCode::Char('i').into()).await?,
            "visual" => press(&mut app, &mut tui, &mut server, KeyCode::Char('v').into()).await?,
            "non-vim" => app.chat_widget.toggle_vim_mode_and_notify(),
            _ => {}
        }
        app.chat_widget.handle_server_notification(
            codex_app_server_protocol::ServerNotification::TurnStarted(
                codex_app_server_protocol::TurnStartedNotification {
                    thread_id: app.chat_widget.thread_id().unwrap().to_string(),
                    turn: codex_app_server_protocol::Turn {
                        id: "active-turn".into(),
                        root_turn_id: None,
                        items_view: codex_app_server_protocol::TurnItemsView::Full,
                        items: Vec::new(),
                        status: codex_app_server_protocol::TurnStatus::InProgress,
                        error: None,
                        started_at: None,
                        completed_at: None,
                        duration_ms: None,
                    },
                },
            ),
            None,
        );
        assert!(app.chat_widget.is_task_running_for_test());
        app.transcript_view.scroll(&app.transcript_cells, -20);
        let draft = app.chat_widget.capture_thread_input_state();
        let cursor = paging_draft_cursor(&app);
        let contexts = app.active_keymap_contexts();
        while events.try_recv().is_ok() {}
        let latest = KeyEvent::new(
            KeyCode::Char('e'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
        );
        let release = KeyEvent::new_with_kind(
            latest.code,
            latest.modifiers,
            crossterm::event::KeyEventKind::Release,
        );
        press(&mut app, &mut tui, &mut server, release).await?;
        assert!(!app.transcript_view.is_following());
        for _ in 0..2 {
            press(&mut app, &mut tui, &mut server, latest).await?;
            assert!(app.transcript_view.is_following(), "{mode}");
            assert_eq!(
                app.chat_widget.capture_thread_input_state(),
                draft,
                "{mode}"
            );
            assert_eq!(paging_draft_cursor(&app), cursor, "{mode}");
            assert_eq!(app.active_keymap_contexts(), contexts, "{mode}");
        }
        let repeat = KeyEvent::new_with_kind(
            latest.code,
            latest.modifiers,
            crossterm::event::KeyEventKind::Repeat,
        );
        press(&mut app, &mut tui, &mut server, repeat).await?;
        assert!(app.transcript_view.is_following());
        assert_eq!(app.chat_widget.capture_thread_input_state(), draft);
        assert!(
            !std::iter::from_fn(|| events.try_recv().ok()).any(|event| matches!(
                event,
                super::AppEvent::CodexOp(_) | super::AppEvent::Exit(_)
            ))
        );
        server.shutdown().await?;
        tui.set_owned_screen(false)?;
    }
    Ok(())
}

#[tokio::test]
async fn latest_shortcut_does_not_take_keys_from_a_composer_popup() -> Result<()> {
    let (mut app, mut tui, mut server, _events) = vim_paging_app().await?;
    app.chat_widget.apply_external_edit("/".into());
    assert!(!app.chat_widget.no_modal_or_popup_active());
    let latest = TuiEvent::Key(KeyEvent::new(
        KeyCode::Char('e'),
        KeyModifiers::CONTROL | KeyModifiers::ALT,
    ));
    assert!(!app.handle_owned_transcript_event(&mut tui, &mut server, &latest)?);
    assert!(!app.transcript_view.is_following());
    server.shutdown().await?;
    tui.set_owned_screen(false)?;
    Ok(())
}

#[tokio::test]
async fn vim_normal_pages_history_without_changing_the_draft_or_cursor() -> Result<()> {
    for (key, direction, half) in [
        ('u', -1, true),
        ('d', 1, true),
        ('b', -1, false),
        ('f', 1, false),
    ] {
        let (mut app, mut tui, mut server, _events) = vim_paging_app().await?;
        app.chat_widget.insert_str("preserved draft");
        press(&mut app, &mut tui, &mut server, KeyCode::Char('h').into()).await?;
        let draft = app.chat_widget.capture_thread_input_state();
        let cursor = paging_draft_cursor(&app);
        assert!(cursor.is_some());
        let (before, height) = paging_top_row(&mut app, &mut tui)?;
        assert!(matches!(
            app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(ctrl(key)))
                .await?,
            super::AppRunControl::Continue
        ));
        let (after, _) = paging_top_row(&mut app, &mut tui)?;
        let distance = if half {
            (height + 1) / 2
        } else {
            (height - 1).max(1)
        };
        assert_eq!(after - before, direction * distance, "Ctrl+{key}");
        assert_eq!(
            app.chat_widget.capture_thread_input_state(),
            draft,
            "Ctrl+{key}"
        );
        assert_eq!(paging_draft_cursor(&app), cursor, "Ctrl+{key}");
        assert!(
            app.active_keymap_contexts()
                .contains(KeymapContext::VimNormal)
        );
        server.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn vim_normal_jumps_to_history_extents_without_changing_any_draft() -> Result<()> {
    for text in ["", "one\ntwo\nthree"] {
        for bottom in [
            KeyCode::Char('G').into(),
            KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT),
        ] {
            let (mut app, mut tui, mut server, _events) = vim_paging_app().await?;
            app.chat_widget.insert_str(text);
            press(&mut app, &mut tui, &mut server, KeyCode::Char('h').into()).await?;
            let draft = app.chat_widget.capture_thread_input_state();
            let cursor = paging_draft_cursor(&app);
            let before = paging_top_row(&mut app, &mut tui)?.0;
            press(&mut app, &mut tui, &mut server, KeyCode::Char('g').into()).await?;
            assert!(app.key_chord_matcher.is_pending());
            assert_eq!(paging_top_row(&mut app, &mut tui)?.0, before);
            press(&mut app, &mut tui, &mut server, KeyCode::Char('g').into()).await?;
            assert_eq!(paging_top_row(&mut app, &mut tui)?.0, 0);
            assert!(!app.key_chord_matcher.is_pending());
            assert_eq!(app.chat_widget.capture_thread_input_state(), draft);
            assert_eq!(paging_draft_cursor(&app), cursor);
            press(&mut app, &mut tui, &mut server, bottom).await?;
            assert!(app.transcript_view.is_following());
            assert_eq!(app.chat_widget.capture_thread_input_state(), draft);
            assert_eq!(paging_draft_cursor(&app), cursor);
            server.shutdown().await?;
        }
    }
    Ok(())
}

#[tokio::test]
async fn vim_history_top_waits_for_unloaded_pages_and_bottom_cancels_loading() -> Result<()> {
    let (mut app, mut tui, mut server, _events) = vim_paging_app().await?;
    app.scrollback_has_older_history = true;
    app.transcript_view.history = TranscriptHistoryState::Partial;
    let before = paging_top_row(&mut app, &mut tui)?.0;
    for _ in 0..2 {
        press(&mut app, &mut tui, &mut server, KeyCode::Char('g').into()).await?;
    }
    assert_eq!(
        app.transcript_view.history,
        TranscriptHistoryState::LoadingBeginning
    );
    assert_eq!(paging_top_row(&mut app, &mut tui)?.0, before);
    press(&mut app, &mut tui, &mut server, KeyCode::Char('G').into()).await?;
    assert!(app.transcript_view.is_following());
    assert_ne!(
        app.transcript_view.history,
        TranscriptHistoryState::LoadingBeginning
    );
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn vim_history_extents_leave_editing_and_pending_commands_with_the_draft() -> Result<()> {
    for command in ["i", "v", "d", "c", "2", "/", "f"] {
        let (mut app, mut tui, mut server, _events) = vim_paging_app().await?;
        app.chat_widget.insert_str("one\ntwo\nthree");
        let before = paging_top_row(&mut app, &mut tui)?.0;
        for key in command.chars().chain("G".chars()) {
            press(&mut app, &mut tui, &mut server, KeyCode::Char(key).into()).await?;
        }
        assert_eq!(paging_top_row(&mut app, &mut tui)?.0, before, "{command}G");
        server.shutdown().await?;
    }
    for (command, expected) in [("dgg", ""), ("kk0cG", "")] {
        let (mut app, mut tui, mut server, _events) = vim_paging_app().await?;
        app.chat_widget.insert_str("one\ntwo\nthree");
        let before = paging_top_row(&mut app, &mut tui)?.0;
        for key in command.chars() {
            press(&mut app, &mut tui, &mut server, KeyCode::Char(key).into()).await?;
        }
        assert_eq!(paging_top_row(&mut app, &mut tui)?.0, before, "{command}");
        assert_eq!(
            app.chat_widget.composer_text_with_pending(),
            expected,
            "{command}"
        );
        server.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn vim_history_navigation_keeps_hjkl_in_the_draft() -> Result<()> {
    let (mut app, mut tui, mut server, _events) = vim_paging_app().await?;
    app.chat_widget.insert_str("one\ntwo\nthree");
    let before = paging_top_row(&mut app, &mut tui)?.0;
    let cursor = paging_draft_cursor(&app);
    for key in "hklj".chars() {
        press(&mut app, &mut tui, &mut server, KeyCode::Char(key).into()).await?;
        assert_eq!(paging_top_row(&mut app, &mut tui)?.0, before);
    }
    assert_ne!(paging_draft_cursor(&app), cursor);
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "one\ntwo\nthree"
    );
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn vim_history_extents_reuse_configured_chords_and_respect_other_bindings() -> Result<()> {
    let (mut app, mut tui, mut server, _events) = vim_paging_app().await?;
    let config = toml::from_str(
        "[vim_normal]\njump_top = 'z z'\njump_bottom = 'z b'\n[global]\ntoggle_raw_output = 'G'\n",
    )?;
    app.keymap = RuntimeKeymap::from_config(&config).expect("valid history jump bindings");
    app.chat_widget
        .apply_keymap_update(config.clone(), &app.keymap);
    app.local_settings.tui.keymap = config;
    app.chat_widget.insert_str("draft");
    let cursor = paging_draft_cursor(&app);
    let before = paging_top_row(&mut app, &mut tui)?.0;
    press(&mut app, &mut tui, &mut server, KeyCode::Char('G').into()).await?;
    assert!(app.chat_widget.raw_output_mode());
    assert_eq!(paging_top_row(&mut app, &mut tui)?.0, before);
    for key in "zz".chars() {
        press(&mut app, &mut tui, &mut server, KeyCode::Char(key).into()).await?;
    }
    assert_eq!(paging_top_row(&mut app, &mut tui)?.0, 0);
    for key in "zb".chars() {
        press(&mut app, &mut tui, &mut server, KeyCode::Char(key).into()).await?;
    }
    assert!(app.transcript_view.is_following());
    assert_eq!(app.chat_widget.composer_text_with_pending(), "draft");
    assert_eq!(paging_draft_cursor(&app), cursor);
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn vim_history_gg_cancellation_timeout_and_mode_changes_do_not_leak_prefixes() -> Result<()> {
    for boundary in ["escape", "timeout", "mode", "focus", "wrong"] {
        let (mut app, mut tui, mut server, _events) = vim_paging_app().await?;
        app.chat_widget.insert_str("draft");
        let before = paging_top_row(&mut app, &mut tui)?.0;
        press(&mut app, &mut tui, &mut server, KeyCode::Char('g').into()).await?;
        assert!(app.key_chord_matcher.is_pending());
        match boundary {
            "escape" => press(&mut app, &mut tui, &mut server, KeyCode::Esc.into()).await?,
            "timeout" => {
                tokio::time::pause();
                tokio::time::advance(crate::keymap::KEY_CHORD_TIMEOUT).await;
                app.handle_tui_event(&mut tui, &mut server, TuiEvent::Draw)
                    .await?;
            }
            "mode" => {
                app.chat_widget.toggle_vim_mode_and_notify();
            }
            "wrong" => {
                press(&mut app, &mut tui, &mut server, KeyCode::Char('h').into()).await?;
            }
            "focus" => {
                app.handle_tui_event(&mut tui, &mut server, TuiEvent::FocusLost)
                    .await?;
            }
            _ => unreachable!(),
        }
        press(&mut app, &mut tui, &mut server, KeyCode::Char('g').into()).await?;
        assert_eq!(paging_top_row(&mut app, &mut tui)?.0, before, "{boundary}");
        assert_eq!(app.key_chord_matcher.is_pending(), boundary != "mode");
        server.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn vim_normal_empty_draft_ctrl_d_pages_instead_of_quitting_even_after_selection() -> Result<()>
{
    for selected in [false, true] {
        let (mut app, mut tui, mut server, mut events) = vim_paging_app().await?;
        if selected {
            press(&mut app, &mut tui, &mut server, ctrl(' ')).await?;
            assert!(app.transcript_view.has_active_interaction());
        }
        let (before, height) = paging_top_row(&mut app, &mut tui)?;
        assert!(matches!(
            app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(ctrl('d')))
                .await?,
            super::AppRunControl::Continue
        ));
        assert_eq!(
            paging_top_row(&mut app, &mut tui)?.0 - before,
            (height + 1) / 2
        );
        assert!(app.chat_widget.composer_is_empty());
        while let Ok(event) = events.try_recv() {
            assert!(!matches!(event, super::AppEvent::Exit(_)));
        }
        server.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn vim_paging_preserves_insert_and_non_vim_editor_shortcuts() -> Result<()> {
    for vim_insert in [true, false] {
        let (mut app, mut tui, mut server, mut events) = vim_paging_app().await?;
        if vim_insert {
            press(&mut app, &mut tui, &mut server, KeyCode::Char('i').into()).await?;
        } else {
            app.chat_widget.toggle_vim_mode_and_notify();
        }
        app.chat_widget.insert_str("draft");
        let before = paging_top_row(&mut app, &mut tui)?.0;
        press(&mut app, &mut tui, &mut server, ctrl('b')).await?;
        app.chat_widget.insert_str("X");
        assert_eq!(app.chat_widget.composer_text_with_pending(), "drafXt");
        press(&mut app, &mut tui, &mut server, ctrl('f')).await?;
        app.chat_widget.insert_str("Y");
        assert_eq!(app.chat_widget.composer_text_with_pending(), "drafXtY");
        press(&mut app, &mut tui, &mut server, ctrl('u')).await?;
        assert!(app.chat_widget.composer_is_empty());
        assert_eq!(paging_top_row(&mut app, &mut tui)?.0, before);
        press(&mut app, &mut tui, &mut server, ctrl('d')).await?;
        assert!(
            std::iter::from_fn(|| events.try_recv().ok())
                .any(|event| matches!(event, super::AppEvent::Exit(_)))
        );
        server.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn vim_paging_respects_custom_bindings_and_release_events() -> Result<()> {
    let (mut app, mut tui, mut server, _events) = vim_paging_app().await?;
    let config = toml::from_str(
        "[global]\ntoggle_raw_output = 'ctrl-u'\n[pager]\npage_up = 'ctrl-y'\npage_down = 'z'\n",
    )?;
    app.keymap = RuntimeKeymap::from_config(&config).expect("valid paging bindings");
    app.chat_widget
        .apply_keymap_update(config.clone(), &app.keymap);
    app.local_settings.tui.keymap = config;
    let before = paging_top_row(&mut app, &mut tui)?.0;
    press(&mut app, &mut tui, &mut server, ctrl('u')).await?;
    assert!(app.chat_widget.raw_output_mode());
    assert_eq!(paging_top_row(&mut app, &mut tui)?.0, before);
    let mut release = ctrl('d');
    release.kind = crossterm::event::KeyEventKind::Release;
    press(&mut app, &mut tui, &mut server, release).await?;
    assert_eq!(paging_top_row(&mut app, &mut tui)?.0, before);
    let (_, height) = paging_top_row(&mut app, &mut tui)?;
    press(&mut app, &mut tui, &mut server, ctrl('y')).await?;
    assert_eq!(
        paging_top_row(&mut app, &mut tui)?.0,
        before - (height - 1).max(1)
    );
    let before = paging_top_row(&mut app, &mut tui)?.0;
    let mut repeat = ctrl('d');
    repeat.kind = crossterm::event::KeyEventKind::Repeat;
    press(&mut app, &mut tui, &mut server, repeat).await?;
    assert_eq!(
        paging_top_row(&mut app, &mut tui)?.0 - before,
        (height + 1) / 2
    );
    let before = paging_top_row(&mut app, &mut tui)?.0;
    press(&mut app, &mut tui, &mut server, KeyCode::Char('z').into()).await?;
    assert_eq!(
        paging_top_row(&mut app, &mut tui)?.0 - before,
        (height - 1).max(1)
    );
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn vim_paging_leaves_visual_pending_commands_and_search_with_their_editor() -> Result<()> {
    for command in ['v', 'd', '/', '2'] {
        let (mut app, mut tui, mut server, _events) = vim_paging_app().await?;
        app.chat_widget.insert_str("draft text");
        press(
            &mut app,
            &mut tui,
            &mut server,
            KeyCode::Char(command).into(),
        )
        .await?;
        let before = paging_top_row(&mut app, &mut tui)?.0;
        press(&mut app, &mut tui, &mut server, ctrl('b')).await?;
        assert_eq!(paging_top_row(&mut app, &mut tui)?.0, before, "{command}");
        server.shutdown().await?;
    }
    let (mut app, mut tui, mut server, _events) = vim_paging_app().await?;
    app.transcript_view.begin_search();
    press(&mut app, &mut tui, &mut server, ctrl('d')).await?;
    assert!(app.transcript_view.is_search_editing());
    assert!(app.chat_widget.composer_is_empty());
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn vim_paging_does_not_take_default_space_or_shift_space_from_the_draft() -> Result<()> {
    let (mut app, mut tui, mut server, _events) = vim_paging_app().await?;
    app.chat_widget.insert_str("draft text");
    for modifiers in [KeyModifiers::NONE, KeyModifiers::SHIFT] {
        let before = paging_top_row(&mut app, &mut tui)?.0;
        press(
            &mut app,
            &mut tui,
            &mut server,
            KeyEvent::new(KeyCode::Char(' '), modifiers),
        )
        .await?;
        assert_eq!(paging_top_row(&mut app, &mut tui)?.0, before);
    }
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn vim_paging_leaves_modal_navigation_with_the_modal() -> Result<()> {
    let (mut app, mut tui, mut server, _events) = vim_paging_app().await?;
    app.chat_widget.show_selection_view(SelectionViewParams {
        title: Some("Select an item".to_owned()),
        items: vec![SelectionItem {
            name: "First".to_owned(),
            ..Default::default()
        }],
        ..Default::default()
    });
    let before = paging_top_row(&mut app, &mut tui)?.0;
    press(&mut app, &mut tui, &mut server, ctrl('b')).await?;
    assert!(app.chat_widget.has_active_view());
    assert_eq!(paging_top_row(&mut app, &mut tui)?.0, before);
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn vim_buffer_jumps_route_default_chords_in_normal_and_operator_contexts() -> Result<()> {
    for (input, command, expected) in [
        ("one\ntwo\nthree", "gg", "!one\ntwo\nthree"),
        ("one\ntwo\nthree", "dgg", "!"),
        ("ag bg", "0fg", "a!g bg"),
        ("ag bg", "0dfg", "! bg"),
    ] {
        let (mut app, mut tui, mut app_server) = chord_app().await?;
        app.chat_widget.toggle_vim_mode_and_notify();
        app.chat_widget.insert_str(input);

        for (index, key) in command.chars().enumerate() {
            press(
                &mut app,
                &mut tui,
                &mut app_server,
                KeyCode::Char(key).into(),
            )
            .await?;
            if key == 'g' && index + 1 < command.len() {
                assert!(app.key_chord_matcher.is_pending());
            }
        }

        assert!(!app.key_chord_matcher.is_pending());
        app.chat_widget.insert_str("!");
        assert_eq!(app.chat_widget.composer_text_with_pending(), expected);
    }
    Ok(())
}

#[tokio::test]
async fn global_chord_keeps_hints_and_completes_before_deadline() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;

    press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
    assert!(app.key_chord_matcher.is_pending());
    assert!(app.overlay.is_none());
    tokio::time::pause();
    tokio::time::advance(std::time::Duration::from_millis(/*millis*/ 500)).await;
    app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Draw)
        .await?;
    assert!(app.key_chord_matcher.is_pending());
    insta::assert_snapshot!(
        render_bottom_popup(&app.chat_widget, /*width*/ 80)
            .replace(&test_path_display("/tmp/project"), "/tmp/project"),
        @"
    › Ask Codex to do anything

      ⌃x then · ⌃t open transcript · ⌃u interrupt turn · esc cancel
    "
    );

    press(&mut app, &mut tui, &mut app_server, ctrl('t')).await?;
    assert!(!app.key_chord_matcher.is_pending());
    assert!(app.overlay.is_some());
    Ok(())
}

#[tokio::test]
async fn completed_global_chords_toggle_output_and_request_external_editor() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;
    let config = toml::from_str(
        "[global]\ntoggle_raw_output = [\"ctrl-x r\", \"pageup\"]\nopen_external_editor = [\"ctrl-x e\"]",
    )?;
    app.keymap = RuntimeKeymap::from_config(&config).expect("valid global chords");
    app.chat_widget.apply_keymap_update(config, &app.keymap);

    for key in [ctrl('x'), KeyCode::Char('r').into()] {
        press(&mut app, &mut tui, &mut app_server, key).await?;
    }
    assert!(app.chat_widget.raw_output_mode());
    tui.set_owned_screen(/*owned*/ true)?;
    press(&mut app, &mut tui, &mut app_server, KeyCode::PageUp.into()).await?;
    assert!(!app.chat_widget.raw_output_mode());
    let config = toml::from_str(
        "[composer]\nsubmit = 'pageup'\n[editor]\nmove_line_start = 'pagedown'\n[global]\nopen_external_editor = 'ctrl-x e'",
    )?;
    app.keymap = RuntimeKeymap::from_config(&config).expect("valid composer and editor bindings");
    app.chat_widget.apply_keymap_update(config, &app.keymap);
    app.chat_widget.apply_external_edit("draft".to_string());
    assert!(!app.handle_owned_transcript_event(
        &mut tui,
        &mut app_server,
        &TuiEvent::Key(KeyCode::PageUp.into()),
    )?);
    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyCode::PageDown.into(),
    )
    .await?;
    app.chat_widget.insert_str("X");
    assert_eq!(app.chat_widget.composer_text_with_pending(), "Xdraft");
    tui.set_owned_screen(/*owned*/ false)?;

    for key in [ctrl('x'), KeyCode::Char('e').into()] {
        press(&mut app, &mut tui, &mut app_server, key).await?;
    }
    assert_eq!(
        app.chat_widget.external_editor_state(),
        super::ExternalEditorState::Requested
    );
    Ok(())
}

#[tokio::test]
async fn wrong_second_stroke_passes_through_but_escape_is_consumed() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;

    press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
    let wrong_second = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    assert_eq!(
        app.route_key_chord_event(&mut tui, wrong_second),
        Some(wrong_second)
    );

    press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
    let escape = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(app.route_key_chord_event(&mut tui, escape), None);
    assert!(!app.key_chord_matcher.is_pending());
    assert!(!app.backtrack.primed);
    Ok(())
}

#[tokio::test]
async fn config_editor_chord_preserves_physical_completion_key_until_release() -> Result<()> {
    let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
    let mut config = TuiKeymap::default();
    config.list.accept = Some(KeybindingsSpec::One(KeybindingSpec(
        "enter enter".to_string(),
    )));
    let runtime =
        RuntimeKeymap::from_config(&config).map_err(|error| color_eyre::eyre::eyre!(error))?;
    app.chat_widget.apply_keymap_update(config, &runtime);
    app.keymap = runtime;
    app.reconnect.offline = true;
    let mut app_server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.chat_widget.open_config_panel();
    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyCode::Char('/').into(),
    )
    .await?;
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Paste("terminal title".to_string()),
    )
    .await?;

    for _ in 0..2 {
        press(&mut app, &mut tui, &mut app_server, KeyCode::Enter.into()).await?;
    }
    let handoff = std::iter::from_fn(|| events.try_recv().ok())
        .find_map(|event| match event {
            crate::app_event::AppEvent::OpenConfigEditor(handoff) => Some(handoff),
            _ => None,
        })
        .expect("config editor handoff");
    assert_eq!(
        handoff.activation_key,
        crate::key_hint::plain(KeyCode::Enter)
    );
    assert!(handoff.active_at().is_some());

    app.chat_widget.open_config_editor(
        handoff,
        crate::config_panel::ResumeCurrentAvailability::Available,
    );
    assert!(
        render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("Configure Terminal Title")
    );
    let repeat = KeyEvent::new_with_kind(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Repeat);
    app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Key(repeat))
        .await?;
    let release =
        KeyEvent::new_with_kind(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Release);
    app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Key(release))
        .await?;
    for _ in 0..2 {
        press(&mut app, &mut tui, &mut app_server, KeyCode::Enter.into()).await?;
    }
    assert!(
        std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, crate::app_event::AppEvent::TerminalTitleSetup { .. }))
    );
    Ok(())
}

#[tokio::test]
async fn queued_config_handoff_observes_a_consumed_chord_prefix() -> Result<()> {
    let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
    let mut config = TuiKeymap::default();
    config.list.move_down = Some(KeybindingsSpec::One(KeybindingSpec("ctrl-x j".to_string())));
    let runtime =
        RuntimeKeymap::from_config(&config).map_err(|error| color_eyre::eyre::eyre!(error))?;
    app.chat_widget.apply_keymap_update(config, &runtime);
    app.keymap = runtime;
    let mut app_server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.chat_widget.open_config_panel();
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Paste("terminal title".to_string()),
    )
    .await?;

    press(&mut app, &mut tui, &mut app_server, KeyCode::Enter.into()).await?;
    let handoff = std::iter::from_fn(|| events.try_recv().ok())
        .find_map(|event| match event {
            crate::app_event::AppEvent::OpenConfigEditor(handoff) => Some(handoff),
            _ => None,
        })
        .expect("config editor handoff");
    assert!(handoff.active_at().is_some());

    press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
    assert_eq!(handoff.active_at(), None);
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn legacy_terminal_recovers_vim_escape_before_normal_commands() -> Result<()> {
    for (key, expected) in [
        (KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT), "ab"),
        (
            KeyEvent::new(KeyCode::Char('D'), KeyModifiers::ALT | KeyModifiers::SHIFT),
            "ab",
        ),
        (KeyEvent::new(KeyCode::Char('h'), KeyModifiers::ALT), "abc"),
    ] {
        let (mut app, mut tui, mut app_server) = chord_app().await?;
        app.chat_widget.toggle_vim_mode_and_notify();
        app.chat_widget.insert_str("abc");
        press(
            &mut app,
            &mut tui,
            &mut app_server,
            KeyCode::Char('i').into(),
        )
        .await?;

        press(&mut app, &mut tui, &mut app_server, key).await?;

        assert_eq!(app.chat_widget.composer_text_with_pending(), expected);
        assert!(
            !app.chat_widget
                .should_handle_vim_insert_escape(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        );
    }
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn legacy_terminal_preserves_default_and_configured_editor_alt_bindings() -> Result<()> {
    for (key, expected) in [
        ('b', "!abc def"),
        ('f', "abc! def"),
        ('d', "! def"),
        ('k', "!"),
    ] {
        let (mut app, mut tui, mut app_server) = chord_app().await?;
        if key == 'k' {
            let mut config = TuiKeymap::default();
            config.editor.kill_line_end =
                Some(KeybindingsSpec::One(KeybindingSpec("alt-k".to_string())));
            let runtime = RuntimeKeymap::from_config(&config)
                .map_err(|error| color_eyre::eyre::eyre!(error))?;
            app.chat_widget.apply_keymap_update(config, &runtime);
            app.keymap = runtime;
        }
        app.chat_widget.toggle_vim_mode_and_notify();
        app.chat_widget.insert_str("abc def");
        let insert = KeyCode::Char('i').into();
        press(&mut app, &mut tui, &mut app_server, insert).await?;
        press(&mut app, &mut tui, &mut app_server, ctrl('a')).await?;
        press(
            &mut app,
            &mut tui,
            &mut app_server,
            KeyEvent::new(KeyCode::Char(key), KeyModifiers::ALT),
        )
        .await?;

        assert!(
            app.chat_widget
                .should_handle_vim_insert_escape(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        );
        app.chat_widget.insert_str("!");
        assert_eq!(app.chat_widget.composer_text_with_pending(), expected);
    }
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn legacy_terminal_preserves_active_global_alt_shortcuts() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;
    app.chat_widget.toggle_vim_mode_and_notify();
    app.chat_widget.insert_str("abc");
    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyCode::Char('i').into(),
    )
    .await?;

    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyEvent::new(KeyCode::Char('r'), KeyModifiers::ALT),
    )
    .await?;

    assert!(app.chat_widget.raw_output_mode());
    assert!(
        app.chat_widget
            .should_handle_vim_insert_escape(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
    );
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn owned_transcript_alt_jumps_preserve_legacy_vim_insert_and_draft() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;
    app.enhanced_keys_supported = false;
    tui.set_owned_screen(/*owned*/ true)?;
    app.transcript_cells = vec![std::sync::Arc::new(
        crate::history_cell::PlainHistoryCell::new(vec!["Earlier transcript".into()]),
    )];
    app.chat_widget.toggle_vim_mode_and_notify();
    app.chat_widget.insert_str("draft");
    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyCode::Char('i').into(),
    )
    .await?;

    let area = ratatui::layout::Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 4,
    );
    let mut buffer = ratatui::buffer::Buffer::empty(area);
    app.transcript_view
        .render(area, &mut buffer, &app.transcript_cells);

    for modifiers in [KeyModifiers::ALT, KeyModifiers::ALT | KeyModifiers::SHIFT] {
        app.transcript_view.history = TranscriptHistoryState::Partial;
        for (character, following, history) in [
            ('<', false, TranscriptHistoryState::LoadingBeginning),
            ('>', true, TranscriptHistoryState::LoadingOlder),
        ] {
            press(
                &mut app,
                &mut tui,
                &mut app_server,
                KeyEvent::new(KeyCode::Char(character), modifiers),
            )
            .await?;
            assert_eq!(
                (
                    app.transcript_view.is_following(),
                    app.transcript_view.history,
                    app.chat_widget.composer_text_with_pending(),
                    app.chat_widget
                        .should_handle_vim_insert_escape(KeyEvent::new(
                            KeyCode::Esc,
                            KeyModifiers::NONE,
                        )),
                ),
                (following, history, "draft".to_string(), true)
            );
        }
    }
    tui.set_owned_screen(/*owned*/ false)?;
    app_server.shutdown().await?;
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn legacy_terminal_preserves_configured_composer_alt_shortcuts() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;
    let mut config = TuiKeymap::default();
    config.composer.history_search_previous =
        Some(KeybindingsSpec::One(KeybindingSpec("alt-q".to_string())));
    let runtime =
        RuntimeKeymap::from_config(&config).map_err(|error| color_eyre::eyre::eyre!(error))?;
    app.chat_widget.apply_keymap_update(config, &runtime);
    app.keymap = runtime;
    app.chat_widget.toggle_vim_mode_and_notify();
    app.chat_widget.insert_str("abc");
    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyCode::Char('i').into(),
    )
    .await?;

    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyEvent::new(KeyCode::Char('q'), KeyModifiers::ALT),
    )
    .await?;

    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("reverse-i-search:"));
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn legacy_terminal_preserves_active_alt_chords() -> Result<()> {
    for (binding, prefix, completion) in [
        (
            "alt-q r",
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::ALT),
            KeyCode::Char('r').into(),
        ),
        (
            "ctrl-x alt-h",
            ctrl('x'),
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::ALT),
        ),
    ] {
        let (mut app, mut tui, mut app_server) = chord_app().await?;
        let mut config = TuiKeymap::default();
        config.global.toggle_raw_output =
            Some(KeybindingsSpec::One(KeybindingSpec(binding.to_string())));
        let runtime =
            RuntimeKeymap::from_config(&config).map_err(|error| color_eyre::eyre::eyre!(error))?;
        app.chat_widget.apply_keymap_update(config, &runtime);
        app.keymap = runtime;
        app.chat_widget.toggle_vim_mode_and_notify();
        app.chat_widget.insert_str("abc");
        press(
            &mut app,
            &mut tui,
            &mut app_server,
            KeyCode::Char('i').into(),
        )
        .await?;

        press(&mut app, &mut tui, &mut app_server, prefix).await?;
        assert!(app.key_chord_matcher.is_pending());
        press(&mut app, &mut tui, &mut app_server, completion).await?;

        assert!(app.chat_widget.raw_output_mode());
        assert!(!app.key_chord_matcher.is_pending());
    }
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn legacy_terminal_preserves_image_paste_without_reading_clipboard() -> Result<()> {
    for mode in ['i', 'R'] {
        let (mut app, mut tui, mut app_server) = chord_app().await?;
        app.chat_widget.toggle_vim_mode_and_notify();
        app.chat_widget.insert_str("abc");
        press(
            &mut app,
            &mut tui,
            &mut app_server,
            KeyCode::Char(mode).into(),
        )
        .await?;
        for key in [
            KeyEvent::new(KeyCode::Char('v'), KeyModifiers::ALT),
            KeyEvent::new(KeyCode::Char('V'), KeyModifiers::ALT | KeyModifiers::SHIFT),
        ] {
            assert!(!app.should_recover_vim_insert_escape(key));
        }
        assert!(app.should_recover_vim_insert_escape(KeyEvent::new(
            KeyCode::Char('x'),
            KeyModifiers::ALT,
        )));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn legacy_terminal_preserves_agent_shortcuts_without_editor_word_bindings() -> Result<()> {
    for mode in ['i', 'R'] {
        let (mut app, mut tui, mut app_server) = chord_app().await?;
        let mut config = TuiKeymap::default();
        config.editor.move_word_left = Some(KeybindingsSpec::Many(vec![]));
        config.editor.move_word_right = Some(KeybindingsSpec::Many(vec![]));
        let runtime =
            RuntimeKeymap::from_config(&config).map_err(|error| color_eyre::eyre::eyre!(error))?;
        app.chat_widget.apply_keymap_update(config, &runtime);
        app.keymap = runtime;
        app.chat_widget.toggle_vim_mode_and_notify();
        press(
            &mut app,
            &mut tui,
            &mut app_server,
            KeyCode::Char(mode).into(),
        )
        .await?;

        for key in ['b', 'f'] {
            let event = KeyEvent::new(KeyCode::Char(key), KeyModifiers::ALT);
            assert!(!app.should_recover_vim_insert_escape(event));
        }
        app.chat_widget.insert_str("abc");
        for key in ['b', 'f'] {
            let event = KeyEvent::new(KeyCode::Char(key), KeyModifiers::ALT);
            assert!(app.should_recover_vim_insert_escape(event));
        }
    }
    Ok(())
}

#[tokio::test]
async fn vim_escape_recovery_preserves_enhanced_terminals_and_altgr() -> Result<()> {
    for (enhanced_keys_supported, modifiers) in [
        (true, KeyModifiers::ALT),
        (false, KeyModifiers::ALT | KeyModifiers::CONTROL),
    ] {
        let (mut app, mut tui, mut app_server) = chord_app().await?;
        app.enhanced_keys_supported = enhanced_keys_supported;
        app.chat_widget.toggle_vim_mode_and_notify();
        app.chat_widget.insert_str("abc");
        press(
            &mut app,
            &mut tui,
            &mut app_server,
            KeyCode::Char('i').into(),
        )
        .await?;

        press(
            &mut app,
            &mut tui,
            &mut app_server,
            KeyEvent::new(KeyCode::Char('x'), modifiers),
        )
        .await?;

        assert!(
            app.chat_widget
                .should_handle_vim_insert_escape(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        );
    }
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn recovered_vim_escape_cancels_a_pending_key_chord_first() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;
    app.chat_widget.toggle_vim_mode_and_notify();
    app.chat_widget.insert_str("abc");
    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyCode::Char('i').into(),
    )
    .await?;
    press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
    assert!(app.key_chord_matcher.is_pending());

    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT),
    )
    .await?;

    assert!(!app.key_chord_matcher.is_pending());
    press(&mut app, &mut tui, &mut app_server, KeyCode::Left.into()).await?;
    assert_eq!(app.chat_widget.composer_text_with_pending(), "abcx");
    assert!(
        app.chat_widget
            .should_handle_vim_insert_escape(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
    );
    Ok(())
}

#[tokio::test]
async fn physical_dispatch_band_events_are_dropped() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;
    let (code, modifiers) = app
        .keymap
        .app
        .open_transcript
        .last()
        .expect("configured chord appends a dispatch token")
        .parts();

    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyEvent::new(code, modifiers),
    )
    .await?;

    assert!(app.overlay.is_none());
    Ok(())
}

#[tokio::test]
async fn physical_chords_route_list_and_mixed_request_input_modals() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;
    app.chat_widget.show_selection_view(SelectionViewParams {
        view_id: Some("list"),
        items: ["First", "Second"]
            .into_iter()
            .map(|name| SelectionItem {
                name: name.to_string(),
                dismiss_on_select: true,
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    });
    assert_eq!(
        app.chat_widget.selected_index_for_present_view("list"),
        Some(0)
    );

    press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
    assert!(app.key_chord_matcher.is_pending());
    assert_eq!(
        app.chat_widget.selected_index_for_present_view("list"),
        Some(0)
    );
    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyCode::Char('j').into(),
    )
    .await?;
    assert_eq!(
        app.chat_widget.selected_index_for_present_view("list"),
        Some(1)
    );

    press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
    press(&mut app, &mut tui, &mut app_server, KeyCode::Enter.into()).await?;
    assert_eq!(
        app.chat_widget.selected_index_for_present_view("list"),
        None
    );

    app.chat_widget
        .handle_request_user_input_now(ToolRequestUserInputParams {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: "call-1".to_string(),
            questions: vec![ToolRequestUserInputQuestion {
                id: "choice".to_string(),
                header: "Pick one".to_string(),
                question: "Choose an option.".to_string(),
                is_other: false,
                is_secret: false,
                options: Some(
                    ["First", "Second"]
                        .into_iter()
                        .map(|label| ToolRequestUserInputOption {
                            label: label.to_string(),
                            description: label.to_string(),
                        })
                        .collect(),
                ),
            }],
            is_blocking: true,
            auto_resolution_ms: None,
        });
    let contexts = app.chat_widget.keymap_contexts();
    assert!(contexts.contains(KeymapContext::Chat));
    assert!(contexts.contains(KeymapContext::List));
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("› 1. First"));

    press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
    assert!(app.key_chord_matcher.is_pending());
    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyCode::Char('j').into(),
    )
    .await?;
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("› 2. Second"));

    press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
    press(&mut app, &mut tui, &mut app_server, ctrl('u')).await?;
    assert!(app.chat_widget.can_launch_external_editor());
    Ok(())
}

#[tokio::test]
async fn dashboard_chord_hint_survives_refresh_and_clears_on_cancel() -> Result<()> {
    let mut app = make_test_app().await;
    app.agents_overview
        .view_state
        .lock()
        .unwrap()
        .server_version_notice = Some("Older server".into());
    app.keymap =
        RuntimeKeymap::from_config(&toml::from_str("[agents]\nnew_task = [\"ctrl-x n\"]")?)
            .unwrap();
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    let before = render_bottom_popup(&app.chat_widget, /*width*/ 80);
    assert_eq!(app.route_key_chord_event(&mut tui, ctrl('x')), None);
    let _ = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    insta::assert_snapshot!(
        render_bottom_popup(&app.chat_widget, /*width*/ 80).lines().last().unwrap(),
        @"  ⌃x then  n new task  esc cancel"
    );
    assert_eq!(
        app.route_key_chord_event(&mut tui, KeyCode::Esc.into()),
        None
    );
    assert_eq!(render_bottom_popup(&app.chat_widget, /*width*/ 80), before);
    Ok(())
}

#[tokio::test]
async fn command_center_chords_do_not_capture_search_text() -> Result<()> {
    let mut app = make_test_app().await;
    app.keymap = RuntimeKeymap::from_config(&toml::from_str(
        "[agents]\nnew_task = 'n n'\n[ list ]\naccept = 'ctrl-x enter'",
    )?)
    .unwrap();
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    app.chat_widget.handle_key_event(KeyCode::Char('/').into());
    for key in "new".chars() {
        let event = KeyCode::Char(key).into();
        assert_eq!(app.route_key_chord_event(&mut tui, event), Some(event));
        app.chat_widget.handle_key_event(event);
    }
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("Search › new"));
    assert_eq!(app.route_key_chord_event(&mut tui, ctrl('x')), None);
    assert!(app.key_chord_matcher.is_pending());
    app.route_key_chord_event(&mut tui, KeyCode::Esc.into());
    app.chat_widget.handle_key_event(KeyCode::Esc.into());
    assert_eq!(
        app.route_key_chord_event(&mut tui, KeyCode::Char('n').into()),
        None
    );
    assert!(app.key_chord_matcher.is_pending());
    Ok(())
}

#[tokio::test]
async fn transcript_fixed_keys_take_precedence_over_pager_chord_prefixes() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;
    app.keymap = RuntimeKeymap::from_config(&serde_json::from_value(serde_json::json!({
        "global": {"copy": ["ctrl-x ctrl-u"]},
        "pager": {
            "scroll_up": ["ctrl-space x", "ctrl-home x", "ctrl-end x"],
            "close_transcript": ["ctrl-x ctrl-u"]
        }
    }))?)
    .expect("valid pager chords");
    app.chat_widget.insert_str("draft");
    for owned in [true, false] {
        tui.set_owned_screen(owned)?;
        app.open_transcript_overlay(&mut tui);
        for code in [KeyCode::Home, KeyCode::End, KeyCode::Char(' ')] {
            let key = KeyEvent::new(code, KeyModifiers::CONTROL);
            assert_eq!(app.route_key_chord_event(&mut tui, key), Some(key));
            assert!(!app.key_chord_matcher.is_pending());
        }
        press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
        press(&mut app, &mut tui, &mut app_server, ctrl('u')).await?;
        assert!(app.overlay.is_none());
        assert!(!app.transcript_view.is_detailed());
        assert_eq!(app.chat_widget.composer_text_with_pending(), "draft");
    }
    app.transcript_cells = vec![std::sync::Arc::new(
        crate::history_cell::PlainHistoryCell::new(
            (0..50).map(|_| "transcript row".into()).collect(),
        ),
    )];
    tui.set_owned_screen(/*owned*/ true)?;
    let home = KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL);
    for (binding, keys) in [
        ("ctrl-home", vec![home]),
        ("ctrl-home ctrl-u", vec![home, ctrl('u')]),
        ("ctrl-x ctrl-home", vec![ctrl('x'), home]),
    ] {
        app.keymap = RuntimeKeymap::from_config(&toml::from_str(&format!(
            "[pager]\nclose_transcript = '{binding}'\n[global]\nopen_external_editor = '{binding}'"
        ))?)
        .expect("valid close binding");
        app.open_transcript_overlay(&mut tui);
        app.render_owned_transcript(
            &mut tui,
            ratatui::layout::Size::new(/*width*/ 80, /*height*/ 12),
        )?;
        app.transcript_view
            .scroll(&app.transcript_cells, /*rows*/ -10);
        assert!(app.transcript_view.can_return_to_latest());
        for key in &keys {
            press(&mut app, &mut tui, &mut app_server, *key).await?;
        }
        assert!(!app.transcript_view.is_detailed());
        assert_eq!(app.chat_widget.composer_text_with_pending(), "draft");
        for key in keys {
            press(&mut app, &mut tui, &mut app_server, key).await?;
        }
        assert_eq!(
            app.chat_widget.external_editor_state(),
            crate::chatwidget::ExternalEditorState::Requested
        );
        app.reset_external_editor_state(&mut tui);
    }
    for voice in ["f9", "f9 v"] {
        app.keymap = RuntimeKeymap::from_config(&toml::from_str(&format!(
            "[chat]\ntoggle_voice = '{voice}'\n[pager]\nclose_transcript = 'f9'"
        ))?)
        .unwrap();
        app.open_transcript_overlay(&mut tui);
        press(&mut app, &mut tui, &mut app_server, KeyCode::F(9).into()).await?;
        assert!(!app.transcript_view.is_detailed());
        assert!(!app.key_chord_matcher.is_pending());
    }
    tui.set_owned_screen(/*owned*/ false)?;
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn selection_after_live_commit_uses_the_refreshed_frame() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;
    let area = ratatui::layout::Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 12,
    );
    let mut overlay =
        crate::pager_overlay::TranscriptOverlay::new(Vec::new(), app.keymap.pager.clone());
    overlay.sync_live_tail(
        /*width*/ 40,
        /*key*/ None,
        |_| Some(vec!["committed output".into()]),
    );
    overlay.render(area, &mut ratatui::buffer::Buffer::empty(area));
    overlay.insert_cell(std::sync::Arc::new(
        crate::history_cell::PlainHistoryCell::new(vec!["committed output".into()]),
    ));
    app.overlay = Some(crate::pager_overlay::Overlay::Transcript(overlay));
    // The active cell has completed, but no Draw has reached the overlay yet.
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL)),
    )
    .await?;
    let Some(crate::pager_overlay::Overlay::Transcript(overlay)) = &mut app.overlay else {
        panic!("overlay closed")
    };
    assert!(overlay.has_active_interaction());
    let mut buffer = ratatui::buffer::Buffer::empty(area);
    overlay.render(area, &mut buffer);
    let text = buffer
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();
    assert_eq!(text.matches("committed output").count(), 1);
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
    )
    .await?;
    set_active_cell(
        &mut app.chat_widget,
        Box::new(crate::history_cell::StreamingAgentTailCell::new(
            vec!["live continuation wraps at the resized width".into()],
            /*is_first_line*/ false,
        )),
    );
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Resize(ratatui::layout::Size::new(
            /*width*/ 20, /*height*/ 12,
        )),
    )
    .await?;
    app.close_transcript_overlay(&mut tui);
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn key_capture_receives_voice_shortcut_and_chord_prefix() -> Result<()> {
    let (mut app, mut rx, _op_rx) = super::make_test_app_with_channels().await;
    let mut app_server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    for config in ["", "[chat]\ntoggle_voice = 'f8 v'"] {
        app.keymap = RuntimeKeymap::from_config(&toml::from_str(config)?).unwrap();
        app.chat_widget.open_keymap_capture(
            "composer".into(),
            "submit".into(),
            crate::app_event::KeymapEditIntent::ReplaceAll,
            crate::app_event::KeymapCaptureMode::SingleKey,
            &app.keymap,
        );
        press(&mut app, &mut tui, &mut app_server, KeyCode::F(8).into()).await?;
        assert!(
            matches!(rx.try_recv()?, crate::app_event::AppEvent::KeymapCaptured { key, .. } if key == "f8")
        );
    }
    Ok(())
}
