# Feature: Editing correctness (line model, CRLF, tabs, Home/End)

<!--
Split from PLAN.md items 2.1 ("One line model: cursor, trailing newline,
CRLF"), 2.2 ("Tabs") and 2.3 ("Home / End in Edit mode"). Built on branch
`complete-plan`.
-->

**Status:** done
**Author:** plan agent (split from `PLAN.md`), approved by luis
**Date:** 2026-10-31
**Architecture version:** commit 7dc3b40 of `.opencode/docs/architecture.md`

---

## 1. Product requirements (PRD)

### 1.1 Problem
Hand-editing a resolution corrupted files in three ways. The cursor started on
a phantom empty line after the final newline, so typed text had no line
terminator and was glued onto the next text line on save (`foo();between
line`). In CRLF files, edits split on `\n` only, so Enter produced lines
without `\r`, mixing line endings, and the cursor could sit after a `\r`. Tab
was ignored in Edit mode, so Makefile recipes could not be typed, and ratatui
draws `\t` as zero width, so the caret drifted on tab-indented lines. Home and
End did nothing while editing.

### 1.2 Users and roles
Single role: the developer at the terminal; no permission model. With this
feature they can type a resolution in any file — LF or CRLF, tab-indented or
not — and get exactly the bytes they typed. Outsider and unauthenticated
visitor: not applicable.

### 1.3 User stories
- As a developer, I want text I type at the end of a resolution to stay on its
  own line so that the next line of the file is not damaged.
- As a developer on a CRLF file, I want new lines to use CRLF so that I do not
  commit mixed line endings.
- As a developer resolving a Makefile, I want Tab to insert a real tab and the
  caret to line up so that the recipe works and I can see where I type.
- As a developer, I want Home and End to jump within the line so that editing
  long lines is quick.

### 1.4 Acceptance criteria
1. Given a resolution, when the user enters Edit mode, then the cursor sits at
   the end of its last real line (not on a phantom line after it).
2. Given a resolution that ends in a newline, when the user types a new line
   at its end and saves, then the following text line of the file is
   unchanged.
3. Given an empty resolution, when the user types `foo`, then the resolution
   is `foo` plus the conflict's line terminator.
4. Given a conflict in a CRLF file, when the user presses Enter and types,
   then every line of the resolution ends in `\r\n`, and the cursor never
   moves past the end of the visible text onto the `\r`.
5. Given CRLF lines, when Backspace joins two lines, then no stray `\r` is
   left in the middle.
6. Given any conflict, its line terminator is taken from its `<<<<<<<` line.
7. Given two conflicts, when the user edits one, then the other is unchanged.
8. Given Edit mode, when the user presses Tab, then a `\t` is inserted; in
   Normal mode Tab does nothing.
9. Given a line with tabs, when it is drawn, then each tab expands to the next
   multiple of 4 cells and the caret is drawn at the expanded column.
10. Given Edit mode, when the user presses Home or End, then the cursor moves
    to the start or end of the current line.
11. Given Edit mode, when the user types a marker line on its own line, then
    save is still rejected.

### 1.5 Out of scope
- A configurable tab width (fixed at 4).
- Converting line endings or tabs.
- Unicode grapheme-aware cursor movement (columns count `char`s; exact for
  ASCII).

### 1.6 Open questions resolved

| # | Question | Answer |
| --- | --- | --- |
| A1 | Which line ending does a conflict use? | The `<<<<<<<` line's: `\r\n` if it ends with one, else `\n`. Assumption (not asked); git writes markers with the file's own endings. |
| A2 | Tab width for drawing? | 4. Assumption (not asked); the text model keeps real tabs. |
| A3 | Cursor columns: chars or display cells? | Chars. Assumption (not asked); good enough outside ASCII. |

---

## 2. Engineering requirements (ERD)

### 2.1 Design considered
**Option A:** one line model in `src/app.rs` — `edit_lines` (strip one
trailing terminator, split on `\n`, strip `\r`) and its inverse `join_lines`
(join with the conflict's `line_ending`, add one trailing terminator) — used
by every edit, cursor move and `end_cursor`.
**Option B:** keep splitting on `\n` and patch each call site for `\r` and the
trailing newline separately.
**Chosen:** A. B is what produced three separate bugs. A keeps one invariant in
the pure core (architecture.md §5 rule 2). Tabs are expanded only when drawing
(`src/ui/mod.rs`), so the saved bytes are what was typed (§5 rule 6).

### 2.2 Domain changes

```
Conflict (modified)                              src/conflict.rs
  line_ending : &'static str, "\r\n" or "\n", from the <<<<<<< line

Direction (modified)                             src/app.rs
  + LineStart   cursor -> (line, 0)
  + LineEnd     cursor -> (line, width of line)
```

```
app::edit_lines(text: &str) -> Vec<String>                  (new, private)
  One trailing terminator removed, then split on \n with any \r stripped.
  "" -> [""].

app::join_lines(lines: &[String], line_ending: &str) -> String   (new, private)
  Inverse of edit_lines: joined with line_ending plus one trailing
  line_ending; [""] -> "" (the empty resolution).

app::end_cursor(text: &str) -> (usize, usize)               (modified, private)
  (last line index, char count of the last line) over edit_lines(text).

App.edit_current / App.move_cursor                          (modified, private)
  Split and join through edit_lines / join_lines.
```

### 2.3 Data and migrations
None — no database. The written file keeps each conflict's own line ending;
text outside conflicts is untouched.

### 2.4 Services, jobs and side effects
None.

### 2.5 Routes

```
src/ui/mod.rs   main TUI screen, Edit mode
  Tab  -> Action::InsertChar('\t')
  Home -> Action::MoveCursor(Direction::LineStart)
  End  -> Action::MoveCursor(Direction::LineEnd)
```

### 2.6 Authorization and ownership
None — OS file permissions only (architecture.md §8). No new file path.

### 2.7 HTTP boundary

```
Edit-mode key (printable, Enter, Backspace, Delete, Tab)
  edit_lines(current_text) -> edit -> join_lines(.., line_ending)
  -> resolved = true, dirty = true; never fails

Edit-mode arrows / Home / End
  move_cursor over edit_lines; never fails, clamps at line ends
```

### 2.8 Views and components

```
src/ui/mod.rs
  TAB_WIDTH = 4
  expand_tabs(text: &str) -> String
    Tabs replaced by spaces up to the next multiple of TAB_WIDTH.
  expanded_column(text: &str, column: usize) -> usize
    Chars of expand_tabs(text) before text's column-th char.
  draw_panel
    Every line drawn as expand_tabs(text); the caret line uses
    caret_line(&expand_tabs(text), expanded_column(text, cursor.1), style).
    A caret on a tab highlights its first space.
  panel_document
    Strips one trailing \r from every display line.
```

### 2.9 Client-side behavior
None.

### 2.10 External dependencies
None — no change to the `git` contract.

### 2.11 Configuration
None. `TAB_WIDTH` is a constant, not configuration.

---

## 3. Test data and fixtures
`src/app.rs` test module:

```rust
const CRLF_SIDES: &str =
    "<<<<<<< ours\r\nlocal line\r\n=======\r\nremote line\r\n>>>>>>> theirs\r\n";
const EMPTY_SIDES: &str = "<<<<<<< ours\n=======\n>>>>>>> theirs\n";
```

plus the existing `ONE_CONFLICT_BASIC`, `TWO_CONFLICTS_WITH_TEXT` and
`TYPED_MARKER_LINE`, and a helper `type_text(app, text)` that dispatches one
`InsertChar` per char. `src/conflict.rs` uses its existing `CRLF_SIDES` and
`ONE_CONFLICT_BASIC`.

---

## 4. Tests

```
src/app.rs
  starts_on_first_conflict                       [AC1]  (cursor now (3, 10))
  typing_keeps_following_text_on_its_own_line    [AC2]
  typing_into_empty_resolution_adds_terminator   [AC3]
  crlf_edits_keep_crlf                           [AC4]
  backspace_joins_without_stray_cr               [AC5]
  edit_changes_only_selected_conflict            [AC7]  (exact assert)
  typed_marker_line_is_rejected_on_save          [AC11] (InsertNewline first)
  home_end_move_within_line                      [AC10]

src/conflict.rs
  line_ending_comes_from_open_marker             [AC6]

src/ui/mod.rs
  tab_inserts_in_edit_mode                       [AC8]
  tab_is_ignored_in_normal_mode                  [AC8]
  expand_tabs_cases                              [AC9]
  expanded_column_cases                          [AC9]
  home_end_map_in_edit_mode                      [AC10]
```

Also proven by manual acceptance (`PLAN.md`): check 5 (CRLF file, type and
Enter → `od -c` shows only `\r\n`) and check 6 (Makefile, Tab → real tab,
caret aligned).

---

## 5. Build order

1. `line_ending`, `edit_lines`, `join_lines`, `end_cursor`, `\r` stripping in
   `panel_document`, and the 2.1 tests (commit `34d663c`)
   -> `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
2. Tab in Edit mode, `TAB_WIDTH`, `expand_tabs`, `expanded_column`, tab-aware
   drawing, and the ui test module (commit `d943761`) -> same command
3. `Direction::LineStart` / `LineEnd`, Home/End in Edit mode, and the 2.3
   tests (commit `f5299f6`) -> same command
4. `cargo fmt --check && cargo check --all-targets && cargo clippy --all-targets -- -D warnings && cargo test` -> all green
5. `architecture.md` §3 (`Conflict`, `Direction`) updated

---

## 6. Verification checklist

- [x] Every file in §2 exists at the exact path written there
- [x] Every test in §4 exists with the exact name and passes
- [x] `cargo clippy --all-targets -- -D warnings` reports no errors
- [x] `cargo check --all-targets` exits zero
- [x] Every acceptance criterion in §1.4 is referenced by a passing test or the
      command named for it
- [x] `grep -rn "Command::new" src/ | grep -v 'src/git.rs'` returns nothing
      (architecture.md §5 rule 4; grep narrowed per 000 D1)
- [x] `cargo tree --depth 1` lists no dependency besides `ratatui`
      (architecture.md §5 rule 1)
- [x] `architecture.md` updated if any name, table or rule changed

---

## 7. Deviations

| # | Plan section | Plan said | Built instead |
| --- | --- | --- | --- |
| D1 | PLAN.md 2.3 tests | One test: app.rs `home_end_move_within_line` | Also added ui `home_end_map_in_edit_mode`, which checks the Home/End key mapping. |
| D2 | PLAN.md 2.1 tests | `typing_into_empty_resolution_adds_terminator` uses an inline string | The string is a named fixture `EMPTY_SIDES`, and typing goes through a `type_text` helper shared by four tests. |

---

## 8. Drifts

```
D1
  Cause: plan-gap
  Evidence: PLAN.md 2.3 changes translate_key (Home/End in Edit mode) but its
    only test drives App directly, so the key mapping had no test.
  Why the plan missed it: it treated the translate_key change as too small to
    need its own test, unlike Tab in 2.2.
  Fix applied: home_end_map_in_edit_mode in src/ui/mod.rs tests.
  Follow-up: none.

D2
  Cause: convention-conflict
  Evidence: architecture.md §7 asks for fixtures named for their scenario;
    four 2.1 tests repeat the same InsertChar loop.
  Why the plan missed it: each test was specified on its own.
  Fix applied: EMPTY_SIDES constant and type_text helper in the src/app.rs
    test module.
  Follow-up: none.
```
