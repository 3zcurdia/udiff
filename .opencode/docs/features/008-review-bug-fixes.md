# Feature: Code-review bug fixes

<!--
From the /code-review audit of commit 21312ae (2026-11-02), findings 1, 5, 6
and 10. The speed findings are planned separately in
009-render-performance.md, which builds on this doc: build 008 first.
-->

**Status:** draft
**Author:** plan agent (from the 2026-11-02 code review), approval pending (luis)
**Date:** 2026-11-02
**Architecture version:** commit 4fe21a7 of `.opencode/docs/architecture.md`

---

## 1. Product requirements (PRD)

### 1.1 Problem
The code review found four defects that can show the developer the wrong
thing or lose their file:

- **B1, big files:** each panel's vertical offset is passed to ratatui as a
  `u16`. In a file longer than 65,535 lines (lockfiles, generated code), a
  conflict past that line can never be shown. The panels show unrelated text
  thousands of lines above the highlighted conflict.
- **B2, empty resolutions:** a conflict whose current text is empty (the
  developer took an empty side, which is a deletion) produces zero lines. In
  Edit mode the caret is drawn on the next read-only line, as if typing would
  edit it. At the end of the file no caret is drawn at all. An empty remote or
  local side has the same problem: nothing in that panel shows where the
  conflict is.
- **B3, Omarchy theme removed:** when the theme directory disappears or
  `colors.toml` becomes unreadable, `OmarchyWatch::check` records the change
  and returns `None`. The stale palette stays on screen, and because the new
  mtime is recorded, the change is never retried.
- **B4, non-atomic save:** `save` overwrites the file in place with
  `std::fs::write`. A crash, kill or full disk during the write leaves a
  truncated file: the markers and the resolution are both lost.

### 1.2 Users and roles
Single role: the developer at the terminal; no permission model. With this
feature they can resolve conflicts in files of any length, see where an empty
side or resolution is, see the built-in palette when their Omarchy theme goes
away, and never get a half-written file. Outsider and unauthenticated visitor:
not applicable.

### 1.3 User stories
- As a developer resolving a conflicted lockfile with 80,000 lines, I want the
  panels to show the selected conflict so that I can resolve it in udiff.
- As a developer whose resolution is empty, I want the caret on a line inside
  the conflict so that I can see where my typing goes.
- As a developer comparing a deletion against an edit, I want the empty side
  to show a highlighted blank line so that I can see where the conflict is.
- As an Omarchy user who removes or switches off a theme, I want udiff to fall
  back to its built-in colors so that the screen matches my system.
- As a developer whose machine crashes during a save, I want either my old
  file or my new file, never a truncated one.

### 1.4 Acceptance criteria
1. Given a file of 70,000 text lines followed by one conflict, when udiff
   draws the first frame in an 80×24 terminal, then the remote panel shows the
   remote side and the local panel shows the local side of that conflict.
2. Given any file, when the user scrolls with ↑/↓/PgUp/PgDn/Home, then the
   panels move and clamp exactly as before this feature (feature 005).
3. Given a line wider than the panel, when the user scrolls with ←/→, then the
   horizontal scroll is unchanged from feature 005.
4. Given a conflict whose side is empty, then that panel shows exactly one
   blank line for the conflict, drawn with the selection highlight when the
   conflict is selected.
5. Given a selected conflict whose current text is empty, when the user enters
   Edit mode, then the caret is drawn on the blank placeholder line of that
   conflict, also when the conflict is the last region of the file.
6. Given an empty resolution, when the user saves, then the saved file
   contains no line for that conflict (the placeholder is display-only).
7. Given a loaded Omarchy theme, when its `colors.toml` is removed (or its
   directory, or it becomes unreadable), then the next `check()` returns the
   built-in theme once, and the following `check()` returns `None`.
8. Given no Omarchy theme at startup, then the first `check()` returns `None`
   and udiff uses the built-in theme, as before.
9. Given a successful save, then the file holds exactly the resolved text and
   no temporary file is left in its directory.
10. Given a file with mode `0755`, when it is saved, then its mode is still
    `0755` (Unix).
11. Given a path that is a symlink, when it is saved, then the path is still a
    symlink and its target holds the resolved text (Unix).
12. Given a directory where a new file cannot be created, when the user saves,
    then udiff exits 1 with `udiff: cannot write <path>: …` and the original
    file is byte-identical.

