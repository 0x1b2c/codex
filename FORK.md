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
Horizontal `h` and `l` stay on the current logical line, and word text objects
handle whitespace consistently.

The same topic extends the existing Vim editor with counted motions and edits,
counted dot repeat, repeated character searches, paste-before, characterwise and
linewise Visual editing, line joining, bracket matching, paragraph motions and
text objects, and numbered, named, and black-hole registers.

*Topic: `vim-editing`. Current range: `d8c2783d49..9e20c3ad07`.*

### Prompt-cache timing should be visible

The status line offers an estimated prompt-cache expiry time after prompt
submission. This is a local estimate, not confirmation of server-side cache
retention or a guarantee that the next request will hit the cache. The segment
is omitted when its value is unknown.

*Topic: `cache-expiry`. Current range: `283cdd1476..d8c2783d49`.*

### Build and developer experience

**`local-build`** adds `just build` to build and install both the CLI and its
required code-mode host, including the prebuilt V8 setup. Building only the CLI
can otherwise leave the companion executable missing.

*Current range: `rust-v0.162.1..283cdd1476`.*

## Patch-stack conventions

### Layout

Topics form one linear stack on a recorded upstream base. Each topic branch
points to the final commit of that topic, and `master` is the integration tip.
These are boundaries on one history, not independent branches repeatedly merged
into the integration branch.

The intended layout for the existing topics is:

```text
upstream base
  -> fork-base
  -> local-build
  -> cache-expiry
  -> vim-editing
  -> master
```

`fork-base` contains the fork notice, this document, and shared fork-maintenance
rules, so later topics can update their documentation in the same commit as the
behavior they introduce. Local session notes and machine-specific settings do
not belong in that layer.

The existing implementation stack is based on upstream tag `rust-v0.162.1`
(`092d3acd6bec3e3a14bdc7e7a2810ab628ab759d`). It is currently integrated on
`cache-expiry`, including the Vim changes. The topic names above describe the
target layout; those separate topic refs and the bottom `fork-base` layer have
not yet been established. They will be organized during the next upstream sync,
without rewriting history just to introduce these conventions. The ranges in the
feature list identify the actual implementation boundaries until then.

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
refs still mark the intended boundaries. Branch refs need to move along with the
stack; `rebase --update-refs` supports this for eligible branches.

When upstream supersedes a patch, retain its entry with a strikethrough and an
explanation. Preserve the original patch under `fork/superseded/<topic>` before
dropping it from the active stack. Creating or publishing those tags follows
the maintainer's normal Git workflow.

## Building and running

From the repository root:

```sh
just build
```

The default is a development build; use `just build --release` for a release
build. This command also installs the binaries under `~/.local/lib/codex-local/`
and creates `~/.local/bin/codex-local` and
`~/.local/bin/codex-code-mode-host` links. Run `codex-local` to use the local CLI.
It does not replace the official `codex` command.

## License and attribution

The upstream Apache-2.0 license and attribution notices are unchanged. See
[LICENSE](LICENSE) and [NOTICE](NOTICE).
