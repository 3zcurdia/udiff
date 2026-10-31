# Feature: 000 — Project setup

<!--
File name: .opencode/docs/features/000-project-setup.md
Written by the init-context skill once the two interviews are complete.
Executed by implement-feature, which follows it top to bottom.
Every section is mandatory; write "None" rather than leaving one out.
This file is never deleted and never renumbered: 000 is always the scaffold.
-->

**Status:** done
**Author:** init-context skill, approved by luis
**Date:** 2026-10-30
**Architecture version:** commit none yet (the repository has no commits as of 2026-10-30)

---

## 1. Product requirements (PRD)

### 1.1 Problem

Terminal-first developers spend part of every merge or rebase hand-editing
`<<<<<<<`/`=======`/`>>>>>>>` markers (slow, error-prone) or opening a GUI
merge tool — opendiff is unavailable on Linux, kdiff3 is too much GUI for the
job. This scaffold must therefore stand up a project whose first running
artifact already solves that problem end to end: a micro TUI that opens one
conflicted file, steps through its conflicts, and saves a marker-free, staged
result. The product is a single micro binary, so feature zero builds the whole
core loop — the scaffold and the product skeleton are the same change. Later
features (001+) exist only to re-scope items from §1.5 if the user asks.

### 1.2 Users and roles

Single role: the developer at the terminal (owner = ordinary member; there is
no permission model). On day one they can open a conflicted file, step through
its conflicts, choose remote/local/both or type the resolution, save and stage
the file, and quit without touching the file. Outsider: not applicable — no
network, no accounts, only local files the OS already lets the user write.
Unauthenticated visitor: not applicable — there is no server and nothing to
authenticate to.

### 1.3 User stories

1. As a developer mid-merge, I see all conflicts of a file in one three-panel
   view (remote | local over current) so I understand each conflict without
   opening anything else.
2. As a developer, I jump to the next/previous conflict with a keystroke so I
   can work through a file fast.
3. As a developer, I navigate between conflict regions (seeing the non-conflict
   text around them read-only) and for the selected conflict choose remote,
   local or both (in that order) or hand-edit its current content — nothing
   outside a conflict is ever modified.
4. As a developer, I hand-edit the current (bottom) panel so I can write a
   resolution that is not exactly remote or local.
5. As a developer, I save the merged result — the file is written conflict-free
   and staged so `git rebase --continue` / `git merge --continue` can proceed.
6. As a developer, I can quit without saving and leave the file untouched.

### 1.4 Acceptance criteria

1. **Given** a clean checkout, **when** `mise install` and
   `cargo build --release` run, **then** `target/release/udiff` exists and
   `udiff --version` and `udiff --help` each exit 0. [scaffold]
2. **Given** a file with at least one conflict marker block, **when** udiff
   opens it, **then** the TUI shows the number of conflicts and the first one
   is selected. [product AC1]
3. **Given** an open file with two or more conflicts, **when** the user
   requests next/previous conflict, **then** the selection moves and all three
   panels show that conflict's content; at the first and last conflict the
   selection stops (no wrap). [product AC2]
4. **Given** a selected conflict, **when** the user takes remote, **then** the
   current panel shows exactly the remote side; taking local shows exactly the
   local side; taking both shows remote then local in that order. [product AC3]
5. **Given** the current panel of the selected conflict, **when** the user
   types, **then** only that conflict's resolution text changes; text outside
   conflicts cannot be edited and is byte-preserved on save. [product AC4]
6. **Given** any state, **when** the user saves, **then** the file on disk
   contains exactly the current panel's content for every conflict plus the
   unchanged text between conflicts, and no conflict marker line remains
   (user-typed marker-like lines are rejected before the write). [product AC5]
7. **Given** any state, **when** the user quits without saving, **then** the
   file is byte-identical to before. [product AC6]
8. **Given** udiff is invoked by `git mergetool` during a merge (4-arg form),
   **when** it opens, **then** the remote and local headers show the two sides'
   branch labels: the marker labels, with the current side's `HEAD`/empty label
   resolved to the branch name (short SHA when detached) when git reports it.
   [product AC7]
9. **Given** a file with zero conflicts, **when** udiff is invoked on it,
   **then** it prints `udiff: no conflicts in <file>` and exits non-zero
   without opening the TUI. [product AC8]
10. **Given** a conflict was resolved and saved, **when** save completes,
    **then** the file is staged (`git add`) and udiff exits 0; if staging
    fails the file is still saved, `warning: resolved file saved but not
    staged` is printed, and udiff exits non-zero. [product AC9]
11. **Given** a wrong number of arguments (0, 2–3, or more than 4 — flags
    excluded), **when** udiff is invoked, **then** it prints usage on stderr
    and exits 2. [scaffold]
12. **Given** the build agent runs the full verify chain, **when** it
    completes, **then** every command exits zero. [scaffold]
13. **Given** an Omarchy theme directory (via `UDIFF_OMARCHY_DIR`) and one
    without, **when** the TUI draws, **then** the palette follows
    `colors.toml` in the first case and the built-in palette in the second.
    [scaffold]

### 1.5 Out of scope

The product will not do these; this list is the boundary between the scaffold
and any future feature:

1. Not a general diff viewer — conflicted files only.
2. No merge-driver / `git config merge.*` integration beyond `git mergetool`.
3. No 2-way diff mode; the three-panel layout is the only view.
4. No syntax highlighting, no themes beyond the OS-followed palette plus one
   built-in fallback.
5. No mouse support.
6. No multi-file navigator (udiff never lists all conflicted files).
7. No undo/redo history beyond the editing session.
8. No non-text (binary) files — refused with a clear message.
9. No diff3/ancestor ("base") column — remote and local only.
10. No udiff config file or dotfile; key bindings are fixed.

### 1.6 Open questions resolved