### 1.5 Out of scope
- The render speed findings (rebuilding all panels on every frame, redrawing
  on ignored keys, Edit-mode allocations, linear conflict lookups, the
  startup git call). They are planned in `009-render-performance.md`.
- Horizontal offsets above 65,535 columns. They stay clamped to `u16::MAX`.
  No real source line is that wide.
- Keeping the file's owner, group, extended attributes and hard links on save.
  Owner and group would need `libc` (a second dependency, architecture.md §5
  rule 1). The rename gives the file a new inode, so other hard links keep the
  old content. Recorded as A4.
- Any change to when or how often the Omarchy theme is checked.

### 1.6 Open questions resolved

| # | Question | Answer |
| --- | --- | --- |
| A1 | Show a placeholder line for empty sides in all three panels, or only for an empty resolution in the current panel? | All three panels. The review asked only for the current panel, but an empty remote or local side has the same "where is the conflict?" problem, and one rule is simpler than two. Assumption (not asked). |
| A2 | Fix B1 by clamping differently, or by slicing the visible lines in `draw_panel`? | Slice the visible lines and stop using `Paragraph::scroll` for the vertical axis. This removes the `u16` limit completely and is also the first speed step (only about one screen of lines is styled per panel). Assumption (not asked). |
| A3 | `PLAN.md` Phase 4 said "Don't change `OmarchyWatch::check`". Does that still hold? | No. That rule only stopped Phase 4 (a docs-only phase) from changing code. Feature 006 is `done`. B3 changes only what `check` returns when the theme goes away; when and how often it checks stays the same. Assumption (not asked). |
| A4 | Atomic save: keep the owner, group and hard links too? | No. Keep only the permission bits (std can do that). Owner, group and xattrs would need `libc`. The file is normally owned by the user who runs udiff, so the new file has the same owner anyway. Assumption (not asked). |
| A5 | Where does the temp file go? | Next to the real file (after following symlinks), named `.<name>.udiff-<pid>.tmp`. `rename` is atomic only on one filesystem, so the system temp dir cannot be used. Assumption (not asked). |

---

## 2. Engineering requirements (ERD)

### 2.1 Design considered

**B1 — vertical offset beyond `u16`**
**Option A:** keep `Paragraph::scroll((top, x))`, but pass all lines and use a
wider type. Not possible: ratatui 0.30 takes `(u16, u16)`.
**Option B:** in `draw_panel`, build `Line`s only for
`document.lines[top..min(top + rows, len)]` and call
`Paragraph::scroll((0, x))`.
**Chosen:** B. It removes the limit and makes each frame style only about one
screen of lines (architecture.md §5 rule 8: input-driven redraw must stay
cheap).

**B2 — empty conflict sides**
**Option A:** special-case the caret in `draw_panel` (draw a phantom caret row
when `current_text` is empty).
**Option B:** in `panel_document`, an empty conflict piece adds one blank line
owned by its region, in every panel.
**Chosen:** B. Every other rule (selection highlight, unresolved dim/italic,
caret, `selected_start`, scroll clamp) then works without special cases. It is
display-only: `render_resolved` reads `current_text`, not the panel
documents, so nothing is saved (architecture.md §5 rule 6).

