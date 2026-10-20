# Codex personal fork

A personal fork of [OpenAI Codex](https://github.com/openai/codex), maintained by
[0x1b2c](https://github.com/0x1b2c).

## Overview

This fork carries focused fixes and workflow improvements for daily use. It is
not a separate product: Codex's name, license, and upstream attribution remain
unchanged. Keeping the patch stack small and inexpensive to carry across upstream
updates is a priority.

The README keeps upstream's content apart from the fork notice. This file
describes the fork's differences; longer fork-specific usage guides belong in
`docs/fork/` when needed.

## Problems this fork solves

Entries describe a problem and its solution at topic-branch granularity. A topic
can contain several cohesive commits; individual commands and implementation
steps do not each need a separate entry.

### Vim editing should work naturally with Chinese text

Vim word motions in the composer now use punctuation and whitespace boundaries
for Chinese text, without introducing dictionary-based word segmentation.
The `w` / `b` / `e` motions and their `W` / `B` / `E` counterparts distinguish
word and whitespace-delimited WORD boundaries. Horizontal `h` and `l` stay on
the current logical line, and word text objects handle whitespace consistently.

The same topic extends the existing Vim editor with counted motions and edits,
counted dot repeat, repeated character searches, paste-before, characterwise and
linewise Visual editing, line joining, bracket matching, paragraph motions and
text objects, and numbered, named, and black-hole registers. It also adds
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

*Topic: `vim-editing`. Range: `wheel-scrolling..vim-editing`.*

### Prompt-cache timing should be visible

The status line offers an estimated prompt-cache expiry time after prompt
submission. This is a local estimate, not confirmation of server-side cache
retention or a guarantee that the next request will hit the cache. The segment
is omitted when its value is unknown.

*Topic: `cache-expiry`. Range: `local-build..cache-expiry`.*

### Usage warnings should not interrupt the conversation

Proactive usage-threshold warnings no longer add messages to the conversation or
warning count. Check `/status` or the status line for usage; the composer usage
notice, actual limit errors, and model-switch and credit-recovery flows remain
available.

*Topic: `usage-warnings`. Range: `cache-expiry..usage-warnings`.*

### History scrolling should not redraw an old frame for every wheel event

In the main fullscreen view, consecutive wheel events update the scroll position
without synchronously painting an intermediate frame when the displayed viewport
geometry is unchanged. The existing frame scheduler draws the accumulated
position. Initial layout, terminal resizes, and composer-height changes still
refresh geometry before handling the wheel; clicks and selection drags retain
their synchronous layout refresh.

*Topic: `wheel-scrolling`. Range: `usage-warnings..wheel-scrolling`.*

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

*Topic: `tui-no-daemon`. Range: `vim-editing..tui-no-daemon`.*

### Build and developer experience

**`local-build`** adds an optimized release profile, `just build`, and
`just install-fork`. The build recipe builds the complete CLI and its code-mode
host together, without requiring an official Codex installation or a separate
build script. Installation is a separate step; see
[Building and running](#building-and-running).

*Topic: `local-build`. Range: `fork-base..local-build`.*

## Patch-stack conventions

### Layout

Topics form one linear stack on a recorded upstream base. Each topic branch
points to the final commit of that topic, and `master` is the integration tip.
These are boundaries on one history, not independent branches repeatedly merged
into the integration branch.

The current stack is:

```text
rust-v0.162.1
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

The stack is based on upstream tag `rust-v0.162.1`
(`092d3acd6bec3e3a14bdc7e7a2810ab628ab759d`). The `lock-alignment` layer is
the first patch above upstream: it aligns workspace package versions in the lock
file without changing external dependencies. The build recipe's `--locked`
guard belongs to `local-build`.

All topic refs above mark actual boundaries on the same linear history.
`master` adds integration-level documentation and the simplified two-binary
`local-build` follow-up after `tui-no-daemon`. Topic refs retain their current
boundaries until the next deliberate stack reorganization.
`backup/patch-stack-before-reorder-20261020` preserves the integration tip before
the first stack reorganization; keep that backup fixed during future rebases.
The reorganization did not change the upstream base or any tracked content
outside this document.

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
`workspace.package.version` in `codex-rs/Cargo.toml`. The upstream `rust-v0.162.1`
release changed the manifest version but left local package versions in the lock
file at `0.0.0`; this fork records the matching versions without changing external
dependencies. Check this alignment again when updating the upstream base.

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
just build
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

Additional arguments are forwarded to Cargo. The default uses the native target;
for example, `just build --target aarch64-apple-darwin` selects that explicit
target and can reuse the target layout used by the former build script.

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
By default, the two executables are `codex-rs/target/release/codex` and
`codex-rs/target/release/codex-code-mode-host`. `CARGO_TARGET_DIR`, Cargo
configuration, or `--target` can change this location; an explicit target adds
its triple before `release/`. Keep both executables together so the CLI can find
its companion.

On macOS and Linux, `just install-fork` installs the existing binaries into
`~/.local/bin`; it does not build them. Both sources are copied successfully
before either command is replaced. Existing file symlinks are replaced with
regular executable files, leaving their targets untouched; directory destinations
are rejected. The upstream `just install` recipe only prepares development
dependencies and is unchanged.

The first optional argument selects the build output directory, relative to
`codex-rs/` unless absolute. For an explicit target:

```sh
just build --target aarch64-apple-darwin
just install-fork target/aarch64-apple-darwin/release
```

If `CARGO_TARGET_DIR` or Cargo configuration changes the output location, pass
that directory explicitly. A second optional argument selects the installation
prefix, for example `just install-fork /tmp/codex-target/release /tmp/codex-prefix`
installs both commands into `/tmp/codex-prefix/bin`.

The recipe does not change `PATH`. Put `~/.local/bin` before other Codex
installations in `PATH`, then run `codex`. The command has no fork-specific
suffix. Windows builds produce the corresponding `.exe` files; copy both into
the same directory on `PATH`.

## License and attribution

The upstream Apache-2.0 license and attribution notices are unchanged. See
[LICENSE](LICENSE) and [NOTICE](NOTICE).
