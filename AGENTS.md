# AGENTS.md

## What this is

`udiff` is a micro opendiff/kdiff3 clone whose **only** purpose is resolving git
merge conflicts. Human-facing overview: `README.md` — keep it in sync when the
design changes.

## Hard constraints

- **Scope**: merge-conflict resolution only. Reject feature creep — "micro" is
  the point. The crate currently has **zero dependencies**; keep it that way
  unless a dependency is clearly unavoidable.
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

- `src/main.rs` is the single entrypoint and is still a Hello-World placeholder;
  real code does not exist yet. There is no test suite or CI. Add tests with the
  standard `#[cfg(test)]` modules in `src/` as code lands.
