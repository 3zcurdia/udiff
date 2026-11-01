# udiff — completion plan

## Status (2026-10-31)

All items 0 → 5.3 are done, one commit each on branch `complete-plan`. The
suite is 67 unit tests plus 6 CLI tests, with `fmt --check` and
`clippy -D warnings` clean.

Manual acceptance: checks 1–9 were run through tmux and pass. Check 10
(Omarchy live theme switch) is **pending, for the owner**, because it needs an
Omarchy desktop.

Notes from the run:
- **Xcode license:** on this machine, linking failed until Xcode's license is
  accepted (`sudo xcodebuild -license`). Workaround used:
  `DEVELOPER_DIR=/Library/Developer/CommandLineTools`.
- **Scripted key input:** when driving udiff from a script, pause after
  `Escape`. An Esc immediately followed by a key reaches the terminal as Alt+key,
  which udiff ignores. Humans don't type that fast.

Goal: make every promise in `README.md` true, and make it impossible for udiff
to stage a file that still contains conflict markers or unresolved code — while
staying micro (AGENTS.md: conflict resolution only, fixed three-panel layout).

This plan is written to be executed step by step by an agent with no other
context. Every code location is given by **symbol name** (function, type,
test), not line number, because line numbers move after the first commit.

---

## Rules for the implementing agent (read first, follow for every item)

1. **Setup once:** `mise install`. Then always run cargo through mise:
   `mise exec -- cargo …`.
2. **Baseline (2026-10-31, commit `7dc3b40`):** `cargo test` → 34 passed;
   `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings`
   clean. If your baseline differs, stop and report it before changing anything.
3. **Do the items strictly in order:** 0 → 1.1 → 1.2 → 1.3 → 1.4 → 1.5 → 2.1 → 2.2 → 2.3 → 3 → 4 → 5.
   Later items assume the earlier ones are done.
4. **Per-item loop:**
   1. Read every symbol the item names, in full.
   2. Write or update the tests listed for the item.
   3. Implement the change.
   4. Run, in this order, and fix until all three pass:
      ```sh
      mise exec -- cargo fmt
      mise exec -- cargo clippy --all-targets -- -D warnings
      mise exec -- cargo test
      ```
   5. Commit with exactly one commit per item. The message is
      `<item id>: <summary from the item heading>`, e.g.
      `1.1: q must not report success`.