| # | Question | Answer |
| --- | --- | --- |
| Q1 | Who has the problem, and when? | Terminal-first developers mid `git merge`/`git rebase`/`git pull` resolving hunks one at a time; markers are error-prone, GUI tools (opendiff missing on Linux, kdiff3 too heavy) are friction. |
| Q2 | Users and roles? | Single role: the current terminal user. No permission model. Outsider/unauthenticated: not applicable. |
| Q3 | User stories? | All six proposed stories kept; the refined story 3 limits editing to conflict regions and keeps the rest read-only and byte-preserved. |
| Q4 | Core flow and entry model? | One file per invocation. Primary entry is `git mergetool` (opendiff/Xcode style); direct `udiff <file>` also works. Panel headers show each side's branch name when git reports it. Save stages the file for the ongoing merge/rebase. |
| Q5 | Acceptance criteria? | Nine product ACs confirmed; next/prev stop at boundaries (no wrap); zero-conflict file exits non-zero with a message and no TUI. |
| Q6 | Out of scope? | The ten items in §1.5 accepted as listed. |
| Q7 | How do we know it worked? | All ACs pass and a real `git rebase` resolves end to end via `git mergetool` (no markers left, `--continue` works), with a micro footprint (stripped binary in the low MBs) and instant startup. |
| Q8 | Monetization? | None — free, open source; no conversion event, no provider, no free/paid boundary. |
| Q9 | Dependencies? | Minimal: `ratatui` 0.30 only (crossterm via `ratatui::crossterm`); `clap`, `anyhow`/`thiserror`, `unicode-width` rejected. "It's micro for a reason." |
| Q10 | Where does logic live? | Pure core / thin shell (`conflict.rs`, `app.rs` pure; `ui/` renders and translates; `git.rs` the only exec'er; `main.rs` glue). |
| Q11 | External contract (git)? | Marker file is the single source of truth for content and labels, hybrid labels (chosen A): marker labels verbatim, `HEAD`/empty resolved via `git branch --show-current` → `git rev-parse --short HEAD` → literal label. Outgoing calls: label resolution at startup (read-only) and `git add -- <file>` on save. |
| Q12 | Data and persistence? | The conflicted file in place is the only persistence; conflicts identified by 1-based document-order index; fixtures are inline `&str` in `#[cfg(test)]` modules plus `std::env::temp_dir()` files; no dev-dependencies. |
| Q13 | Auth and ownership? | None — OS file permissions only; EACCES becomes a message + non-zero exit. |
| Q14 | Validation? | Load-time (readable, UTF-8, well-formed markers; error names the line) and save-time (no user-typed marker-like lines); no per-keystroke validation. |
| Q15 | Caching, async, revalidation? | None — single thread, redraw per input event (plus theme-change redraw), file read once, `git add` awaited; external edits mid-session are not detected (last save wins). |
| Q16 | Interface states? | Persistent top status line with transient one-line messages; no overlays, no popups, no quit confirmation. |
| Q17 | Styling and theming? | One `Theme` role layer; OS-theme follow exactly ampwire-style (Omarchy `colors.toml`, live reload, silent fallback); no udiff config file. |
| Q18 | Testing? | Std `cargo test`, `#[cfg(test)]` modules over the pure core (`conflict.rs`, `app.rs`) plus pure helpers in `git.rs` and `ui/theme`; no golden-frame tests. |
| Q19 | Command list? | The list in `architecture.md §2`, confirmed on this machine (mise, rust 1.99.1). |
| A1 | assumption (not asked) — key bindings | Two modes. Normal: `n` next, `p` previous, `r` take remote, `l` take local, `b` take both, `Enter`/`i` edit, `s` save, `q` quit without saving. Edit: printable keys insert at the cursor, `Enter` newline, `Backspace`/`Delete` delete, arrows move within the current text, `Esc` back to Normal. Rationale: single-key commands and free-form text entry cannot share keys without modes. |
| A2 | assumption (not asked) — initial current text | A conflict's `current_text` starts as `remote_text + local_text` (both, remote first) so an unedited save can never silently drop a side. |
| A3 | assumption (not asked) — minimum terminal size | Below 60×12 cells the status line shows `terminal too small (need 60x12)` and the panels are not drawn. |
| A4 | assumption (not asked) — exit codes and usage | 0 = success or clean quit; 1 = runtime failure (unreadable/invalid file, zero conflicts, save/stage failure); 2 = usage error. Usage line: `usage: udiff <file> | udiff <remote> <local> <base> <merged>`. |
| A5 | assumption (not asked) — detached-HEAD label | When the current side's label is `HEAD`/empty and the repo is detached (mid-rebase), the label shows the short SHA (e.g. `abc1234`). |

---

## 2. Engineering requirements (ERD)

### 2.1 Design considered

Twelve structural choices, each designed twice (full table and reasons in the
Gate 2 record of 2026-10-30):

1. **Dependencies.** A: `ratatui` 0.30 only, everything else `std`. B: plus
   `clap`, `anyhow`/`thiserror`, `unicode-width`. **Chosen: A** — micro
   footprint is a product requirement (architecture.md §5 rule 1).
2. **Where logic lives.** A: pure core / thin shell. B: module-per-feature
   with logic next to the UI. **Chosen: A** — rule: a decision belongs to the
   layer that can make it without a terminal and without a subprocess
   (architecture.md §5 rule 2).
3. **Panel/label source.** A: the marker file is the single source of truth.
   B: git's `$REMOTE`/`$LOCAL` temp files + `git rev-parse`. **Chosen: A** with
   hybrid labels (Q11) — marker blocks already contain both sides and labels
   (architecture.md §5 rule 3).
4. **Test data.** A: inline `&str` fixtures + `std::env::temp_dir()` files.
   B: checked-in `tests/fixtures/` + `tempfile`. **Chosen: A** — zero
   dev-dependencies (architecture.md §5 rule 10).
5. **Auth.** A: none, OS permissions only. B: require a git work tree.
   **Chosen: A** — B breaks legitimate non-repo uses and adds no safety
   (architecture.md §8).
6. **Validation.** A: load-time and save-time. B: load-time only. **Chosen:
   A** — upholds acceptance criterion 6 even against user-typed marker-like
   lines (architecture.md §5 rule 9).
7. **Async.** A: single thread, synchronous. B: background `git add` + timer
   redraw. **Chosen: A** — no CPU burn, no concurrency bugs (architecture.md
   §5 rule 8).
8. **Interface states.** A: one persistent status line with transient
   messages. B: modal overlays/popups. **Chosen: A** — quiet chrome like
   opendiff; no quit confirmation (architecture.md §6).
9. **Theming.** A: `Theme` roles + Omarchy OS-follow (ampwire's mechanism).
   B: plus per-OS detection (macOS `defaults`, pluggable theme systems).
   **Chosen: A** — exactly the proven mechanism; the fallback already adapts
   via the terminal palette (architecture.md §5 rule 7).
10. **Testing.** A: unit tests over the pure core only. B: plus golden-frame
    `TestBackend` tests and `#[ignore]`d mergetool e2e. **Chosen: A** — the
    pure core makes the valuable tests free (architecture.md §7).
11. **Command list.** The `architecture.md §2` list, confirmed on this
    machine. No alternative was defensible.
12. **crossterm access.** A: `use ratatui::crossterm` (re-export). B: direct
    `crossterm` dependency. **Chosen: A** — versions must not drift
    (architecture.md §5 "Rust specifics").

### 2.2 Structure and types

Every file created or modified, with its exact shape. `->` marks a new file,
`~` a modified one.

```
~ Cargo.toml
  [package] name = "udiff", version = "0.1.0", edition = "2024"   (unchanged)
  [dependencies]
  ratatui = "0.30"                        # the one and only dependency
  (no [dev-dependencies] section is added)

→ Cargo.lock
  generated by cargo on first build after the dependency is added; committed.

~ src/main.rs
  use std::process::ExitCode;
  mod app; mod conflict; mod git; mod ui;

  enum Invocation
    File(PathBuf)                                      # udiff <file>
    Mergetool { remote: PathBuf, local: PathBuf,
                base: PathBuf, merged: PathBuf }        # udiff <remote> <local> <base> <merged>
    Help                                               # --help | -h
    Version                                            # --version | -V

  struct TerminalGuard                                # RAII: raw mode + alternate screen on; restored on Drop
  fn main() -> ExitCode
  fn parse_args(args: &[String]) -> Result<Invocation, ()>
  fn open_and_run(path: &Path) -> Result<i32, String>  # parse, build App, run the loop; returns the exit code

→ src/conflict.rs
  struct ConflictFile { regions: Vec<Region>, ends_with_newline: bool }
  enum Region { Text(String), Conflict(Conflict) }
  struct Conflict
    remote_label : String    # the `>>>>>>>` label, verbatim
    remote_text  : String    # the side between `=======` and `>>>>>>>`, lines with their terminators
    local_label  : String    # the `<<<<<<<` label, verbatim
    local_text   : String    # the side between `<<<<<<<` and `=======`, lines with their terminators
    current_text : String    # starts as remote_text + local_text (A2); mutated only by App actions
  struct ParseError { line: usize, kind: ParseErrorKind }
  enum ParseErrorKind { MissingSeparator, MissingClose, MissingOpen, NestedOpen }
  fn parse(text: &str) -> Result<ConflictFile, ParseError>
  fn render_resolved(file: &ConflictFile) -> String
  fn count_conflicts(file: &ConflictFile) -> usize
  Invariants: regions alternate Text/Conflict in document order; marker lines
  (with their line terminators) are consumed by the grammar and appear in no
  text run; parse is all-or-nothing.

→ src/app.rs
  struct App
    path         : PathBuf
    file         : ConflictFile
    selected     : usize          # 0-based internally, displayed 1-based
    cursor       : (usize, usize) # (line, column) inside the selected conflict's current_text
    mode         : Mode
    dirty        : bool
    message      : Option<Message>
    local_branch : Option<String> # resolved once at startup via git.rs
  enum Mode { Normal, Edit }
  enum Message { Info(String), Error(String) }
  enum Action
    NextConflict | PrevConflict
    TakeRemote | TakeLocal | TakeBoth
    EnterEdit | ExitEdit
    InsertChar(char) | InsertNewline | Backspace | Delete
    MoveCursor(Direction)
    Save | Quit | Ignored
  enum Direction { Left, Right, Up, Down }
  enum Outcome
    Continue
    SaveRequested { text: String }
    Quit { saved: bool }
  Invariants: only Conflict::current_text is ever mutated; Text runs are never
  touched; `dirty` turns true on the first mutation and never resets; `selected`
  stays in 0..count_conflicts.

→ src/git.rs
  fn current_branch_label() -> Option<String>
  fn stage(path: &Path) -> Result<(), String>
  fn branch_label_display(marker_label: &str, resolved: Option<&str>) -> String
  fn stage_command_args(path: &Path) -> Vec<String>
  fn branch_display_fallback(sha: Option<String>) -> Option<String>

→ src/ui/mod.rs
  fn render(f: &mut Frame, app: &App, theme: &Theme)
  fn translate_key(key: KeyEvent, mode: Mode) -> Action
  fn draw_status_line(f, area, app, theme)          # internal
  fn draw_panels(f, area, app, theme)               # internal

→ src/ui/theme/mod.rs
  enum Role { Border, BorderActive, LabelRemote, LabelLocal, LabelCurrent,
              Selection, Status, StatusModified, StatusError }
  struct Theme { border: Color, border_active: Color, label_remote: Color,
                 label_local: Color, label_current: Color, selection: Color,
                 status: Color, status_modified: Color, status_error: Color }
  fn default_theme() -> Theme
  fn map_palette(palette: &omarchy::Palette) -> Theme
  fn parse_color(s: &str) -> Option<Color>
  fn distance(a: Color, b: Color) -> f32
  fn lighten(c: Color, amount: f32) -> Color

→ src/ui/theme/omarchy.rs
  struct Palette
    accent, muted, background, foreground, red, cyan : Option<Color>
    color : [Option<Color>; 16]
  fn theme_dir() -> Option<PathBuf>
  fn parse_colors_toml(text: &str) -> Palette
  struct OmarchyWatch { path: Option<PathBuf>, mtime: Option<SystemTime> }
  impl OmarchyWatch
    fn new() -> OmarchyWatch
    fn check(&mut self) -> Option<Theme>   # Some only when the palette actually changed

~ README.md
  Status and Build & run sections updated (keymap, mergetool wiring, binary).
  Keep in sync with the design (AGENTS.md).
```

### 2.3 Data and schema

None — there is no database, no schema file and no migration. The only on-disk
data is the conflicted file itself, written in place; git's conflict-marker
format is the de-facto schema and is owned by git, not by udiff.

### 2.4 Modules and entry points

```
src/conflict.rs
  parse(text: &str) -> Result<ConflictFile, ParseError>
    Splits the file into Text and Conflict regions; all-or-nothing; on failure
    reports the 1-based line number and the ParseErrorKind. Pure.
  render_resolved(file: &ConflictFile) -> String
    Concatenates Text runs and each conflict's current_text, byte-exact;
    never emits a marker line from the original file.
  count_conflicts(file: &ConflictFile) -> usize
    Number of Conflict regions (the N in "conflict i of N").

src/app.rs
  App::new(path: PathBuf, file: ConflictFile, local_branch: Option<String>) -> App
    Starts on conflict 1 in Normal mode, cursor at the end of the current
    text, clean, no message.
  App::dispatch(&mut self, action: Action) -> Outcome
    Applies one action to the pure state; Save returns SaveRequested with the
    fully rendered text (validation already applied); Quit returns Quit with
    the saved flag. Never performs I/O.
  App::validate_save(&self) -> Result<(), String>
    Rejects current_text containing a line starting with 7+ `<`, `=` or `>`
    characters (marker-like lines); returns the error text for the status line.
  App::selected_conflict(&self) -> &Conflict
    The conflict under the selection; panics only on an empty file, which
    parse + the zero-conflict exit make unreachable.

src/git.rs
  current_branch_label() -> Option<String>
    Runs `git branch --show-current`; empty result (detached) falls back to
    `git rev-parse --short HEAD`; any failure returns None. Read-only.
  stage(path: &Path) -> Result<(), String>
    Runs `git add -- <path>`; exit 0 returns Ok; non-zero returns the fixed
    error string "resolved file saved but not staged". Idempotent; never
    retried automatically.
  branch_label_display(marker_label: &str, resolved: Option<&str>) -> String
    Pure: "HEAD"/empty -> resolved label or "local"; anything else -> the
    marker label verbatim. Remote labels never go through this.

src/main.rs
  parse_args(args: &[String]) -> Result<Invocation, ()>
    Pure argv parsing per the CLI surface (§2.5); () means usage error.
  open_and_run(path: &Path) -> Result<i32, String>
    Reads the file (errors name the file; parse errors name the line), refuses
    zero-conflict files, resolves the branch label via git.rs, builds App and
    Theme, runs the event loop, and on SaveRequested writes the file and calls
    git::stage. Returns the process exit code.

src/ui/mod.rs
  render(f: &mut Frame, app: &App, theme: &Theme)
    Draws the status line and the three panels from App state; decides nothing.
  translate_key(key: KeyEvent, mode: Mode) -> Action
    Pure key -> Action mapping per the keymap (§2.7); in Edit mode printable
    keys become InsertChar.

src/ui/theme/mod.rs
  default_theme() -> Theme
    The built-in fallback palette: Color::Reset/named colors, bold on
    border_active only.
  map_palette(palette: &omarchy::Palette) -> Theme
    Omarchy palette -> roles (accent -> border_active/selection, foreground ->
    status/labels, red -> status_error, muted -> border, cyan -> label_remote;
    contrast-distance guard: a role whose color is within distance 40 of its
    background neighbor is lightened by 0.35).

src/ui/theme/omarchy.rs
  theme_dir() -> Option<PathBuf>
    UDIFF_OMARCHY_DIR (must be a directory) -> ~/.local/state/omarchy/current/theme
    -> ~/.config/omarchy/current/theme; each candidate must contain
    colors.toml; else None.
  parse_colors_toml(text: &str) -> Palette
    Tolerant flat `key = "#RRGGBB"` parse: named keys accent, muted,
    background, foreground, red, cyan plus legacy color0-color15; ignores
    unknown keys and comments; never fails.
  OmarchyWatch::check(&mut self) -> Option<Theme>
    Re-reads colors.toml only when its mtime changed; returns Some(theme) on
    actual change, None otherwise.
```

### 2.5 Routes and entry points

The CLI is the only entry surface (src/main.rs):

```
src/main.rs   CLI   udiff <file>
  Opens the conflicted file in the TUI. Success: exits 0 after save+stage or
  after quit-without-save. Failure: message on stderr, exit 1 (unreadable
  file, invalid UTF-8, malformed markers with line number, zero conflicts).

src/main.rs   CLI   udiff <remote> <local> <base> <merged>     (git mergetool form)
  Same as above with <merged> as the file; args 1-3 are accepted and ignored
  for content (sides and labels come from the markers, architecture.md §5 rule 3).

src/main.rs   CLI   udiff --help | -h
  Prints the usage line and the keymap summary on stdout, exit 0. No TUI.

src/main.rs   CLI   udiff --version | -V
  Prints `udiff 0.1.0` on stdout, exit 0. No TUI.

src/main.rs   CLI   udiff    (0 args), 2-3 args, or >4 args
  Prints `usage: udiff <file> | udiff <remote> <local> <base> <merged>` on
  stderr, exit 2.
```

### 2.6 Authorization and ownership

None — architecture.md §8: OS file permissions are the whole model. udiff
touches only the file path given on its command line. An unauthorized request
is the OS refusing read/write (EACCES): the OS error is printed and udiff
exits non-zero. No new file paths are created except the resolved write to the
same path and (indirectly) git's own index update.

### 2.7 Interface

One screen (src/ui/mod.rs), the three-panel merge view:

```
src/ui/mod.rs   TUI screen: three-panel merge view
  layout: row 0 = status line; the remaining rows split horizontally: top
  half = remote panel (left) + local panel (right) at 50/50; bottom half =
  current panel (full width). Below 60x12 cells: status line shows
  `terminal too small (need 60x12)`, panels not drawn (A3).
  panel titles: `remote: <label>`, `local: <label>`, `current`.
  labels: remote = the marker label verbatim; local = branch_label_display(...)
  per §2.4 (A5).
  status line: `conflict i of N · <remote label> | <local label> · modified`
  plus the transient Message area (replaced on the next key event).
  states: loading (none — draws on the first event or theme change);
  empty (unreachable — zero conflicts exit pre-TUI); error (status line, editor
  keeps its text); success (panels + labels + counter + live current text).
```

Keymap (assumption A1):

```
Normal mode
  n          next conflict (stops at the last)
  p          previous conflict (stops at the first)
  r          take remote   (current = remote_text)
  l          take local    (current = local_text)
  b          take both     (current = remote_text + local_text)
  Enter / i  enter Edit mode
  s          save (write + stage; exits 0 on success)
  q          quit without saving (no confirmation)

Edit mode
  printable  insert at the cursor
  Enter      insert newline
  Backspace  delete before the cursor
  Delete     delete at the cursor
  arrows     move the cursor within the current text (bounded)
  Esc        back to Normal mode
  (all command keys are inert here — `q` types the letter q)
```

Mouse input is disabled (out of scope §1.5 item 5).

### 2.8 Client-side behavior

None — there is no client/server split. The TUI runs in one process on the
user's machine.

### 2.9 External dependencies

The only external system is the `git` CLI:

```
git (CLI)
  contract (incoming): git mergetool runs
    `udiff "$REMOTE" "$LOCAL" "$BASE" "$MERGED"` via mergetool.udiff.cmd;
    udiff accepts 1 arg (file) or 4 args (mergetool form, $MERGED is the file),
    plus --help/--version/-h/-V. Anything else is a usage error (exit 2).
  contract (outgoing 1, startup, read-only): `git branch --show-current`
    -> branch name; empty (detached) -> `git rev-parse --short HEAD` -> short
    SHA (A5); failure or not a repo -> None -> the literal marker label is
    shown.
  contract (outgoing 2, on save): `git add -- <file>`; exit 0 = staged;
    non-zero = `warning: resolved file saved but not staged`, exit non-zero.
  failures: git missing / not a repo / permission denied -> call 1 falls back
    silently to marker labels; call 2 warns and exits non-zero. The file write
    happens before call 2 and is never lost.
  retry / idempotency: no retries; `git add` is idempotent, call 1 is
    read-only. No timeouts beyond the OS default, no rate limits, no network.
  owned by: src/git.rs (the only module that runs a subprocess)
```

### 2.10 Configuration

```
UDIFF_OMARCHY_DIR                    default: unset
  Overrides the Omarchy theme directory (used by tests). Absent -> probe
  ~/.local/state/omarchy/current/theme, then ~/.config/omarchy/current/theme;
  neither readable -> the built-in palette. Nothing breaks when absent.

git config merge.tool = udiff        default: unset (user runs the wiring)
  Absent -> `git mergetool` needs `-t udiff`; direct `udiff <file>` unaffected.

git config mergetool.udiff.cmd       default: unset (user runs the wiring)
  Absent -> `git mergetool` cannot launch udiff.

git config mergetool.udiff.trustExitCode   default: unset (user runs the wiring)
  Absent -> git ignores udiff's exit code and would continue after a failed
  save; set it so a failed resolution stops the merge.
```

No other environment variables, no config file, no background schedules
(architecture.md §5 rule 7).

### 2.11 Dependencies

```
cargo add ratatui@0.30   ratatui 0.30   the TUI framework — the product itself;
                                        crossterm arrives as ratatui::crossterm
```

Rejected, with reasons: `clap` (arg parsing — one flag-less argument outweighs
the crate), `anyhow`/`thiserror` (errors — `Box<dyn Error>` and fixed strings
cover every failure), `unicode-width` (wide-character width — rare in code
conflicts; an open question for a later feature if it bites), `tempfile`
(dev-only temp files — `std::env::temp_dir()` with a pid/counter name is
enough). A dependency without a recorded reason is scope creep
(architecture.md §5 rule 1).

### 2.12 Tooling and assets

- Formatting: `cargo fmt` with rustfmt defaults; no `rustfmt.toml`.
- Lint: `cargo clippy --all-targets -- -D warnings` with defaults; no
  `clippy.toml`; no `#[allow]` unless a lint fires on intentional code, which
  is then recorded in §7.
- Fonts, images, icons: none — the UI is text and ratatui borders.
- Generated code: none; nothing needs regeneration.

---

## 3. Test data and fixtures

Inline `&str` constants in the `#[cfg(test)]` modules of `src/` (duplicated
per module on purpose — no cross-module test helpers):

```
src/conflict.rs  (#[cfg(test)] fixtures)
  ONE_CONFLICT_BASIC          one conflict, two lines per side, trailing newline
  TWO_CONFLICTS_WITH_TEXT     text before, between and after the conflicts
  ZERO_CONFLICTS              plain text, no markers (the AC9 path)
  MISSING_SEPARATOR           `<<<<<<<` block closed by `>>>>>>>`, no `=======`
  MISSING_CLOSE               `<<<<<<<` block never closed
  NESTED_OPEN                 `<<<<<<<` inside an open conflict
  CRLF_SIDES                  CRLF line endings inside the sides
  NO_TRAILING_NEWLINE         file without a final newline
  MARKER_LABELS               `<<<<<<< HEAD` / `>>>>>>> feature/x` label parsing
  TYPED_MARKER_LINE           current text containing a `>>>>>>>`-like line

src/app.rs  (#[cfg(test)] fixtures)
  the conflict fixtures above (local copies), plus save-flow tests writing
  std::env::temp_dir()/udiff-test-<pid>-<counter> files, removed by the test.

src/ui/theme/omarchy.rs  (#[cfg(test)] fixtures)
  OMARCHY_COLORS_TOML         named keys: accent, muted, background, foreground,
                              red, cyan
  OMARCHY_LEGACY_COLORS_TOML  only color0-color15
```

Fixture data is named for its scenario, never `one`, `two`, `test`.

---

## 4. Tests

The builder writes exactly these tests, in this order, before or alongside the
code they cover. Acceptance criteria from §1.4 appear in brackets.

```
src/conflict.rs
  parses_one_conflict_with_labels                    [AC2, AC8]
  parses_two_conflicts_preserving_text_runs          [AC2, AC6]
  zero_conflicts_returns_empty_file                  [AC9]
  rejects_missing_separator_with_line                [AC12, AC11]
  rejects_missing_close_with_line                    [AC12, AC11]
  rejects_nested_open_with_line                      [AC12, AC11]
  parses_crlf_sides                                  [AC2]
  preserves_missing_trailing_newline                 [AC6]
  marker_labels_carry_branch_names                   [AC8]
  render_resolved_writes_no_marker_lines             [AC6]
  render_resolved_keeps_text_runs_byte_exact         [AC6]

src/app.rs
  starts_on_first_conflict                           [AC2]
  next_stops_at_last_prev_stops_at_first             [AC3]
  take_remote_sets_current                           [AC4]
  take_local_sets_current                            [AC4]
  take_both_concatenates_remote_then_local           [AC4]
  edit_changes_only_selected_conflict                [AC5]
  typed_marker_line_is_rejected_on_save              [AC6, AC12]
  save_text_equals_original_when_untouched           [AC6, AC7]
  dirty_flag_set_on_first_edit                       [AC5]
  quit_without_save_produces_no_write                [AC7]

src/main.rs
  parse_args_accepts_single_file_form                [AC11]
  parse_args_accepts_mergetool_form                  [AC8, AC11]
  parse_args_rejects_wrong_arity                     [AC11]
  parse_args_accepts_help_and_version                [AC1]

src/git.rs
  branch_label_display_resolves_head_marker          [AC8]
  branch_label_display_keeps_plain_labels            [AC8]
  stage_command_args_are_the_git_add_command         [AC10]

src/ui/theme/mod.rs
  default_theme_resolves_every_role                  [AC13]
  map_palette_maps_roles_with_contrast_guard         [AC13]

src/ui/theme/omarchy.rs
  parse_colors_toml_reads_named_keys                 [AC13]
  parse_colors_toml_reads_legacy_color_keys          [AC13]
  theme_dir_prefers_env_override                     [AC13]
  check_returns_some_only_on_change                  [AC13]
```

Criteria proven by command instead of a test (stated per the template):

- AC1 — `cargo build --release && target/release/udiff --version && target/release/udiff --help`
  (build-order step 8 and the §6 checklist).
- AC12 — the full verify chain in build-order step 9.
- AC5's "byte-preserved on save" and AC6/AC7's file-level bytes are proven by
  the `app.rs` save tests over real temp files.

---

## 5. Build order

Numbered, commit-sized, in dependency order. Each step carries its own verify
command; never batch steps.

1. Add the dependency: `cargo add ratatui@0.30`; confirm `mise install` has
   run. -> `cargo check --all-targets` and `cargo tree --depth 1` lists only
   `ratatui`.
2. `src/conflict.rs` — types, marker grammar, `parse`, `render_resolved`,
   `count_conflicts`, tests first. -> `cargo test conflict::`
3. `src/app.rs` — `App`, `Action`, `Outcome`, `dispatch`, `validate_save`,
   tests first. -> `cargo test app::`
4. `src/git.rs` — label resolution, `stage`, pure helpers, tests first.
   -> `cargo test git::`
5. `src/ui/theme/mod.rs` and `src/ui/theme/omarchy.rs` — roles, fallback
   palette, role mapping, Omarchy follow, tests first. -> `cargo test ui::theme`
6. `src/main.rs` CLI surface — `parse_args`, `--help`, `--version`, usage
   errors (exit 2), zero-conflict and parse errors (exit 1), `TerminalGuard`.
   -> `cargo test parse_args && cargo run -- --help`
7. `src/ui/mod.rs` plus the event loop in `src/main.rs` — `render`,
   `translate_key`, the loop wiring `dispatch` to the file write and
   `git::stage`, status line, panels, keymap. -> `cargo check --all-targets && cargo test`
8. Binary and entry smoke: release build; `--version`/`--help` exit 0.
   -> `cargo build --release && target/release/udiff --version`
9. Full verify chain. -> `cargo fmt --check && cargo check --all-targets && cargo clippy --all-targets -- -D warnings && cargo test` — all green.
10. Docs reconcile: update `README.md` (Build & run, keymap, mergetool wiring)
    and `.opencode/docs/architecture.md` §1 status table plus any section whose
    names, tables or rules changed. -> `git status --short` shows changes only
    under `README.md` and `.opencode/docs/`.

---

## 6. Verification checklist

The builder ticks these before reporting done.

- [x] `mise install` ran; `Cargo.lock` exists and is committed (present and
      not gitignored — this run made no commits: the whole feature is one
      uncommitted tree, per the implement-feature skill's no-commit rule)
- [x] Every file in §2 exists at the exact path written there
- [x] Every test in §4 exists with the exact name and passes (`cargo test`:
      34 tests, 34 §4 names, 0 missing)
- [x] `cargo clippy --all-targets -- -D warnings` reports no errors
- [x] `cargo fmt --check` exits zero
- [x] `cargo build --release` exits zero and `target/release/udiff --version`
      prints `udiff 0.1.0`
- [x] Every acceptance criterion in §1.4 is observably true (each proven by a
      test in §4 or by the command named there; AC2/AC7/AC8/AC10/AC13 also
      observed live in a real pty: three panels + `conflict 1 of 2`, `cmp`-clean
      quit, `local: main` / `local: 63c7fd7` labels under `git mergetool` and
      the 4-arg form, staged save + non-zero-exit stage failure, both palettes)
- [x] `grep -rn "Command::new" src/ | grep -v 'src/git.rs'` returns nothing
      (architecture.md §5 rule 4 — nothing else spawns a process; the
      non-exec `std::process::ExitCode` and `std::process::id()` uses are
      required by §2.2 and architecture.md §7)
- [x] `grep -rn "Color::Rgb\|Color::new\|#[0-9a-fA-F]\{6\}" src/ | grep -v 'src/ui/theme'`
      returns nothing (architecture.md §5 rule 7)
- [x] `cargo tree --depth 1` lists no dependency besides `ratatui`
      (architecture.md §5 rule 1)
- [x] Manual on a real TTY: the TUI opens on a conflicted sample file with the
      three panels and `conflict 1 of N`; `q` quits leaving the file
      byte-identical (`cmp` before/after) [AC7]
- [x] Manual in a real repository: `git mergetool` launches udiff, the headers
      show the branch labels, `s` saves and stages, `git rebase --continue`
      proceeds [AC8, AC10]
- [x] `architecture.md` updated if any name, table or rule changed

---

## 7. Deviations

Filled by the builder. **What** was done differently from this spec: the
section and line, what it said, what was built instead. One row per deviation.
Small ones count — they are the cheapest signal for the next planner. Write
"None — executed verbatim" if empty.

| # | Plan section | Plan said | Built instead |
| --- | --- | --- | --- |
| D1 | §6 checklist, rule-4 grep | `grep -rn "std::process\|Command::new" src/ \| grep -v 'src/git.rs'` must return nothing | Narrowed to `grep -rn "Command::new" src/ \| grep -v 'src/git.rs'`; the broad pattern also matched `src/main.rs:7` (`std::process::ExitCode`, mandated by §2.2) and `src/app.rs:403` / `src/ui/theme/omarchy.rs:208` (`std::process::id()`, mandated by architecture.md §7 temp names), none of which spawn a process. User-approved 2026-10-31. |
| D2 | §3 fixtures | `TYPED_MARKER_LINE` listed under `src/conflict.rs`, and app.rs to hold "the conflict fixtures above (local copies)" | `TYPED_MARKER_LINE` is declared only in `src/app.rs` (its only test lives there); app.rs copies only the three fixtures its tests use. An unused fixture constant is `dead_code` and fails `clippy -D warnings` (§2.12). |
| D3 | §2.4 `OmarchyWatch` | `check` returns "Some only when the palette actually changed"; no fn listed for the initial theme load | `new()` observes no mtime, so the first `check()` is the initial load and returns `Some` (the shell uses it for the initial theme). architecture.md §4 forbids any module but `omarchy.rs` from reading theme files, and §2.2 lists no loader. |
| D4 | §2.4/§2.10 `theme_dir` | Arrow chain "UDIFF_OMARCHY_DIR → ~/.local/state → ~/.config; each must contain colors.toml; else None" | Strict override: when `UDIFF_OMARCHY_DIR` is set, it must be a directory containing `colors.toml` or the built-in palette is used — the home probe runs only when the variable is unset. The only reading of AC13's "a theme directory … and one without → built-in" that holds on any machine. |
| D5 | §2.2 `src/ui/theme/mod.rs` | `Role` enum listed with no accessor connecting it to `Theme` | Added `Theme::get(role) -> Color`; architecture.md §6 requires widgets to ask the theme for a role instead of naming colors, and an unconsumed `Role` is `dead_code`. |
| D6 | §2.7 TUI screen | Layout and titles fixed, panel *content* unspecified | All three panels render the whole document (byte-exact text runs, every conflict, selected conflict highlighted and scrolled into view with 2 lines of context) — user story 3 requires seeing the non-conflict text around conflicts read-only, and AC3 requires the selected conflict to be shown. |
| D7 | §6 status line | `saved and staged` listed as a status-line success message | §2.7's keymap makes `s` exit on success, so the status line cannot hold it; udiff prints `saved and staged` to stdout after the terminal is restored (stage failure prints `warning: resolved file saved but not staged` on stderr, per AC10). |
| D8 | §2.4 `parse` error line | "reports the 1-based line number" without naming which line | The detection line: the `>>>>>>>` line for `MissingSeparator`, the opening `<<<<<<<` line for `MissingClose` (EOF has no line), the offending marker line for `MissingOpen`/`NestedOpen`. A second `=======` inside a conflict is remote-side content (save-time validation rejects it). |

---

## 8. Drifts

Filled by the builder, one entry per deviation in §7. Why the spec and reality
diverged, so the next plan does not repeat it. Each drift has a cause:

- `plan-gap` — the spec did not cover a case the code had to handle.
- `stale-context` — the spec described code or schema that had already changed.
- `convention-conflict` — the spec contradicted a rule in `architecture.md` or
  a pattern in the nearest similar file.
- `infeasible` — the spec could not pass as written (failing constraint,
  missing dependency, library behavior).
- `user-decision` — the user changed the requirement during the build.

```
D1
  Cause: infeasible
  Evidence: §6 checklist vs src/main.rs:7 (`use std::process::ExitCode;`,
    required by §2.2 `fn main() -> ExitCode`), src/app.rs:403 and
    src/ui/theme/omarchy.rs:208 (`std::process::id()`, required by
    architecture.md §7 `udiff-test-<pid>-<counter>`); the grep could never
    return nothing without breaking one of those two requirements.
  Why the spec missed it: the grep operationalised architecture.md §5 rule 4
    ("nothing else ever execs") with the wider pattern `std::process`, which
    also matches the non-exec std APIs the same doc mandates.
  Fix applied: checklist grep narrowed to `Command::new` (user-approved
    2026-10-31); `Command::new` occurs only in src/git.rs.
  Follow-up: none — architecture.md §5 rule 4 needs no change; the rule text
    is about exec, not the `std::process` path.

D2
  Cause: plan-gap
  Evidence: §3 lists `TYPED_MARKER_LINE` under src/conflict.rs though only
    src/app.rs's `typed_marker_line_is_rejected_on_save` uses it;
    `cargo check --all-targets` warned "constant `MARKER_LABELS` is never
    used" for the mirror-image copy in app.rs (clippy §2 lint is `-D warnings`).
  Why the spec missed it: §3's "duplicated per module on purpose" assumes
    every module uses every fixture; unused `const`s in `#[cfg(test)]` are
    `dead_code` and fail the verify chain.
  Fix applied: each fixture lives only in the module whose §4 tests use it.
  Follow-up: none.

D3
  Cause: plan-gap
  Evidence: §2.2's omarchy fn list is `theme_dir`, `parse_colors_toml`,
    `OmarchyWatch::{new, check}` — there is no initial-theme loader, and
    architecture.md §4 says only src/ui/theme/omarchy.rs reads theme files,
    so the shell cannot load the first theme itself.
  Why the spec missed it: `check` was specified as change-only, leaving the
    first frame without a theme source.
  Fix applied: `new()` records no mtime, so the first `check()` is the
    initial observation and returns `Some(theme)`; later calls return `Some`
    only on a real mtime/directory change (`check_returns_some_only_on_change`
    pins all four transitions).
  Follow-up: none.

D4
  Cause: plan-gap
  Evidence: §2.4 theme_dir's chain vs §1.4 AC13 ("a theme directory via
    UDIFF_OMARCHY_DIR and one without → colors.toml palette … built-in
    palette") — with fall-through, an override pointing at a directory
    without colors.toml would silently use the home theme, not built-in.
  Why the spec missed it: the arrow chain reads both as candidate order and
    as override semantics; AC13 only holds under strict override semantics
    on machines with Omarchy installed.
  Fix applied: `UDIFF_OMARCHY_DIR` set → that directory must contain
    colors.toml or the built-in palette is used; home probes only when unset.
  Follow-up: none.

D5
  Cause: plan-gap
  Evidence: §2.2 lists `enum Role` (nine variants) and the private `Theme`
    fields but no accessor; architecture.md §6: "A widget may not name a raw
    color — it asks the `Theme` for a role."
  Why the spec missed it: the struct listing shows the data, not the
    read path; without one the `Role` variants are dead code under §2.12.
  Fix applied: `Theme::get(role) -> Color` in src/ui/theme/mod.rs, used by
    every widget in src/ui/mod.rs.
  Follow-up: none — architecture.md §6 already states the rule it implements.

D6
  Cause: plan-gap
  Evidence: §2.7 fixes layout, titles and the status line but never says
    what the panel bodies contain; §1.3 story 3 requires "seeing the
    non-conflict text around them read-only" and AC3 requires the selected
    conflict to be visible in all three panels.
  Why the spec missed it: the layout spec was written at the chrome level.
  Fix applied: whole-document rendering per panel (text runs + every
    conflict's side for that panel), selected conflict highlighted with
    `Role::Selection`, scrolled to its start minus 2 context lines with the
    edit caret kept in view.
  Follow-up: none.

D7
  Cause: plan-gap
  Evidence: §6 lists `saved and staged` as a status-line message while
    §2.7's keymap says `s` "write + stage; exits 0 on success" — a message
    drawn into a screen that is left immediately cannot be seen.
  Why the spec missed it: the status table and the keymap were specified
    independently.
  Fix applied: on success the shell restores the terminal and prints the
    app's `Message::Info("saved and staged")` to stdout; on stage failure it
    prints `warning: resolved file saved but not staged` (AC10 exact) and
    exits 1; the write always precedes staging.
  Follow-up: none.

D8
  Cause: plan-gap
  Evidence: §2.4 says parse errors report "the 1-based line number" but a
    `MissingClose` is only detected at EOF, which has no line number.
  Why the spec missed it: it named the field, not the semantics per kind.
  Fix applied: the detection line (close-marker line for MissingSeparator,
    opening-marker line for MissingClose, offending marker for
    MissingOpen/NestedOpen); a later `=======` inside a conflict is
    remote-side content, caught by `validate_save` instead.
  Follow-up: none.
```

Set `Status: done` only after §7 and §8 are filled and every §6 box is ticked.

---

Status: done
Date: 2026-10-30
Commit: none yet (no commits in the repository as of 2026-10-30)