**B3 — theme removed**
**Option A:** when the new state has no readable palette, return
`Some(default_theme())`.
**Option B:** do not record the new path and mtime on a failed read, so the
next check retries.
**Chosen:** A. B keeps the stale palette and, if the file stays unreadable,
reads it again every second. A matches the module contract ("missing or
unreadable → built-in fallback", architecture.md §6) with no new state.

**B4 — non-atomic save**
**Option A:** write a sibling temp file, copy the permission bits, `sync_all`,
then `rename` it over the real file (after following symlinks).
**Option B:** keep `std::fs::write`, but first copy the original to a `.orig`
backup.
**Chosen:** A. B still truncates the real file during the write, and it leaves
a file behind that git would show as untracked. A uses std only
(architecture.md §5 rule 1).

### 2.2 Domain changes

No type in `src/conflict.rs` or `src/app.rs` changes. The changed units:

```
ui::draw_panel (modified)                                  src/ui/mod.rs
  Builds Lines only for index in top..min(top + rows, document.lines.len()),
  with the same per-line style rules as today (selection, unresolved
  dim/italic, caret on cursor_line). Calls Paragraph::scroll((0, x16)) where
  x16 = x.min(u16::MAX as usize) as u16. The caret logic (follow on
  cursor_line and caret x) is unchanged and runs before the slice.

ui::visible_range(top: usize, rows: usize, len: usize) -> Range<usize> (new)
  top.min(len)..(top + rows).min(len). Empty when len == 0 or rows == 0.

ui::panel_document (modified)                              src/ui/mod.rs
  A Region::Conflict whose piece for this panel is "" contributes "\n"
  instead: exactly one blank line owned by that region. Text regions and
  non-empty pieces are unchanged.
  Invariant after this change: every conflict region owns at least one line
  in every panel, so selected_start < lines.len() always holds.
```

```
OmarchyWatch.check(&mut self) -> Option<Theme> (modified)  src/ui/theme/omarchy.rs
  Unchanged when path and mtime equal the last call: None.
  On a change: records path and mtime, then returns
    Some(map_palette(..)) when colors.toml is readable,
    Some(default_theme()) when it is not (dir gone, file gone, unreadable),
  except on the very first call when there is no theme (path None, mtime
  None): it still returns None. That case is caught by the existing equality
  check, because new() starts with (None, None).
  Import: use super::default_theme.
```

```
main::write_atomically(path: &Path, text: &str) -> io::Result<()>  (new)  src/main.rs
  target = fs::canonicalize(path), or path itself when that fails (the file
  does not exist yet).
  temp   = target.with_file_name(".<file name>.udiff-<pid>.tmp").
  Steps: File::create(temp); write_all(text); when fs::metadata(target)
  succeeds, file.set_permissions(metadata.permissions()); sync_all();
  fs::rename(temp, target).
  On any error: best-effort fs::remove_file(temp), then return the error.
  Never leaves the target truncated: target is touched only by the rename.
  An error for a target with no file name is
  io::Error::new(ErrorKind::InvalidInput, "not a file path").

main::save (modified)
  std::fs::write(path, text) is replaced by write_atomically(path, text).
  The error message ("udiff: cannot write <path>: <error>"), the staging
  logic and the exit codes do not change.
```

### 2.3 Data and migrations
None — no database. The file format does not change: the saved bytes are the
same as before. Only the way they reach the disk changes (temp file, then
rename).

### 2.4 Services, jobs and side effects

```
src/main.rs
  save(path, text, stage)
    write_atomically(path, text) -> then git::stage(path) when stage.
    Retry rule: a failed write leaves the target untouched and removes the
    temp file, so running udiff again is safe. A leftover temp file from a
    kill -9 between create and rename is harmless: it starts with a dot, and
    the next save from a process with the same pid truncates it
    (File::create).
```

### 2.5 Routes

```
src/ui/mod.rs   main TUI screen
  No new key or state. Visible changes: an empty conflict side or empty
  resolution is drawn as one blank line (highlighted when selected), and in
  Edit mode the caret sits on it.
```

### 2.6 Authorization and ownership
None — OS file permissions only (architecture.md §8). New file path: the
sibling temp file `.<name>.udiff-<pid>.tmp` in the directory of the file being
saved, created with the same permission bits as the file and renamed over it.
If the directory is not writable, the save fails with the OS error, exit 1.

### 2.7 HTTP boundary

```
s (Normal mode), all conflicts resolved, no marker lines
  App::dispatch(Save) -> SaveRequested { text } -> main::save
  -> write_atomically -> success: as today (exit 0, or 3 when git add fails)
                      / failure: "udiff: cannot write <path>: <os error>",
                        exit 1, original file byte-identical
```

### 2.8 Views and components

```
src/ui/mod.rs  draw_panel
  Paragraph over the visible slice only; vertical scroll 0, horizontal
  scroll x. Empty and error states unchanged.

src/ui/mod.rs  panel_document
  One blank placeholder line per empty conflict piece. Highlighted with
  Role::Selection when it belongs to the selected conflict; dim and italic in
  the current panel while the conflict is unresolved.
```

### 2.9 Client-side behavior
None.

### 2.10 External dependencies
None changed. `git add` is still called after the write, now after the
rename.

### 2.11 Configuration
None. `UDIFF_OMARCHY_DIR` behaves as before. Pointing it at a directory
without `colors.toml` while udiff runs now switches to the built-in palette
live (before, it only did so at startup).

---

## 3. Test data and fixtures

- `src/ui/mod.rs` tests (new inline fixtures):
  - `huge_prefix_conflict()` — a helper returning a `String`: 70,000 lines
    `"text {n}\n"`, then
    `"<<<<<<< ours\nLOCAL-MARK\n=======\nREMOTE-MARK\n>>>>>>> theirs\n"`.
  - `const EMPTY_SIDES_MIDDLE: &str = "before\n<<<<<<< ours\n=======\n>>>>>>> theirs\nafter\n";`
  - `const EMPTY_SIDES_AT_END: &str = "before\n<<<<<<< ours\n=======\n>>>>>>> theirs\n";`
  - A local helper `screen(app: &mut App) -> String` that draws one frame on
    `ratatui::backend::TestBackend::new(80, 24)` the same way `main` does
    (`app.scroll = clamp_scroll(..)`, then `render(.., &default_theme())`)
    and returns the buffer as text, one row per line. It also returns or
    exposes the buffer so caret tests can check `Modifier::REVERSED` on a
    cell. `TestBackend` is part of ratatui 0.30 (`ratatui::backend`), so this
    is not a new dependency.
- `src/app.rs` tests: the existing `EMPTY_SIDES` fixture.
- `src/ui/theme/omarchy.rs` tests: existing `OMARCHY_COLORS_TOML`,
  `temp_theme_dir`, `EnvGuard` and `lock_env`.
- `src/main.rs` tests: the existing `temp_path(name)` helper. Unix-only tests
  are marked `#[cfg(unix)]` and use `std::os::unix::fs::{PermissionsExt,
  symlink}`.

---

## 4. Tests

```
src/ui/mod.rs
  visible_range_cases                                  [AC1]
    (0, 20, 100) -> 0..20; (90, 20, 100) -> 90..100;
    (5, 20, 3) -> 3..3; (0, 0, 10) -> 0..0
  render_shows_conflict_past_line_65535                [AC1]
    screen(huge_prefix_conflict) contains "REMOTE-MARK" and "LOCAL-MARK"
  scroll_still_clamps_after_slicing                    [AC2, AC3]
    the existing clamp_scroll_saturates fixture drawn through screen():
    after scroll = (1000, 0) the last remote line "remote 29" is visible;
    after (-1000, 0) the first conflict line is visible
  empty_side_gets_one_placeholder_line                 [AC4]
    panel_document over EMPTY_SIDES_MIDDLE, all three panels:
    lines == ["before", "", "after"], line_region[1] == Some(1),
    selected_start == 1
  empty_resolution_caret_sits_inside_conflict          [AC5]
    EMPTY_SIDES_MIDDLE, TakeLocal, EnterEdit, screen(): the current panel's
    caret cell (REVERSED) is on the placeholder row, not on "after"
  empty_resolution_caret_at_end_of_file                [AC5]
    the same with EMPTY_SIDES_AT_END: exactly one REVERSED cell in the
    current panel

src/app.rs
  empty_resolution_saves_no_line                       [AC6]
    "before\n<<<<<<< ours\n=======\nx\n>>>>>>> theirs\nafter\n", TakeLocal,
    Save -> SaveRequested { text: "before\nafter\n" }

src/ui/theme/omarchy.rs
  check_reverts_to_builtin_when_theme_disappears       [AC7]
    load (Some), remove colors.toml, check() == Some(t) with
    t == default_theme(), then check() is None
  check_without_theme_returns_none_first               [AC8]
    UDIFF_OMARCHY_DIR = an empty temp dir: first check() is None

src/main.rs
  save_leaves_no_temp_file                             [AC9]
    save to a temp path in its own temp dir; the dir then holds exactly one
    entry, and its content is "x\n"
  save_keeps_permissions          #[cfg(unix)]         [AC10]
    create the file with mode 0o755, save, mode & 0o777 == 0o755
  save_through_symlink_keeps_link #[cfg(unix)]         [AC11]
    link -> real file; save(link); symlink_metadata(link) is a symlink and
    real file == "x\n"
  failed_save_leaves_original     #[cfg(unix)]         [AC12]
    dir with the original file "orig\n", then chmod the dir to 0o555;
    save -> Err starting with "udiff: cannot write"; file still "orig\n";
    restore 0o755 before cleanup. Skip the assertions (early return) when
    running as root, detected by the chmod having no effect (a probe
    File::create in the dir succeeds).
  save_without_stage_writes_and_exits_0   (existing, unchanged)
  save_with_failed_stage_exits_3          (existing, unchanged)
  save_to_unwritable_path_errors          (existing, unchanged)
```

AC2 and AC3 are also covered by the existing `panel_top_cases`,
`follow_cases`, `clamp_scroll_saturates` and `arrows_scroll_in_normal_mode`,
which must pass unchanged.

---

## 5. Build order

Every verify command runs through mise: `mise exec -- <command>`.
Baseline at 21312ae: 67 unit tests plus 6 CLI tests pass. If yours differs,
stop and report it.

1. **B1.** Add `visible_range`, the `screen` test helper and
   `huge_prefix_conflict`. Write `visible_range_cases`,
   `render_shows_conflict_past_line_65535` and
   `scroll_still_clamps_after_slicing`, and confirm the 65535 test fails.
   Then change `draw_panel` to iterate `visible_range(top, rows, len)` and
   call `Paragraph::scroll((0, x16))`.
   -> `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
   Commit: `008 B1: draw only visible lines; no u16 vertical limit`
2. **B2.** Write `empty_side_gets_one_placeholder_line`,
   `empty_resolution_caret_sits_inside_conflict`,
   `empty_resolution_caret_at_end_of_file` and
   `empty_resolution_saves_no_line`. Then make `panel_document` use `"\n"`
   for an empty conflict piece. Delete the "Known, accepted edge" sentence
   about the empty-resolution caret wherever it still appears in docs
   (`grep -rn "accepted edge" .opencode PLAN.md README.md`).
   -> same command
   Commit: `008 B2: placeholder line for empty conflict sides`
3. **B3.** Write `check_reverts_to_builtin_when_theme_disappears` and
   `check_without_theme_returns_none_first`. Then change the last line of
   `OmarchyWatch::check` to `Some(theme.unwrap_or_else(default_theme))`.
   Update the module doc comment and the `check` doc comment to say that a
   theme that goes away returns the built-in theme once.
   -> same command
   Commit: `008 B3: revert to built-in palette when the Omarchy theme goes away`
4. **B4.** Write the four new `src/main.rs` save tests. Then add
   `write_atomically` and call it from `save`.
   -> same command
   Commit: `008 B4: atomic save through a sibling temp file`
5. `cargo fmt --check && cargo check --all-targets && cargo clippy --all-targets -- -D warnings && cargo test`
   -> all green
6. Update `architecture.md`:
   - §3 "Git boundary → Staging": "after a successful atomic write (temp
     file + rename)".
   - §6 "Color and theme": "a theme that goes away while udiff runs switches
     to the built-in palette on the next check".
   - Add a build note to §1 for feature 008.
   - Update the `README.md` `## Theme` section with the same live fallback
     sentence.
   - Flip the 008 row in `features/README.md` to `done` when closing out.
   -> `grep -n "atomic" .opencode/docs/architecture.md && grep -n "built-in palette" README.md`

Manual check (owner, needs a real terminal; never report it as passed without
running it): generate a 70,000-line conflicted file, run `udiff` on it in an
80×24 terminal, and confirm that the conflict is shown and that PgUp/PgDn move
smoothly.

---

## 6. Verification checklist

- [ ] Every file in §2 exists at the exact path written there
- [ ] Every test in §4 exists with the exact name and passes
- [ ] `cargo clippy --all-targets -- -D warnings` reports no errors
- [ ] `cargo check --all-targets` exits zero
- [ ] Every acceptance criterion in §1.4 is referenced by a passing test or the
      command named for it
- [ ] `grep -rn "Command::new" src/ | grep -v 'src/git.rs'` returns nothing
      (architecture.md §5 rule 4; grep narrowed per 000 D1)
- [ ] `cargo tree --depth 1` lists no dependency besides `ratatui`
      (architecture.md §5 rule 1)
- [ ] `grep -n "std::fs::write" src/main.rs` returns nothing outside
      `#[cfg(test)]`
- [ ] `architecture.md` updated if any name, table or rule changed

---

## 7. Deviations
Filled by the builder.

| # | Plan section | Plan said | Built instead |
| --- | --- | --- | --- |
| D1 | | | |

---

## 8. Drifts
Filled by the builder, one entry per deviation in §7.

```
D1
  Cause:
  Evidence:
  Why the plan missed it:
  Fix applied:
  Follow-up:
```