5. **Stop and ask the owner (don't guess) if:**
   - a symbol named here doesn't exist or has a different shape;
   - a test fails that this plan doesn't tell you to change;
   - you would need a new dependency, a new panel, or a different layout;
   - a requirement here seems to contradict another one.
6. **Don't:**
   - add dependencies;
   - rename or rearrange the `remote | local / current` panels;
   - remove the Omarchy theme code;
   - change behavior that this plan doesn't mention;
   - mark a manual acceptance check as passed unless you actually ran it
     (see the end of this plan).

### Owner decisions (final — don't revisit)

| Question | Decision |
|---|---|
| `ratatui` dependency | **Keep.** It is the only dependency; add no others. |
| Omarchy theme following | **Keep** (planned feature). Document it (Phase 4). |
| Order of "both" (`b`) | **local, then remote** (git's ours-first). |

### Module map (for orientation)

```
src/main.rs        shell: argv, terminal lifecycle, event loop, file I/O, exit codes
src/conflict.rs    pure: marker grammar, parse, render_resolved
src/app.rs         pure: session state machine (selection, edits, save gate)
src/git.rs         the only subprocess boundary (branch label, git add)
src/ui/mod.rs      draw + key → Action translation
src/ui/theme/      color roles; omarchy.rs follows the OS theme
```

---

## Phase 0 — Doc truth

### 0: Update AGENTS.md
In `AGENTS.md`:
- **Hard constraints, scope bullet:** replace "The crate currently has **zero
  dependencies**; keep it that way unless a dependency is clearly unavoidable."
  with "The crate has exactly one dependency, `ratatui` (terminal I/O). Add no
  others."
- **Hard constraints:** add this bullet: "The one sanctioned non-conflict
  feature is Omarchy theme following (`src/ui/theme/omarchy.rs`). Keep it, and
  don't treat it as precedent for more."
- **Repo state section:** replace the whole section with the module map above,
  plus: "Tests live in `#[cfg(test)]` modules in `src/` and in `tests/cli.rs`.
  `PLAN.md` tracks remaining work."

No code changes, so there is nothing to test. Still run the per-item loop to
confirm everything is green.

---

## Phase 1 — Never stage a broken file (critical)

### 1.1: q must not report success
**Why:** in `main.rs` `open_and_run`, `Outcome::Quit { saved: _ } => return Ok(0)`.
With `mergetool.udiff.trustExitCode true`, git treats exit 0 as resolved and
stages the file with its markers still in it.

**Change:**
- In `app.rs`:
  - `Outcome::Quit { saved: bool }` → `Outcome::Quit` (no field).
  - Add the field `pub quit_armed: bool` to `App`, initialized to `false` in
    `App::new`.
  - In `App::dispatch`, after `self.message = None;`, insert:
    ```rust
    if action != Action::Quit && action != Action::Ignored {
        self.quit_armed = false;
    }
    ```
  - Replace the `Action::Quit` arm with:
    ```rust
    Action::Quit => {
        if !self.dirty || self.quit_armed {
            Outcome::Quit
        } else {
            self.quit_armed = true;
            self.message = Some(Message::Error(
                "unsaved changes — press q again to discard".to_string(),
            ));
            Outcome::Continue
        }
    }
    ```
- In `main.rs` `open_and_run`: `Outcome::Quit => return Ok(1),`.

**Tests (app.rs `tests`):**
- Update `quit_without_save_produces_no_write`: the session is dirty, so the
  first `Action::Quit` must return `Outcome::Continue` and the second must
  return `Outcome::Quit`. Keep the byte-identical file check.
- Add `clean_session_quits_on_first_q`: `app_over(ONE_CONFLICT_BASIC)`, then
  `dispatch(Quit)` == `Outcome::Quit`.
- Add `other_key_disarms_quit`: make the session dirty (EnterEdit,
  InsertChar('x'), ExitEdit), then Quit → Continue, NextConflict → Continue,
  Quit → Continue (armed again, not quit).

### 1.2: Strict marker grammar, one classifier
**Why:** `conflict.rs` `classify` treats any line of 7+ `=` as a separator, and
any 7+ `<`/`>` as a marker. So `==========` (Markdown/RST underline) makes a
file unopenable. `app.rs` `starts_with_marker` repeats the loose rule and
blocks saving such lines.

**Change in `conflict.rs`:**
- Add `Base` to `Marker` (it is used in 1.3; for now `parse` treats `Base`
  exactly like ordinary content, so behavior doesn't change yet):
  ```rust
  enum Marker<'a> { Open(&'a str), Base, Separator, Close(&'a str) }
  ```
- Replace `strip_marker` with:
  ```rust
  /// The label after exactly `MARKER_LEN` marker bytes followed by the end of
  /// the line or a space — git's own marker shape. `None` otherwise.
  fn marker_label(content: &str, marker: u8) -> Option<&str> {
      let bytes = content.as_bytes();
      if bytes.len() < MARKER_LEN || !bytes[..MARKER_LEN].iter().all(|&b| b == marker) {
          return None;
      }
      match bytes.get(MARKER_LEN) {
          None => Some(""),
          Some(b' ') => Some(content[MARKER_LEN + 1..].trim()),
          Some(_) => None,
      }
  }
  ```
- `classify` becomes:
  ```rust
  fn classify(content: &str) -> Option<Marker<'_>> {
      if let Some(label) = marker_label(content, b'<') { return Some(Marker::Open(label)); }
      if let Some(label) = marker_label(content, b'>') { return Some(Marker::Close(label)); }
      if marker_label(content, b'|').is_some() { return Some(Marker::Base); }
      if content == "=======" { return Some(Marker::Separator); }
      None
  }
  ```
- Add:
  ```rust
  /// True when a line (without its terminator) is a git conflict marker.
  pub fn is_marker_line(content: &str) -> bool {
      classify(content).is_some()
  }
  ```
- In `parse`, when `open` is `None`:
  - a `Separator` line is ordinary text: `text_run.push_str(raw_line)`;
  - a `Base` line is ordinary text too, for now and also after 1.3;
  - `Close` stays `MissingOpen`.
- In `main.rs` `parse_error_message`, change the `MissingOpen` text to
  `">>>>>>> outside a conflict"`.

**Change in `app.rs`:**
- In `validate_save`, replace `starts_with_marker(content)` with
  `crate::conflict::is_marker_line(content)`.
- Delete `app.rs`'s `MARKER_LEN` constant and the `starts_with_marker`
  function.

**Tests (conflict.rs `tests`)**, using these fixtures:
```rust
const SETEXT_OUTSIDE: &str = "Title\n==========\n\
     <<<<<<< ours\nlocal\n=======\nremote\n>>>>>>> theirs\n\
     Sub\n=======\n";
const LOOSE_MARKERS_ARE_TEXT: &str = "<<<<<<<<<<\n<<<<<<<x\n>>>>>>>>\n\
     <<<<<<< ours\nlocal\n=======\nremote\n>>>>>>> theirs\n";
```
- `setext_underlines_outside_conflicts_are_text`: `parse(SETEXT_OUTSIDE)` is
  Ok with 1 conflict. `regions[0] == Text("Title\n==========\n")` and
  `regions[2] == Text("Sub\n=======\n")`.
- `loose_marker_runs_are_text`: `parse(LOOSE_MARKERS_ARE_TEXT)` is Ok with 1
  conflict, and `regions[0] == Text("<<<<<<<<<<\n<<<<<<<x\n>>>>>>>>\n")`.
- `is_marker_line_matches_git_shapes`: true for `"<<<<<<< HEAD"`,
  `"<<<<<<<"`, `"======="`, `"||||||| base"`, `">>>>>>> feature/x"`. False
  for `"=========="`, `"<<<<<<<x"`, `"======= "`, `"<<<<<<"`, `""`.

**Tests (app.rs `tests`):**
- `save_accepts_long_equals_line`: on `ONE_CONFLICT_BASIC`, EnterEdit, type
  `==========` (10 `=`), then `validate_save()` is `Ok(())`.
- The existing `typed_marker_line_is_rejected_on_save` must still pass unchanged.

### 1.3: diff3 / zdiff3 conflict style
**Why:** with `merge.conflictStyle=diff3` or `zdiff3`, the `|||||||` base
marker and the base lines end up in `local_text`, and from there they are
saved and staged.

Real git 2.56 output (base commit `a b c d e`; ours changed `c d` → `Y1
shared`; theirs changed `c d` → `X1 shared`):
```rust
const DIFF3: &str = "a\nb\n<<<<<<< HEAD\nY1\nshared\n||||||| 00c6642\nc\nd\n\
     =======\nX1\nshared\n>>>>>>> feature\ne\n";
const ZDIFF3: &str = "a\nb\n<<<<<<< HEAD\nY1\n||||||| 00c6642\nc\nd\n\
     =======\nX1\n>>>>>>> feature\nshared\ne\n";
const MISPLACED_BASE: &str = "<<<<<<< ours\nlocal\n=======\nremote\n\
     ||||||| base\n>>>>>>> theirs\n";
```

**Change in `conflict.rs`:**
- Add `enum Section { Local, Base, Remote }`. In `OpenConflict`, replace
  `seen_separator: bool` with `section: Section`.
- Add the variant `ParseErrorKind::MisplacedBase`. In `main.rs`
  `parse_error_message`, map it to `"||||||| outside the local side of a
  conflict"`.
- Inside an open conflict:
  - **ordinary line:** goes to `local_text` (Local), is dropped (Base), or
    goes to `remote_text` (Remote);
  - **`Base` marker:** in Local → section = Base; in Base or Remote →
    `MisplacedBase`;
  - **`Separator`:** in Local or Base → section = Remote; in Remote → ordinary
    remote content (as today);
  - **`Close`:** in Local or Base → `MissingSeparator`; in Remote → finish the
    conflict (as today);
  - **`Open`:** `NestedOpen` (as today).

**Tests (conflict.rs `tests`):**
- `parses_diff3_dropping_base`: 3 regions; `regions[0] == Text("a\nb\n")`;
  the conflict has `local_text == "Y1\nshared\n"`, `remote_text ==
  "X1\nshared\n"`, `local_label == "HEAD"`, `remote_label == "feature"`;
  `regions[2] == Text("e\n")`.
- `parses_zdiff3_dropping_base`: `local_text == "Y1\n"`, `remote_text ==
  "X1\n"`, and `regions[2] == Text("shared\ne\n")`.
- `diff3_render_has_no_markers`: for both fixtures, no line of
  `render_resolved` satisfies `marker_like`.
- `rejects_misplaced_base_with_line`: `parse(MISPLACED_BASE)` → error at
  line 5, kind `MisplacedBase`.

### 1.4: Track unresolved conflicts; gate save on them
**Why:** every conflict starts pre-filled as remote+local, and nothing records
whether the user handled it. Pressing `s` right away saves and stages
duplicated code for every conflict.

**Change:**
- In `conflict.rs`:
  - Add the field `pub resolved: bool` to `Conflict`. `parse` sets it to `false`.
  - `parse` sets `current_text = local_text + remote_text`, i.e.
    `format!("{}{}", conflict.local_text, conflict.remote_text)`. Update the
    `current_text` doc comment to say "local then remote".
  - Add:
    ```rust
    /// Number of conflicts the user has not resolved yet.
    pub fn count_unresolved(file: &ConflictFile) -> usize
    ```
- In `app.rs`:
  - `Action::TakeBoth` → `format!("{local}{remote}")`.
  - `replace_current` and `edit_current` set `resolved = true` on the
    selected conflict. Cursor moves and `NextConflict`/`PrevConflict` don't.
  - At the start of the `Action::Save` arm, before `validate_save`:
    ```rust
    let unresolved = count_unresolved(&self.file);
    if unresolved > 0 {
        // index of the first conflict with !resolved
        self.selected = /* first unresolved index */;
        self.cursor = end_cursor(&self.selected_conflict().current_text);
        self.message = Some(Message::Error(format!(
            "{unresolved} of {} conflicts unresolved",
            count_conflicts(&self.file)
        )));
        return Outcome::Continue;
    }
    ```
    (Use an `if` that returns `Outcome::Continue`, written to fit the
    surrounding match arm.)
- In `ui/mod.rs`:
  - `status_text`: after `conflict i of N`, append ` · {n} unresolved` when
    `count_unresolved > 0`. Keep the rest of the line as it is.
  - `draw_panel`: for `Panel::Current` only, if the line's region
    (`document.line_region[index]`) is a `Region::Conflict` with `!resolved`,
    add `Modifier::DIM | Modifier::ITALIC` to that line's style.

**Tests — update these existing tests to the local-first order:**

| Test | New expectation |
|---|---|
| conflict.rs `parses_one_conflict_with_labels` | `current_text == "local one\nlocal two\nremote one\nremote two\n"`; `resolved == false` |
| conflict.rs `parses_crlf_sides` | `current_text == "local line\r\nremote line\r\n"`; `render_resolved == "local line\r\nremote line\r\n"` |
| conflict.rs `preserves_missing_trailing_newline` | `rendered == "local line\nremote line"` |
| conflict.rs `render_resolved_keeps_text_runs_byte_exact` | `"before line\nlocal a\nremote a\nbetween line\nlocal b\nremote b\nafter line\n"` |
| app.rs `starts_on_first_conflict` | `current_text == "local one\nlocal two\nremote one\nremote two\n"`; cursor stays `(4, 0)` until 2.1 |
| app.rs `take_both_concatenates_remote_then_local` | rename to `take_both_concatenates_local_then_remote`; expect `"local one\nlocal two\nremote one\nremote two\n"` |
| app.rs `save_text_equals_original_when_untouched` | rename to `save_after_resolving_all_writes_resolution`: TakeRemote, NextConflict, TakeLocal, Save → `SaveRequested` whose text is `"before line\nremote a\nbetween line\nlocal b\nafter line\n"`. Keep the temp-file round trip. |

**New tests (app.rs):**
- `save_refused_while_unresolved`: `TWO_CONFLICTS_WITH_TEXT`, NextConflict,
  TakeLocal (only conflict 2 resolved), then Save → `Continue`,
  `selected == 0`, and the message is `Error("1 of 2 conflicts unresolved")`.
- `edit_marks_resolved_navigation_does_not`: NextConflict and PrevConflict
  leave `count_unresolved == 2`; EnterEdit and InsertChar('x') → conflict 1
  resolved.

`typed_marker_line_is_rejected_on_save` still passes: typing resolves the
only conflict, and the marker check then rejects the save.

### 1.5: Exit-code contract and staging
**Why:** a `git add` failure after a successful write exits 1, the same code
as "nothing written". Also, under mergetool, git stages `$MERGED` itself, so
udiff's own `git add` there is redundant.

**Exit codes (final):**

| Code | Meaning |
|---|---|
| 0 | resolved file written (and staged, in file form) |
| 1 | quit without saving, or could not read/parse/write |
| 2 | usage error |
| 3 | file form only: written, but `git add` failed |

**Change in `main.rs`:**
- `open_and_run(path: &Path)` → `open_and_run(path: &Path, stage: bool)`.
  `Invocation::File` passes `true`; `Invocation::Mergetool` passes `false`.
- Replace `save` with:
  ```rust
  /// Writes the resolved text, then stages it when `stage`. The write always
  /// happens first; a staging failure is a warning and exit code 3.
  fn save(path: &Path, text: &str, stage: bool) -> Result<i32, String> {
      std::fs::write(path, text)
          .map_err(|error| format!("udiff: cannot write {}: {error}", path.display()))?;
      if !stage {
          println!("udiff: saved {}", path.display());
          return Ok(0);
      }
      match git::stage(path) {
          Ok(()) => {
              println!("udiff: saved and staged {}", path.display());
              Ok(0)
          }
          Err(message) => {
              eprintln!("udiff: warning: {message}");
              Ok(3)
          }
      }
  }
  ```
- In the `SaveRequested` arm, `return save(&app.path, &text, stage);`, and
  delete the `if let Some(Message::Info(note))` print.
- In `app.rs`, the `Save` arm stops setting the `"saved and staged"` Info
  message. If `Message::Info` then has no users, delete the variant and update
  `ui::status_text`'s match.

**Tests (main.rs `tests`):** each uses a unique file in
`std::env::temp_dir()` and removes it afterwards.
- `save_without_stage_writes_and_exits_0`: `save(path, "x\n", false)` ==
  `Ok(0)`, and the file contains `"x\n"`.
- `save_with_failed_stage_exits_3`: write to a temp-dir path (outside any
  repo, so `git add` fails); `save(path, "x\n", true)` == `Ok(3)`, and the
  file contains `"x\n"`.
- `save_to_unwritable_path_errors`: path
  `temp_dir().join("udiff-missing-dir/x")` → `Err`.

---

## Phase 2 — Editing correctness

### 2.1: One line model: cursor, trailing newline, CRLF
**Why:**
- `end_cursor` puts the cursor on the phantom empty line after the final
  `\n`. Typed text has no terminator and is glued onto the next text line
  (`foo();between line`).
- In CRLF files, edits split on `\n` only. Lines keep their `\r`, so Enter
  creates lines without `\r`, and the cursor can sit after the `\r`.

Real git output for a CRLF conflict (the markers are CRLF too):
`"a\r\n<<<<<<< HEAD\r\nOURS\r\n=======\r\nTHEIRS\r\n>>>>>>> feature\r\nc\r\n"`.

**Change:**
- In `conflict.rs`, add the field `pub line_ending: &'static str` to
  `Conflict`. `parse` sets it from the `<<<<<<<` line's raw text: `"\r\n"` if
  `raw_line.ends_with("\r\n")`, else `"\n"`.
- In `app.rs`, add two helpers and use them everywhere `current_text` is split
  or joined (`edit_current`, `move_cursor`, `end_cursor`):
  ```rust
  /// The editable lines of a resolution: one trailing terminator removed, then
  /// split on `\n` with any `\r` stripped. "" → [""].
  fn edit_lines(text: &str) -> Vec<String>

  /// Inverse of `edit_lines`: joined with `line_ending`, plus one trailing
  /// `line_ending` — except `[""]`, which is the empty resolution "".
  fn join_lines(lines: &[String], line_ending: &str) -> String
  ```
  - `edit_lines` body:
    `let body = text.strip_suffix('\n').map(|t| t.strip_suffix('\r').unwrap_or(t)).unwrap_or(text);`
    then `body.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l).to_string()).collect()`.
  - `end_cursor(text)` = `(lines.len() - 1, lines.last().chars().count())`
    over `edit_lines(text)`.
- In `ui/mod.rs` `panel_document`, strip one trailing `\r` from every display
  line.
- Known, accepted edge: an empty resolution in Edit mode draws its caret on
  the line after the conflict. A resolution that is exactly one empty line
  can't be produced by editing.

**Tests:**
- app.rs `starts_on_first_conflict`: cursor becomes `(3, 10)`, the end of
  `"remote two"`.
- app.rs `edit_changes_only_selected_conflict`: replace the `ends_with("X")`
  assert with `assert_eq!(edited, "local a\nremote aX\n")`.
- app.rs `typed_marker_line_is_rejected_on_save`: add
  `app.dispatch(Action::InsertNewline);` right after `EnterEdit`, so the
  marker is typed on its own line.
- New app.rs `typing_keeps_following_text_on_its_own_line`:
  `TWO_CONFLICTS_WITH_TEXT`, TakeLocal, EnterEdit, InsertNewline, type `foo`,
  then resolve conflict 2 with NextConflict and TakeLocal, then Save → text
  `"before line\nlocal a\nfoo\nbetween line\nlocal b\nafter line\n"`.
- New app.rs `typing_into_empty_resolution_adds_terminator`: on
  `"<<<<<<< ours\n=======\n>>>>>>> theirs\n"`, EnterEdit, type `foo` →
  `current_text == "foo\n"`.
- New app.rs `crlf_edits_keep_crlf`: on the conflict.rs `CRLF_SIDES` fixture,
  TakeLocal, EnterEdit, InsertNewline, type `x` → `current_text ==
  "local line\r\nx\r\n"`, and `MoveCursor(Right)` at the end of `"x"` doesn't
  move.
- New app.rs `backspace_joins_without_stray_cr`: on `CRLF_SIDES`, TakeBoth,
  EnterEdit, set `app.cursor = (1, 0)` directly (the field is `pub`), then
  Backspace → `current_text == "local lineremote line\r\n"`. (Copy
  `CRLF_SIDES` into the app.rs tests module; it is private to conflict.rs.)
- New conflict.rs `line_ending_comes_from_open_marker`: `CRLF_SIDES` →
  `"\r\n"`; `ONE_CONFLICT_BASIC` → `"\n"`.

### 2.2: Tabs
**Why:** Tab is ignored in Edit mode, and ratatui draws `\t` as zero width,
so the caret drifts on tab-indented lines (Makefiles, Go).

**Change in `ui/mod.rs`:**
- In `translate_key`, Edit mode: `KeyCode::Tab => Action::InsertChar('\t')`.
- Add:
  ```rust
  const TAB_WIDTH: usize = 4;

  /// Tabs replaced by spaces up to the next multiple of `TAB_WIDTH`.
  fn expand_tabs(text: &str) -> String

  /// Number of chars of `expand_tabs(text)` that come before `text`'s
  /// `column`-th char — where the caret sits in the expanded line.
  fn expanded_column(text: &str, column: usize) -> usize
  ```
  Count positions by chars. This is exact for ASCII and good enough
  elsewhere.
- In `draw_panel`, draw every line as `expand_tabs(text)`. For the caret line,
  call `caret_line(&expand_tabs(text), expanded_column(text, app.cursor.1),
  style)`. A caret on a tab highlights its first space.
- The text model keeps real `\t` characters. Only drawing expands them.

**Tests (ui/mod.rs: add a `#[cfg(test)] mod tests`):**
- `tab_inserts_in_edit_mode`: `translate_key(KeyEvent::from(KeyCode::Tab),
  Mode::Edit) == Action::InsertChar('\t')`.
- `tab_is_ignored_in_normal_mode`.
- `expand_tabs_cases`: `"\tx"` → `"    x"`, `"ab\tc"` → `"ab  c"`, `"abcd\t"`
  → `"abcd    "`, `""` → `""`.
- `expanded_column_cases`: `("\tx", 1)` → 4, `("ab\tc", 3)` → 4, `("abc", 2)`
  → 2.

### 2.3: Home / End in Edit mode
- Add `Direction::LineStart` and `Direction::LineEnd`. In `move_cursor`:
  LineStart → `(line, 0)`, LineEnd → `(line, width)`.
- In `translate_key`, Edit mode: `KeyCode::Home` and `KeyCode::End` map to
  them. (Normal-mode Home is added in Phase 3.)
- **Tests (app.rs):** `home_end_move_within_line`.

---

## Phase 3 — Scrolling ("each panel shows the whole file")

**Why:** each panel is pinned to the start of the selected conflict, so a
conflict side taller than the panel can't be read in full, and long lines are
cut off. Don't wrap lines: wrapping breaks the line alignment between panels.

**Design (final):**
- Scroll is one shared state for all three panels: `(down: isize, right: usize)`.
- Each panel's top line is `its own selected_start − CONTEXT_LINES + down`,
  clamped to that panel. Anchoring each panel at its own conflict start keeps
  the three conflict tops aligned (kdiff3-style synchronized scroll), even
  though the sides differ in length.
- `App` stays pure and only does saturating arithmetic. Clamping needs the
  panel size, so it happens in the draw closure in `main.rs`, right before
  every draw, through a pure `ui::clamp_scroll`. Because every key is followed
  by a draw, the stored scroll is never out of range, so there is no dead zone
  at the ends.

**Change in `app.rs`:**
- Add the field `pub scroll: (isize, usize)` to `App`, starting at `(0, 0)`.
- Add actions `Action::Scroll { down: isize, right: isize }` and
  `Action::ScrollReset`.
  - `Scroll` does `self.scroll.0 = self.scroll.0.saturating_add(down)` and
    `self.scroll.1 = self.scroll.1.saturating_add_signed(right)`.
  - `ScrollReset`, `NextConflict` and `PrevConflict` set `scroll = (0, 0)`.
  - None of these touch `dirty` or `resolved`.

**Change in `ui/mod.rs`:**
- Constants: `PAGE_LINES: isize = 10` and `SCROLL_COLUMNS: isize = 8`.
- `translate_key`, Normal mode:

  | Key | Action |
  |---|---|
  | `Up` / `Down` | `Scroll { down: ∓1, right: 0 }` |
  | `PageUp` / `PageDown` | `Scroll { down: ∓PAGE_LINES, right: 0 }` |
  | `Left` / `Right` | `Scroll { down: 0, right: ∓SCROLL_COLUMNS }` |
  | `Home` | `ScrollReset` |

  Edit mode is unchanged: arrows move the cursor.
- Extract the layout in `render`/`draw_panels` into
  `fn panel_areas(area: Rect) -> [Rect; 3]` (remote, local, current). Both
  `render` and the new functions use it, so the sizes always agree.
- Add:
  ```rust
  /// Top line of a panel: anchored at its conflict, shifted by `down`,
  /// clamped so the panel never scrolls past either end.
  fn panel_top(selected_start: usize, down: isize, len: usize, rows: usize) -> usize {
      let base = selected_start.saturating_sub(CONTEXT_LINES) as isize;
      let max_top = len.saturating_sub(rows) as isize;
      (base + down).clamp(0, max_top) as usize
  }

  /// Shifts a view start so that `pos` is visible in `size` cells.
  fn follow(pos: usize, start: usize, size: usize) -> usize {
      if size == 0 || pos < start { pos } else if pos >= start + size { pos + 1 - size } else { start }
  }

  /// `app.scroll` clamped to the range where at least one panel still moves.
  pub fn clamp_scroll(app: &App, area: Rect) -> (isize, usize)
  ```
- `clamp_scroll` algorithm. Take `rows`/`cols` as the inner size (minus the
  2 border cells) of the *remote* panel from `panel_areas`. Then, for each
  panel document:
  - `base = selected_start.saturating_sub(CONTEXT_LINES) as isize`
  - `max_top = len.saturating_sub(rows) as isize`
  - `lo = min(lo, -base)`
  - `hi = max(hi, max_top - base)`
  - `widest = max(widest, Line::from(expand_tabs(line)).width())` over its lines

  Return `(down.clamp(lo.min(0), hi.max(0)), right.min(widest.saturating_sub(cols)))`.
- `draw_panel`:
  - `top = panel_top(...)` and `x = app.scroll.1`.
  - In Edit mode, for the current panel only:
    - `top = follow(cursor_line, top, viewport)`;
    - `x = follow(caret_x, x, inner_width)`, where `caret_x =
      Line::from(expanded_text_before_caret).width()`.
  - Then `.scroll((top as u16, x as u16))`. Replace the old offset logic.

**Change in `main.rs`:** in the draw closure, before rendering:
```rust
.draw(|frame| {
    app.scroll = ui::clamp_scroll(&app, frame.area());
    ui::render(frame, &app, &theme)
})
```

**Tests:**
- ui `panel_top_cases`:

  | Call | Result |
  |---|---|
  | `panel_top(10, 0, 100, 20)` | 8 |
  | `panel_top(10, -50, 100, 20)` | 0 |
  | `panel_top(10, 500, 100, 20)` | 80 |
  | `panel_top(0, 0, 5, 20)` | 0 |

- ui `follow_cases`:

  | Call | Result |
  |---|---|
  | `follow(5, 0, 10)` | 0 |
  | `follow(12, 0, 10)` | 3 |
  | `follow(2, 5, 10)` | 2 |

- ui `clamp_scroll_saturates`: build an app from a fixture with a 30-line
  remote side. Set `scroll = (1000, 1000)`, call `clamp_scroll` with
  `Rect::new(0, 0, 80, 24)`, and assert the result is finite and that
  `clamp_scroll` on the result returns it unchanged (idempotent).
- app `next_conflict_resets_scroll`; app `scroll_never_dirties`.

---

## Phase 4 — Omarchy theme: make it official

No behavior change. The code works (its tests are in `omarchy.rs` and
`theme/mod.rs`); only the docs are missing.

- **README:** add a `## Theme` section with exactly these facts:
  - With no Omarchy theme present, udiff uses its built-in palette (named
    terminal colors only).
  - On Omarchy, it reads `colors.toml` from
    `~/.local/state/omarchy/current/theme`, else
    `~/.config/omarchy/current/theme`, and follows theme switches live
    (checked once a second while idle; the file is re-read only when its
    modification time changes).
  - `UDIFF_OMARCHY_DIR=<dir>` overrides the location. A directory without a
    `colors.toml` forces the built-in palette, which is how to turn it off.
- **Don't change** `OmarchyWatch::check` or the 1 s `event::poll` interval in
  `open_and_run`.

---

## Phase 5 — Docs, help, CLI tests, CI

### 5.1: README and `--help`
- **README** `### Keymap`: replace the block with exactly this:
  ```
  Normal mode
    n / p        next / previous conflict (stops at the ends)
    r / l / b    take remote / local / both (both = local, then remote)
    Enter / i    enter Edit mode
    ↑ / ↓        scroll all panels one line
    PgUp / PgDn  scroll all panels ten lines
    ← / →        scroll all panels sideways
    Home         scroll back to the selected conflict
    s            save: refused while any conflict is unresolved; writes the
                 file, stages it (udiff <file> form), exits 0
    q            quit without saving, exit 1 (press twice if there are edits)

  Edit mode
    printable    insert at the cursor          Tab: insert a tab
    Enter        newline
    Backspace    delete before the cursor      Delete: delete at the cursor
    arrows       move the cursor               Home / End: line start / end
    Esc          back to Normal mode
  ```
- **README:** add an `### Exit codes` section with the table from 1.5.
- **README:** add a `### Conflict styles` section:
  - `merge`, `diff3` and `zdiff3` are supported; base lines are not shown.
  - A custom `conflict-marker-size` gitattribute is not supported.
- **README `## Status`:** mention that unresolved conflicts block save.
- **README `### git mergetool wiring`:** add one sentence: in mergetool form,
  git stages the file itself after udiff exits 0.
- **`HELP` const in `main.rs`:**
  ```
  usage: udiff <file> | udiff <remote> <local> <base> <merged>

  resolve git merge conflicts in one file: remote | local over current

  <the keymap block above>

  exit codes
    0  saved   1  quit or error   2  usage   3  saved, but git add failed
  ```
- **Test (main.rs):** `help_lists_every_key`: `HELP` contains each of
  `"n / p"`, `"r / l / b"`, `"Enter / i"`, `"PgUp"`, `"Home"`, `"Tab"`,
  `"Esc"` and `"exit codes"`.

### 5.2: CLI integration tests
Create `tests/cli.rs`, with zero dependencies.
- Run the binary as `std::process::Command::new(env!("CARGO_BIN_EXE_udiff"))`
  with `.stdin(Stdio::null())`.
- Write fixtures to unique files under `std::env::temp_dir()`.
- **Only** test paths that exit before the terminal starts. Never run a
  valid conflicted file: it would open the TUI and hang the test run.

| Test | Args | Expect |
|---|---|---|
| `help_exits_0` | `--help` | 0, stdout contains `"exit codes"` |
| `version_exits_0` | `--version` | 0, stdout `"udiff 0.1.0\n"` |
| `bad_arity_exits_2` | `a b` | 2, stderr starts with `"usage:"` |
| `missing_file_exits_1` | `/nonexistent/x` | 1, stderr contains `"cannot read"` |
| `no_conflicts_exits_1` | file `"plain\n"` | 1, stderr contains `"no conflicts"` |
| `malformed_exits_1_with_line` | file `"<<<<<<< a\nx\n"` | 1, stderr contains `"line 1:"` |

### 5.3: CI
The repo is on GitHub (`3zcurdia/udiff`). Create `.github/workflows/ci.yml`:
```yaml
name: ci
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: jdx/mise-action@v2
      - run: cargo fmt --check
      - run: cargo clippy --all-targets -- -D warnings
      - run: cargo test
```
Don't push. Pushing is the owner's call.

### 5.4: Close out
- Update `AGENTS.md` if anything in its module map changed.
- Delete `PLAN.md` only if the owner asks. Otherwise mark each item done
  in it.

---

## Manual acceptance (TUI — needs a real terminal)

Agents: run these only if you can drive a terminal with tmux (recipe below).
Otherwise report them as **pending, for the owner**. Never report them as
passed without running them.

**Setup.** Run from the udiff repo root (the mergetool command uses `$OLDPWD`
to find the binary). Build the binary first:
`mise exec -- cargo build --release`. Then create the scratch repo:
```sh
R=$(mktemp -d) && cd "$R" && git init -q -b main
git config user.email t@t && git config user.name t
printf 'a\nb\nc\nd\ne\n' > f.txt && git add . && git commit -qm base
git checkout -qb feature && printf 'a\nb\nX1\nshared\ne\n' > f.txt && git commit -qam feat
git checkout -q main && printf 'a\nb\nY1\nshared\ne\n' > f.txt && git commit -qam main
git config merge.tool udiff
git config mergetool.udiff.cmd "$OLDPWD/target/release/udiff \"\$REMOTE\" \"\$LOCAL\" \"\$BASE\" \"\$MERGED\""
git config mergetool.udiff.trustExitCode true
git config mergetool.prompt false && git config mergetool.keepBackup false
git merge feature   # conflicts; add -c merge.conflictStyle=zdiff3 for check 3
```

**Driving with tmux:**
```sh
tmux new-session -d -s u -x 100 -y 30 "cd $R && git mergetool; echo EXIT=\$?; sleep 600"
tmux send-keys -t u s          # one key; use Enter, Escape, PageDown, etc. by name
tmux capture-pane -pt u        # read the screen
tmux kill-session -t u
```

**Checks:**
1. `s` right away is refused (status shows `unresolved`). Then `l`, `s` →
   staged; `git diff --cached --check` reports nothing.
2. `q` without saving → git prints `merge of f.txt failed`; the file is
   unstaged and unchanged.
3. With `-c merge.conflictStyle=zdiff3` → no `|||||||` or base lines `c`/`d`
   in any panel or in the saved file.
4. A README.md with `=====` underlines outside and inside a conflict → it
   opens and saves.
5. A CRLF file: type and press Enter → `od -c` shows only `\r\n`.
6. A Makefile conflict: type a tab-indented recipe → it's a real tab, and the
   caret is aligned.
7. A 100-line conflict side in an 80x24 terminal → every line is reachable
   with ↑/↓/PgDn in all three panels.
8. Mid-rebase (detached HEAD) → the local label shows the short SHA.
9. `udiff f.txt` on a copy outside the repo → written, warning, exit 3.
10. On Omarchy, switch the theme while udiff is open → the colors change
    within about a second. With `UDIFF_OMARCHY_DIR=/nonexistent` → built-in
    palette.
