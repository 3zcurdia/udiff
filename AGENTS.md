# AGENTS.md

## What this is

`udiff` is a micro opendiff/kdiff3 clone whose **only** purpose is resolving git
merge conflicts. Human-facing overview: `README.md` — keep it in sync when the
design changes.

## Hard constraints

- **Scope**: merge-conflict resolution only. Reject feature creep — "micro" is
  the point. The crate has exactly one dependency, `ratatui` (terminal I/O).
  Add no others.
- The one sanctioned non-conflict feature is Omarchy theme following
  (`src/ui/theme/omarchy.rs`). Keep it, and don't treat it as precedent for
  more.
- **UI layout is fixed** (opendiff three-panel design):

  ```
  | remote | local  |
  |-----------------|
  |     current     |
  ```

  `remote` + `local` are the conflicting inputs; `current` is the editable
  merged result. Don't rearrange or rename these panels.

## Toolchain

- Rust **edition 2024**, pinned to `rust = "1.99.1"` via `mise.toml`. Run
  `mise install` before any `cargo` command so you don't silently build with a
  different toolchain.

## Repo state

```
src/main.rs        shell: argv, terminal lifecycle, event loop, file I/O, exit codes
src/conflict.rs    pure: marker grammar, parse, render_resolved
src/app.rs         pure: session state machine (selection, edits, save gate)
src/git.rs         the only subprocess boundary (branch label, git add)
src/ui/mod.rs      draw + key → Action translation
src/ui/theme/      color roles; omarchy.rs follows the OS theme
```

Tests live in `#[cfg(test)]` modules in `src/` and in `tests/cli.rs`.
`PLAN.md` tracks remaining work.
