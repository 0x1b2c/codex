//! Check first-frame editor and screen settings using the real client configuration loader.

use super::*;
use clap::Parser;
use crossterm::event::KeyCode;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn selected_profile_controls_submit_before_the_first_frame() -> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    std::fs::write(
        home.path().join("config.toml"),
        "disable_paste_burst = false\n",
    )?;
    let selected = AbsolutePathBuf::from_absolute_path(home.path().join("work.config.toml"))?;
    let cli = Cli::try_parse_from(["codex"])?;
    for (submit, expected_bindings) in [
        ("[]", Vec::new()),
        ("\"f12\"", vec![crate::key_hint::plain(KeyCode::F(12))]),
    ] {
        std::fs::write(
            &selected,
            // Presentation must not compile the local execution profile.
            format!(
                "default_permissions = \"server-only\"\ndisable_paste_burst = true\n[permissions.server-only]\nextends = \"missing-local-parent\"\n[tui.keymap.composer]\nsubmit = {submit}\n"
            ),
        )?;
        let presentation = load(
            &cli,
            home.path(),
            LoaderOverrides {
                user_config_path: Some(selected.clone()),
                user_config_profile: Some("work".parse()?),
                ignore_project_config: true,
                ..LoaderOverrides::without_managed_config_for_tests()
            },
            Vec::new(),
            /*config_cwd*/ None,
        )
        .await?;
        assert_eq!(
            (
                presentation.screen.keymap.composer.submit,
                presentation.screen.disable_paste_burst,
            ),
            (expected_bindings, true),
        );
    }
    Ok(())
}

#[tokio::test]
async fn first_frame_respects_screen_and_status_line_overrides() -> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    for (config, args, expected) in [
        ("", vec!["codex"], (true, true, true)),
        ("[tui]\n", vec!["codex"], (true, true, true)),
        ("", vec!["codex", "--no-alt-screen"], (false, false, true)),
        (
            "[tui]\nfullscreen_transcript = true\n",
            vec!["codex"],
            (true, true, true),
        ),
        (
            "[tui]\nfullscreen_transcript = true\n",
            vec!["codex", "--no-alt-screen"],
            (false, false, true),
        ),
        (
            "[tui]\nfullscreen_transcript = false\n",
            vec!["codex"],
            (true, false, true),
        ),
        (
            "[tui]\nfullscreen_transcript = true\nalternate_screen = \"never\"\nstatus_line = []\n",
            vec!["codex"],
            (false, false, false),
        ),
    ] {
        std::fs::write(home.path().join("config.toml"), config)?;
        let cli = Cli::try_parse_from(args)?;
        let presentation = load(
            &cli,
            home.path(),
            LoaderOverrides {
                ignore_project_config: true,
                ..LoaderOverrides::without_managed_config_for_tests()
            },
            Vec::new(),
            /*config_cwd*/ None,
        )
        .await?;
        assert_eq!(
            (
                presentation.screen.use_alt_screen,
                presentation.screen.transcript_mode.is_owned(),
                presentation.screen.status_line_enabled
            ),
            expected,
        );
    }
    Ok(())
}

#[tokio::test]
async fn tui_no_daemon_preference_selects_embedded_before_discovery() -> anyhow::Result<()> {
    for (setting, cli_flag, explicit_remote, config_override, expected) in [
        ("", false, false, None, false),
        ("no_daemon = false", false, false, None, false),
        ("no_daemon = true", false, false, None, true),
        ("no_daemon = false", true, false, None, true),
        ("no_daemon = true", false, true, None, false),
        ("no_daemon = true", false, false, Some(false), false),
        ("no_daemon = false", false, false, Some(true), true),
    ] {
        let home = tempfile::tempdir()?;
        std::fs::write(
            home.path().join("config.toml"),
            format!("[features]\ndaemon_auto_start = true\n[tui]\n{setting}\n"),
        )?;
        let mut cli = Cli::try_parse_from(["codex"])?;
        cli.no_daemon = cli_flag;
        let presentation = load(
            &cli,
            home.path(),
            LoaderOverrides {
                ignore_project_config: true,
                ..LoaderOverrides::without_managed_config_for_tests()
            },
            config_override
                .map(|value| vec![("tui.no_daemon".to_string(), toml::Value::Boolean(value))])
                .unwrap_or_default(),
            None,
        )
        .await?;
        crate::daemon_startup::apply_no_daemon_preference(
            &mut cli,
            presentation.local_settings.tui.no_daemon,
            explicit_remote,
        );
        assert_eq!(cli.no_daemon, expected);
        assert_eq!(
            presentation
                .bootstrap_config
                .config_toml
                .features
                .as_ref()
                .and_then(|features| features.entries().get("daemon_auto_start").copied()),
            Some(true),
        );
        let excluded =
            crate::daemon_startup::exclusion(&cli, &[], &LoaderOverrides::default(), false, None)
                .is_some();
        assert_eq!(excluded, expected);
        let socket = AbsolutePathBuf::from_absolute_path(home.path().join("daemon.sock"))?;
        let endpoint = explicit_remote.then(|| crate::RemoteAppServerEndpoint::UnixSocket {
            socket_path: socket.clone(),
        });
        let target =
            crate::app_server_target_for_launch(endpoint, Some(socket), !excluded, false, None)?;
        if explicit_remote {
            assert!(matches!(target, crate::AppServerTarget::Remote { .. }));
        } else if expected {
            assert!(matches!(target, crate::AppServerTarget::Embedded));
        } else {
            assert!(matches!(target, crate::AppServerTarget::LocalDaemon { .. }));
        }
    }
    Ok(())
}
