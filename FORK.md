# Codex personal fork

A personal fork of [OpenAI Codex](https://github.com/openai/codex), maintained by
[0x1b2c](https://github.com/0x1b2c).

## At a glance

- **Vim editing:** A left-aligned mode indicator, Chinese-aware word boundaries,
  counted commands, Visual editing, and a limited set of registers, not a complete
  Vim implementation.
- **History navigation:** Ctrl+U/D, Ctrl+B/F, and `gg`/`G` browse the transcript
  from Vim Normal mode without changing the draft.
- **Return to the latest output:** Ctrl+Option+E (Ctrl+Alt+E) works regardless
  of the composer's editing mode, without using Esc as a history shortcut.
- **Less interruption:** remove proactive usage-threshold warnings; retain
  `/status` and actual limit errors.
- **Optional indicators and isolation:** a local prompt-cache time estimate
  and `tui.no_daemon` for terminal sessions that do not join the shared daemon.
- **Build and install:** `just build` builds the complete CLI and code-mode host;
  `just install-fork` builds and installs both under their usual names.

The fork also changes wheel-event rendering. Its independent performance benefit
has not yet been established; increasing wheel travel uses an existing upstream
configuration option, not a new fork feature.

**Release policy:** follow upstream's latest stable releases, not `main` or
prereleases. A large commit gap from upstream `main` is expected and does not,
by itself, indicate that the fork is unmaintained. The recorded base below shows
which release this checkout currently follows.

Start with [feature activation](#enabling-the-features) or
[building and running](#building-and-running). A [简体中文版](FORK.zh-CN.md)
is also available; patch-stack details are further below.

## Overview

This fork carries focused fixes and workflow improvements for daily use. It is
not a separate product: Codex's name, license, and upstream attribution remain
unchanged. Keeping the patch stack small and inexpensive to carry across upstream
updates is a priority.

The README keeps upstream's content apart from the fork notice. This file
describes the fork's differences; longer fork-specific usage guides belong in
`docs/fork/` when needed.

## Enabling the features

Configuration examples below belong in `~/.codex/config.toml`. Merge them into
the existing `[tui]` section rather than creating a second one, and preserve
unrelated settings. Restart the CLI after editing the file. The `/vim` and
`/statusline` commands can change their respective settings in the running UI.

| Feature                                           | Activation and default                                                                                                                   |
| ------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| Prompt-cache estimate                             | Opt in: add `"cache-expiry"` to `tui.status_line`, or select it in `/statusline`. It is not in the default status line.                  |
| Vim editing and Normal-mode history motions       | Enable Vim mode with `/vim`, or set `tui.vim_mode_default = true` for future launches. Vim mode is off by default.                       |
| Return to the latest output                       | Ctrl+Option+E (Ctrl+Alt+E) works in the main fullscreen view without configuration or Vim mode.                                          |
| Independent terminal sessions                     | Opt in with `tui.no_daemon = true`. The default is `false`.                                                                              |
| Wheel travel distance                             | Optional upstream setting: `tui.mouse_scroll_speed = 3.0` increases travel; the default is `1.0`.                                        |
| Usage-warning removal and wheel-rendering changes | Automatic in this fork; no enable switch. Wheel-rendering changes apply to the main fullscreen view and do not increase scroll distance. |
| Local build settings and lock-file protection     | Used automatically by `just build`; no CLI configuration setting is needed.                                                              |

The main fullscreen transcript is enabled by default. Inline or raw-output modes
do not provide the same owned-history navigation behavior.

## Problems this fork solves

Entries describe a problem and its solution at topic-branch granularity. A topic
can contain several cohesive commits; individual commands and implementation
steps do not each need a separate entry.

### Vim editing should work naturally with Chinese text

Use `/vim` to toggle Vim mode in the current session. To start future sessions
in Vim Normal mode, set the existing upstream option:

```toml
[tui]
vim_mode_default = true
```

This setting defaults to `false`. The fork extends Vim editing; it does not
enable Vim mode automatically.

During ordinary draft editing, the main composer's lower-left corner identifies
the active Vim mode as
`-- NORMAL --`, `-- INSERT --`, `-- VISUAL --`, `-- VISUAL LINE --`, or
`-- REPLACE --`. The existing shortcuts hint moves to the lower-right corner
when it is otherwise shown and fits. Narrow layouts prioritize the mode label;
the configured status line uses the remaining space. Shared modal editors use
the same labels without changing their existing layout.

Vim word motions in the composer now use punctuation and whitespace boundaries
for Chinese text, without introducing dictionary-based word segmentation.
The `w` / `b` / `e` motions and their `W` / `B` / `E` counterparts distinguish
word and whitespace-delimited WORD boundaries. Horizontal `h` and `l` stay on
the current logical line, and word text objects handle whitespace consistently.

The same topic extends the existing Vim editor with counted motions and edits,
counted dot repeat, repeated character searches, paste-before, characterwise and
linewise Visual editing, line joining, bracket matching, paragraph motions and
text objects, and yank register `"0`, named registers `"a`–`"z`, and black-hole
register `"_`, alongside the existing unnamed buffer. Delete-history registers
`"1`–`"9`, uppercase append registers, and system clipboard registers are not
implemented. It also adds
line-start navigation and `X` / `S`, including correct `cc` handling on empty
lines.

In the main conversation, Vim Normal mode uses Ctrl+U / Ctrl+D to scroll history
by half a page and Ctrl+B / Ctrl+F by a full page, with the same one-row overlap
as Page Up / Page Down. These motions reuse the pager keybindings; explicit active
editor or app bindings take priority. `gg` jumps to the beginning of history,
loading older pages when needed, and `G` returns to the latest output. These jumps
reuse the Vim `jump_top` / `jump_bottom` bindings and browse history even with a
nonempty draft, preserving its text and cursor. Insert and Visual mode and pending
editing commands keep their editor behavior; `h` / `j` / `k` / `l` still move the
draft cursor. Ctrl+T toggles detailed transcript display without changing input
focus in the main fullscreen view.

Ctrl+Option+E on macOS (Ctrl+Alt+E elsewhere) returns to the latest output in the
main fullscreen conversation, whether the composer is in Vim Normal, Insert,
Visual, or non-Vim mode. It preserves the draft, cursor, and editing mode, and is
safe to press repeatedly while a task is running. Ctrl+End and Alt+> still work.
Ordinary history browsing no longer uses Esc to return to the bottom: Esc retains
its existing editing, cancellation, prompt-backtracking, and task-interruption
roles. Popups and dialogs retain ownership of their input.

_Topic: `vim-editing`. Range: `wheel-scrolling..vim-editing`._

### Prompt-cache timing should be visible

This is an optional status-line item, not a default indicator. Run `/statusline`
and select `cache-expiry`, or add `"cache-expiry"` to the existing
`[tui].status_line` array. If no array is configured, this example preserves the
three default items and appends the estimate:

```toml
[tui]
status_line = ["model-with-reasoning", "current-dir", "thread-name", "cache-expiry"]
```

If you already have a custom status line, keep its items and append
`"cache-expiry"` instead of replacing the array with this example.

After a prompt is submitted, the item displays a local clock time such as
`Cache ~12:18`. The current estimate is submission time plus 30 minutes; it is
not a countdown or confirmation of server-side cache retention, and does not
guarantee that the next request will hit the cache. The item is omitted until
the running session has a submission timestamp to estimate from.

_Topic: `cache-expiry`. Range: `local-build..cache-expiry`._

### Usage warnings should not interrupt the conversation

Proactive usage-threshold warnings no longer add messages to the conversation or
warning count. Check `/status` or the status line for usage; the composer usage
notice, actual limit errors, and model-switch and credit-recovery flows remain
available.

_Topic: `usage-warnings`. Range: `cache-expiry..usage-warnings`._

### History scrolling should not redraw an old frame for every wheel event

Over the transcript in the main fullscreen view, consecutive wheel events update
the scroll position without synchronously painting an intermediate frame when
the displayed viewport geometry is unchanged. The existing frame scheduler draws
the accumulated position. Initial layout, terminal resizes, and composer-height
changes still refresh geometry before handling the wheel; clicks and selection
drags retain their synchronous layout refresh. Wheel events over selectable
footer or turn-tip text also refresh those regions synchronously before
transcript scrolling.

_Topic: `wheel-scrolling`. Range: `usage-warnings..wheel-scrolling`._

### Terminal sessions can stay independent of the app's daemon

The terminal TUI can use its own embedded app server even when a shared daemon
already exists. Set this in the existing `[tui]` section of `~/.codex/config.toml`:

```toml
[tui]
no_daemon = true
```

This is equivalent to `--no-daemon` for local TUI launches, including resume and
fork: it skips both daemon discovery and automatic startup. The default is
`false`, preserving upstream behavior. Explicit `--no-daemon` still takes effect
when the setting is false. Explicit `--remote` connections ignore this local
preference; the existing conflict between `--remote` and the `--no-daemon` flag
remains an error. Restart the CLI after changing the setting.

This preference does not disable the shared daemon for the ChatGPT app, change
`features.daemon_auto_start`, stop an existing daemon, or change remote control.
It does not add daemon version compatibility checks or resolve competing remote
control connections.

_Topic: `tui-no-daemon`. Range: `vim-editing..tui-no-daemon`._

### Build and developer experience

**`local-build`** adds an optimized release profile, `just build`, and
`just install-fork`. The build recipe builds the complete CLI and its code-mode
host together, without requiring an official Codex installation or a separate
build script. The installer builds before installing; see
[Building and running](#building-and-running).

_Topic: `local-build`. Range: `fork-base..local-build`._

## Patch-stack conventions

### Layout

Topics form one linear stack on a recorded upstream base. Each topic branch
points to the final commit of that topic, and `master` is the integration tip.
These are boundaries on one history, not independent branches repeatedly merged
into the integration branch.

The current stack is:

```text
rust-v0.163.0
  -> lock-alignment
  -> fork-base
  -> local-build
  -> cache-expiry
  -> usage-warnings
  -> wheel-scrolling
  -> vim-editing
  -> tui-no-daemon
  -> master
```

`fork-base` contains the fork notice, this document, and shared fork-maintenance
rules, so later topics can update their documentation in the same commit as the
behavior they introduce. Local session notes and machine-specific settings do
not belong in that layer.

The stack is based on upstream tag `rust-v0.163.0`
(`83086188ae0614bd882818ff1aa23f3194962012`). The `lock-alignment` layer is
the first patch above upstream: it aligns workspace package versions in the lock
file without changing external dependencies. The build recipe's `--locked`
guard belongs to `local-build`.

All topic refs above mark actual boundaries on the same linear history.
The two-binary build and installer are consolidated in `local-build`; `master`
adds integration-level documentation after `tui-no-daemon`.
`backup/patch-stack-before-reorder-20261020` preserves the integration tip before
the first stack reorganization; keep that backup fixed during future rebases.
That initial reorganization retained the `rust-v0.162.1` base and did not change
tracked content outside this document. The subsequent upstream sync moved the
stack to `rust-v0.163.0`, retaining upstream changes and all local feature patches.
`backup/before-upstream-0.163.0-20261020` preserves the integration tip before
that sync and must also remain fixed.

### Adding and maintaining patches

- Keep each topic focused on one problem or cohesive feature. Preserve separate
  commits for independent behavior changes; do not squash a topic merely to make
  it a single commit. Fold only pure corrections into the commit they correct
  when history is deliberately being reorganized.
- Update this file with the corresponding behavior change. Describe the problem
  first, then the solution and its limits. Do not list planned work as delivered.
- Document fork-specific behavior here and, when needed, in `docs/fork/`, rather
  than adding it to upstream's release notes. Keep README changes confined to
  the fork notice.
- Once topic branches are published, link each topic using
  `compare/<previous-topic>...<topic>`. Comparing every topic with upstream would
  also show all the lower layers. Prefer branch names to commit hashes so links
  survive an upstream rebase. Do not add links to unpublished or nonexistent refs.
- Keep routine follow-up changes at the integration tip. Reordering them into
  earlier topics can wait for an upstream sync. After a release or shared
  checkpoint, append commits until the next deliberate stack reorganization;
  keep existing release tags fixed.

### Updating the upstream base

Record the old upstream base, integration tip, and topic refs before a sync.
After replaying the stack, use `git range-diff` between the old and new patch
ranges to account for changed, added, and dropped commits, and verify that topic
refs still mark the intended boundaries. Move topic refs along with the stack,
but keep backup refs fixed. When backup refs point into the rewritten range,
do not use automatic ref updates; move only the intended topic refs explicitly.

Keep workspace package versions in `codex-rs/Cargo.lock` aligned with
`workspace.package.version` in `codex-rs/Cargo.toml`. The upstream `rust-v0.162.1` and
`rust-v0.163.0` releases changed the manifest version but left local package
versions in the lock file at `0.0.0`; this fork records the matching versions
without changing external dependencies. Check this alignment again when updating
the upstream base.

When upstream supersedes a patch, retain its entry with a strikethrough and an
explanation. Preserve the original patch under `fork/superseded/<topic>` before
dropping it from the active stack. Creating or publishing those tags follows
the maintainer's normal Git workflow.

## Scrolling configuration

Codex already supports a mouse-wheel speed multiplier. Its default is one text
row per wheel event, which can make trackpad navigation feel slow even when
input is processed promptly. To travel farther with each gesture, set
`mouse_scroll_speed` in `~/.codex/config.toml`, using the existing `[tui]` section
if present:

```toml
[tui]
mouse_scroll_speed = 3.0
```

This requests three rows per wheel event instead of one. Restart the CLI to use
the setting; no rebuild is needed. Adjust the multiplier to taste. This is
configuration tuning using an existing upstream option; the fork leaves the
default unchanged. Scroll distance and rendering latency are separate concerns:
this setting increases travel without fixing processing stalls.

## Building and running

From the repository root:

```sh
just install-fork
```

`just build` enables Cargo's HTML build timing report. Cargo prints the report's
path in the build log.

Local builds use Cargo's `--locked` flag: they use the tracked lock file without
rewriting it. If a manifest change requires a lock-file update, update and review
the lock file explicitly before building again.

The recipe runs Cargo directly with `--release`, building only the `codex` and
`codex-code-mode-host` binaries and their dependencies, not every workspace tool.
The CLI includes the TUI and the usual `resume`, `exec`, and other subcommands.
Both binaries come from this checkout; no installed official CLI or host is
needed. V8's own build script handles its precompiled library download.

Additional arguments to `just build` are forwarded to Cargo. Both recipes default
to an explicit host target (`host-tuple`), or `CARGO_BUILD_TARGET` when set. This
keeps the target-specific cache layout used by earlier fork builds. Omitting an
explicit target uses a different Cargo cache layout; existing caches are not
lost, but switching layouts can trigger extensive recompilation. An explicit
`--target` argument to `just build` overrides this default.

The project's release profile keeps `opt-level = 3`, disables LTO
(`lto = "off"`), uses `codegen-units = 16`, disables debug information
(`debug = 0`), strips symbols (`strip = "symbols"`), and disables incremental
compilation by default (`incremental = false`). Only `codex-tui` enables
incremental compilation, trading extra disk usage for shorter subsequent local
TUI rebuilds. Its first build must generate the incremental cache; final CLI
compilation and linking are still required. These settings reduce compilation and
disk overhead for daily optimized builds, with a possible runtime performance tradeoff.
Backtraces lose source file and line information. Changing the profile triggers
recompilation on the next build; existing cached artifacts are not removed
automatically.

`just build` only builds; it does not install or change existing commands.
By default, the executables are under `codex-rs/target/<host-triple>/release/`.
`CARGO_TARGET_DIR` or Cargo configuration can change the target directory.
Keep both executables together so the CLI can find its companion.

On macOS and Linux, `just install-fork` first builds both binaries, then installs
them into `~/.local/bin`. A failed build leaves the installed commands untouched.
The installer asks Cargo for its target directory, so `CARGO_TARGET_DIR` and
Cargo's `target-dir` configuration apply to both steps without manual paths.
Both sources are copied successfully before either command is replaced. Existing
file symlinks are replaced with regular executable files, leaving their targets
untouched; directory destinations are rejected. The upstream `just install`
recipe only prepares development dependencies and is unchanged.

The first optional argument selects the target triple for both build and install;
the second selects the installation prefix. For example:

```sh
just install-fork aarch64-apple-darwin
just install-fork aarch64-apple-darwin /tmp/codex-prefix
```

The second command installs into `/tmp/codex-prefix/bin`. To change only the
prefix while keeping the default host target, use `host-tuple` as the first
argument. The first argument is a target, not a build output directory.

The recipe does not change `PATH`. Put `~/.local/bin` before other Codex
installations in `PATH`, then run `codex`. The command has no fork-specific
suffix. Windows builds produce the corresponding `.exe` files; copy both into
the same directory on `PATH`.

## License and attribution

The upstream Apache-2.0 license and attribution notices are unchanged. See
[LICENSE](LICENSE) and [NOTICE](NOTICE).
