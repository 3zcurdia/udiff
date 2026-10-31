# udiff — Architecture

Shared context for every agent working on this project. It describes the system
as it exists today, the target domain it is growing toward, and the rules every
plan and every change must follow. When this file and the code disagree, the
code wins and this file is fixed in the same change.

---

## 1. Product

udiff is a micro terminal clone of opendiff / kdiff3 whose single purpose is
resolving git merge conflicts: it opens one conflicted file, shows the two
conflicting sides (remote, local) above an editable merged result (current) in a
fixed three-panel layout, lets the user step through the conflicts, pick a side
or type the resolution, and saves the marker-free file back — staged, so the
ongoing merge or rebase can continue. It is for terminal-first developers on
macOS and Linux who find GUI merge tools too heavy and hand-editing conflict
markers error-prone. The single idea that makes it different: opendiff's
focused, conflict-by-conflict ergonomics at a micro footprint — one dependency,
tiny binary, instant startup — in any terminal, SSH included.

Current state (what is built):

| Area | Status |
| --- | --- |
| Toolchain pin (mise → Rust 1.99.1, edition 2024) | built |
| Product definition (`README.md`) | built |
| Crate skeleton (`Cargo.toml`, Hello-World `src/main.rs`) | built |
| Conflict parsing (git marker grammar) | not started |
| Three-panel TUI (remote / local / current) | not started |
| Per-conflict resolution + current-panel editing | not started |
| Save + stage (`git add`) | not started |
| `git mergetool` entry (1-arg / 4-arg forms) | not started |
| Branch labels (marker labels + HEAD resolution) | not started |
| Theme layer (built-in palette + Omarchy follow) | not started |
| Unit tests (`cargo test`) | not started |

Keep this table honest. A row flips to `built` in the same change that builds
it, never before. When the product charges money, the table always carries a
`Payments` row and an `Analytics events` row.

Recon note (2026-10-30): the repository has no commits yet and `src/main.rs` is
a Hello-World placeholder — every `not started` row is genuinely unbuilt code.

### 1.1 Monetization

- **Model and price:** None — free and open source.
- **Conversion event:** None — there is no paying conversion. No feature doc may
  add one without an explicit user decision recorded here first.
- **Payment provider:** None.
- **Free boundary:** Everything. Every user has every feature; there is no paid
  tier and no gate to place one against.

## 2. Stack

- **Language:** Rust, edition 2024, pinned to `rust = "1.99.1"` in `mise.toml`.
  Change the pin and the file together or neither. Run `mise install` before any
  `cargo` command.
- **UI:** `ratatui 0.30` — a terminal (TUI) application with a fixed three-panel
  layout. Crossterm is imported as `ratatui::crossterm`; it must never be added
  as a direct dependency (version drift).
- **Database:** none. The conflicted file in place is the only persistence
  (§3). There are no schema files and no migrations.
- **Auth, background work, caching, file storage:** none of these exist. One
  thread, no network, no log files, no config file.
