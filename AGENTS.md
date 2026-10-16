# Local session handoff

This is a local fork of OpenAI Codex maintained by the user for daily use. Upstream currently does not accept external code contributions. Work here addresses concrete usability problems, primarily through small, focused changes. Prefer solutions that are easy to carry across upstream updates and keep long-term maintenance costs low. Do not assume that submitting an upstream PR is the goal.

On entering this checkout, first read the root `STATE.md` for the current task, existing changes, and user agreements. Then read `FORK.md` for the fork's feature list and patch-stack maintenance rules. `STATE.md` is a handoff record; the user may have changed branches, files, or configuration since it was written. Treat the current checkout as authoritative. Update the current status and next steps as work progresses.

Keep the fork's durable documentation in the committed, English-language `FORK.md`, with detailed usage guides in `docs/fork/` when needed. Limit README changes to the fork notice at the top; otherwise retain upstream's content. Update the relevant documentation entry in the same unit as a behavior change. Each topic may contain several cohesive, atomic commits; do not squash a topic merely to make it one commit. Follow `FORK.md` for branch boundaries and upstream syncs. Keep local session state out of public documentation.

Follow the global `AGENTS.md`. The current handoff concerns TUI response rendering and history-scrolling performance; see `STATE.md` for progress. Do not automatically include other previously discussed but unimplemented work in this task.

The user builds and tests the application manually. During this round of collaboration, do not run `just build`, Cargo/Bazel builds, or Rust tests. Follow any new user instructions that change this verification arrangement. Preserve the user's existing changes, especially the `Cargo.lock` differences recorded in `STATE.md`.

This `AGENTS.md` and `STATE.md` record local collaboration state. Keep them local and out of feature commits. Do not accumulate session state in the global `AGENTS.md`.