- **Testing:** `cargo test` (Rust std harness). Tests live in
  `#[cfg(test)] mod tests` inside `src/` modules (this repo's `AGENTS.md` rule).
- **Lint and formatting:** `cargo clippy --all-targets -- -D warnings` and
  `cargo fmt`, both with defaults. No custom rules, no `clippy.toml`, no
  `rustfmt.toml`.
- **Deploy target:** one native binary (`target/release/udiff`) for macOS and
  Linux. Windows is not targeted and not tested.

Commands the build agent runs (always through the project's own entry points):

```
mise install
cargo run -- <file>
cargo fmt --check && cargo check --all-targets && cargo clippy --all-targets -- -D warnings && cargo test
cargo build --release
cargo test
```

This list is authoritative. A feature doc's verify step and
`implement-feature`'s final verification both come from here, so every line must
be copy-pasteable on a clean checkout of this machine.

One-time git wiring — run by the user on their machine, never by the build
agent:

```
git config --global merge.tool udiff
git config --global mergetool.udiff.cmd 'udiff "$REMOTE" "$LOCAL" "$BASE" "$MERGED"'
git config --global mergetool.udiff.trustExitCode true
```

## 3. Domain map

One subsection per bounded area, one line per concept, each naming the file
that owns it. A reader who has never opened the repo should be able to predict
where a thing lives.

### Conflict document — `src/conflict.rs`

- `ConflictFile` — the whole parsed file: `regions: Vec<Region>`,
  `ends_with_newline: bool`. One instance per file per process; parse either
  returns it or a `ParseError`, never a partial file.
- `Region` — `Region::Text(String)` (a read-only byte-exact run of text outside
  conflicts) or `Region::Conflict(Conflict)`. Document order is significant and
  preserved.
- `Conflict` — one marker block: `remote_label: String`, `remote_text: String`
  (the side between `=======` and `>>>>>>>`), `local_label: String`,
  `local_text: String` (the side between `<<<<<<<` and `=======`), and
  `current_text: String` (the resolution being built; starts as
  `remote_text + local_text`, assumption A2). Identified by its **1-based
  document-order index**, `conflict 1..N`; unique by construction.
- Marker grammar — a line starting with 7 or more `<` opens a conflict and its
  label is the rest of the line; a line of exactly 7 or more `=` and nothing
  else separates the sides; a line starting with 7 or more `>` closes it and
  carries the remote label. Marker lines (with their line terminator) belong to
  the grammar and are never part of any text run.

### Session — `src/app.rs`

- `App` — `path: PathBuf`, `file: ConflictFile`, `selected: usize` (0-based
  internally, displayed 1-based), `cursor: (usize, usize)` (line, column inside
  the selected conflict's `current_text`), `mode: Mode`, `dirty: bool`,
  `message: Option<Message>`, `local_branch: Option<String>` (resolved once at
  startup). One per process. States: `clean` / `dirty`.
- `Mode` — `Normal` or `Edit`. In `Normal` the single-key commands run; in
  `Edit` every printable key is text.
- `Message` — `Message::Info(String)` or `Message::Error(String)`; transient,
  replaced on the next key event.
- `Action` — the user intent of one key event (`NextConflict`, `TakeRemote`,
  `Save`, …), produced by `ui::translate_key` and consumed by `App::dispatch`.
- `Outcome` — `Outcome::Continue`, `Outcome::SaveRequested { text: String }`, or
  `Outcome::Quit { saved: bool }`; the only way the pure core tells the shell
  to do I/O.

### Git boundary — `src/git.rs`

- Branch label resolution — a marker label of `HEAD` or empty is resolved via
  `git branch --show-current`, falling back to `git rev-parse --short HEAD` when
  detached (mid-rebase); any failure falls back to the literal marker label.
  The remote label is always the marker label verbatim.
- Staging — `git add -- <file>` after a successful write; the only write-side
  git call in the project.

### TUI — `src/ui/`

- Screen — exactly one exists: the three-panel merge view with a status line.
  Rendering (`ui::render`) and key translation (`ui::translate_key`) live here;
  nothing else.
- `Theme` / `Role` — the nine color roles every widget consumes
  (`src/ui/theme/mod.rs`); sourced either from the active Omarchy theme
  (`src/ui/theme/omarchy.rs`) or from the built-in fallback palette.

### Target domain (to be built)

- Nothing is planned beyond the system above for v1. The out-of-scope list in
  `features/000-project-setup.md` §1.5 is the boundary; anything re-scoped
  starts as `features/001-…` and updates this section first.
- Later (names reserved, shapes not designed): `MultiFileSession` (a navigator
  over all conflicted files — out of scope today), `SyntaxHighlight`
  (read-only decoration of panels — out of scope today).

Plans that touch this area build toward these names. A plan that needs to
diverge says so explicitly in its design-considered section and updates this
file.

## 4. Where code lives

| Kind of code | Location | Rule |
| --- | --- | --- |
| CLI arg parsing, terminal lifecycle, event-loop glue, file I/O | `src/main.rs` | Glue only: wires pure decisions to I/O. No parsing of files, no state machine, no drawing. |
| Marker grammar and file parsing | `src/conflict.rs` | Pure: no I/O, no terminal, no subprocess. Anything that decides what a file *means* lives here. |
| Session state machine, save output, save-time validation | `src/app.rs` | Pure: no terminal, no subprocess, no `std::fs`. Decides *what* is saved; never saves it. |
| Rendering and key → `Action` translation | `src/ui/mod.rs` | Renders `App` state and translates input. May not parse files, may not decide resolutions, may not exec. |
| Theme roles, role mapping, fallback palette | `src/ui/theme/mod.rs` | The only place a color value may be declared. |
| OS-theme (Omarchy) follow | `src/ui/theme/omarchy.rs` | The only module that reads theme files. |
| git subprocess calls | `src/git.rs` | The only module allowed to run a subprocess (`std::process::Command`). |
| Unit tests | `#[cfg(test)] mod tests` inside each `src/` module | Inline `&str` fixtures; no cross-module test helpers, no dev-dependencies. |

The one rule that decides which layer owns a decision: **a decision belongs to
the layer that can make it without a terminal and without a subprocess.**
`src/ui/` may not parse or decide; `src/conflict.rs` and `src/app.rs` may not
render or exec; only `src/git.rs` and `src/main.rs` touch the outside world.

## 5. Design rules

Source: the product and engineering interviews of 2026-10-30 (recorded in
`features/000-project-setup.md` §1.6) and this repo's `AGENTS.md`.

1. **Micro is the product.** No dependency beyond `ratatui 0.30`; no
   dev-dependencies. A new crate needs an explicit, recorded reason in the
   feature doc that adds it. Prevents footprint creep in a tool whose appeal is
   its size.
2. **Pure core, thin shell.** Parsing and state live in `src/conflict.rs` and
   `src/app.rs` and are testable without a TTY or a subprocess. Prevents
   untestable logic from hiding behind terminal code.
3. **The marker file is the source of truth.** Panel content and branch labels
   come from the conflicted file's marker blocks; git temp files are never read.
   Prevents two diverging renderings of "the sides".
4. **Exactly two git calls.** Read-only label resolution at startup
   (`git branch --show-current`, then `git rev-parse --short HEAD`) and
   `git add -- <file>` on save — both in `src/git.rs`, nothing else ever execs.
   Prevents hidden coupling to git behavior throughout the codebase.
5. **Conflicted files only.** A file without conflict markers is refused
   (`udiff: no conflicts in <file>`, exit non-zero). Never grow into a general
   diff tool — "micro" is the point (`AGENTS.md`).
6. **Only conflict text is editable.** Text outside conflicts is displayed
   read-only and is byte-preserved on save. Prevents a merge tool from
   mangling the parts of the file git already resolved.
7. **One look, OS-followed.** Widgets consume `Theme` roles only;
   `Color::Rgb`, `Color::new` and hex literals are banned outside
   `src/ui/theme/`. The theme switches only with the OS theme (Omarchy follow).
   There is no udiff config file.
8. **Single thread, synchronous.** No threads, no timers, no fixed-FPS loop;
   redraw on input events only (plus a theme-change redraw). Prevents CPU burn
   and concurrency bugs in a one-file tool.
9. **Errors name the line before the TUI opens; inside the TUI they are
   status-line messages.** Load/parse failures print one message with a line
   number and exit non-zero with nothing written; runtime errors never blank
   the screen or lose the user's text.
10. **Tests live in `#[cfg(test)]` modules in `src/`.** Inline fixtures, temp
    files under `std::env::temp_dir()`, no test-only crates (`AGENTS.md`).

### Rust specifics that follow from the rules

- ratatui 0.30 re-exports crossterm: `use ratatui::crossterm;`. Adding a direct
  `crossterm` dependency is a rule violation — the versions must not drift.
- Terminal setup and teardown are paired on **every** exit path (an RAII guard
  in `src/main.rs` restores the terminal on drop), or the user's shell is left
  in raw mode after an error.
- The TUI needs a real TTY. `cargo run` under a pipe or CI prints garbage — run
  it in a terminal; non-interactive use goes through `--help` / `--version`.
- `Cargo.lock` is committed (binary crate) — keep it that way.
- Run `mise install` before any `cargo` command so the pinned toolchain is used.

## 6. Interface conventions

- **Rendering strategy: one screen, immediate-mode redraw on each input event.**
  The narrow exception: the event poll uses a 1-second timeout so the theme can
  be re-checked; a timeout redraws only when the theme actually changed.
- **Component library:** ratatui primitives only (`Frame`, `Layout`, `Block`,
  `Paragraph`). A widget may not name a raw color — it asks the `Theme` for a
  role. No custom widget crates.
- **Color and theme:** tokens are the nine roles of `Theme`
  (`border`, `border_active`, `label_remote`, `label_local`, `label_current`,
  `selection`, `status`, `status_modified`, `status_error`), declared in
  `src/ui/theme/mod.rs`. The theme switches by OS config only: auto-follow of
  the active Omarchy theme — probe `~/.local/state/omarchy/current/theme/colors.toml`,
  fallback `~/.config/omarchy/current/theme`, `UDIFF_OMARCHY_DIR` overrides for
  tests; a tolerant flat `key = "#RRGGBB"` parse of `accent`, `muted`,
  `background`, `foreground`, `red`, `cyan` and legacy `color0`–`color15`;
  mapped onto the roles with a contrast-distance guard; the theme directory's
  `colors.toml` mtime is compared once per second and a redraw happens only on
  actual change; missing or unreadable → built-in fallback palette on
  `Color::Reset` + named colors. **Banned shortcut:** `Color::Rgb`,
  `Color::new` or hex literals outside `src/ui/theme/`. Bold is reserved for
  the active panel border and the current-conflict highlight.
- **State handling:** per surface —

  | Surface | Loading | Empty | Error | Success |
  | --- | --- | --- | --- | --- |
  | Startup (pre-TUI) | none — the file read is instant | zero conflicts → `udiff: no conflicts in <file>`, exit non-zero, no TUI | one-line message with line number, exit non-zero, nothing written | TUI opens with conflict 1 of N selected |
  | Main TUI | none — first frame draws immediately | unreachable (empty never opens the TUI) | never blanks: errors are status-line messages, the editor keeps the text | three panels, branch labels, `conflict i of N`, live current text |
  | Status line | — | — | `error: line 12: missing ======= in conflict 2`; after save: `warning: resolved file saved but not staged` (then exit non-zero) | `conflict 3 of 7 · modified`; `saved and staged` on success |

- **Accessibility floor:** keyboard-only (mouse is disabled); every state is
  named in text as well as color (`modified`, `conflict 3 of 7`); below 60×12
  cells the status line shows `terminal too small (need 60x12)` and the panels
  are not drawn (assumption A3 in `features/000-project-setup.md` §1.6).

## 7. Testing contract

- **Runner:** `cargo test` (std harness). Tests are `#[cfg(test)] mod tests`
  inside each `src/` module, named after their scenario
  (`parses_two_conflicts_preserving_text_runs`), with inline `&str` fixtures and
  local helper functions in the test module. Save tests write real files under
  `std::env::temp_dir()` named `udiff-test-<pid>-<counter>` and remove them.
- **Every new surface must be tested for:** the happy path, empty input,
  malformed input (with the reported line number), the boundary case (first and
  last conflict), and byte-preservation of text outside conflicts.
- **External systems are never called from tests.** `src/git.rs` is the only
  exec'er and its subprocess calls are kept as thin wrappers; the tested
  surface is the pure mapping beside them (command construction, label
  fallback). The pattern is in `src/git.rs`.
- Done means: `cargo fmt --check && cargo check --all-targets && cargo clippy --all-targets -- -D warnings && cargo test`
  all exit zero, and every acceptance criterion is covered by a passing test or
  by the command named for it in the feature doc.

## 8. Security and data

- **Nothing is logged anywhere.** There are no log files and no telemetry; file
  contents never leave the machine. Error messages carry fixed strings and line
  numbers, never file contents.
- **Ownership is enforced by the OS, nothing else.** udiff reads and writes
  only the file path given on its command line; it never touches `.git/`
  internals directly (only through `git add`). An unauthorized request is the
  OS refusing (EACCES) — udiff prints the OS error and exits non-zero.
- **Secrets: none exist.** The only environment variable udiff reads is
  `UDIFF_OMARCHY_DIR` (theme directory override, mainly for tests). No `.env`
  file exists or is allowed.
- **Rate limiting: none** — there is no network surface.
