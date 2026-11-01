# Feature: Unresolved-conflict save gate

<!--
Split from PLAN.md item 1.4 ("Track unresolved conflicts; gate save on
them"), including the owner decision on the order of "both". Built on branch
`complete-plan`.
-->

**Status:** done
**Author:** plan agent (split from `PLAN.md`), approved by luis
**Date:** 2026-10-31
**Architecture version:** commit 7dc3b40 of `.opencode/docs/architecture.md`

---

## 1. Product requirements (PRD)

### 1.1 Problem
Every conflict opens with its resolution pre-filled as both sides
concatenated, and nothing recorded whether the developer had looked at it.
Pressing `s` right after opening saved and staged duplicated code for every
conflict in the file — a broken file that git then accepted as resolved. The
cost showed up later as a failing build or a bad commit, far from the merge
that caused it.

### 1.2 Users and roles
Single role: the developer at the terminal; no permission model. With this
feature they can see how many conflicts are still open, and they cannot save
until each one is handled. Outsider and unauthenticated visitor: not
applicable.

### 1.3 User stories
- As a developer, I want save refused while any conflict is untouched so that
  I never stage a pre-filled guess.
- As a developer, I want the refusal to take me to the first open conflict so
  that I know where to go next.
- As a developer, I want the status line and the current panel to show which
  conflicts are still open so that I can see progress at a glance.
- As a developer, I want "both" to put my side first so that it matches git's
  ours-first convention.

### 1.4 Acceptance criteria
1. Given a freshly opened file, when it is parsed, then every conflict is
   unresolved and its resolution is the local side followed by the remote
   side.
2. Given a file with unresolved conflicts, when the user presses `s`, then
   nothing is written, the selection moves to the first unresolved conflict
   with the cursor at its end, and the status line reads
   `<n> of <N> conflicts unresolved`.
3. Given a conflict, when the user takes a side (`r`, `l`, `b`) or edits its
   resolution, then it becomes resolved; moving between conflicts (`n`, `p`)
   does not resolve anything.
4. Given a conflict, when the user presses `b`, then the resolution is the
   local side followed by the remote side.
5. Given a file where every conflict is resolved, when the user presses `s`,
   then the saved text holds each resolution and every text run byte-exact.
6. Given unresolved conflicts, when the screen is drawn, then the status line
   reads `conflict <i> of <N> · <n> unresolved · <remote> | <local>` and the
   unresolved conflicts' lines in the current panel are dim and italic.
7. Given CRLF sides or a file without a final newline, when an untouched
   conflict is rendered, then the local-first order holds and the line endings
   and missing final newline are preserved.

### 1.5 Out of scope
- An "accept all local" or "accept all remote" command.
- Marking a conflict resolved without changing it (a "keep as is" key). Press
  `b` to keep both sides.
- Undoing a resolution back to the unresolved state.
- Showing a per-conflict resolved marker outside the current panel.

### 1.6 Open questions resolved

| # | Question | Answer |
| --- | --- | --- |
| Q1 | In which order does "both" (`b`) join the sides? | Local, then remote (git's ours-first). Owner decision, 2026-10-31. |
| A1 | Does an edit that leaves the text unchanged (e.g. cursor moves) resolve the conflict? | No. Only text changes (`edit_current`) and side takes (`replace_current`) resolve. Assumption (not asked). |
| A2 | Where does a refused save leave the user? | On the first unresolved conflict, cursor at the end of its resolution. Assumption (not asked). |

---

## 2. Engineering requirements (ERD)

### 2.1 Design considered
**Option A:** a `resolved: bool` on each `Conflict`, false after parse, set
by the two `App` mutation paths; save refuses while
`count_unresolved > 0`.
**Option B:** compute "unresolved" by comparing `current_text` with its
initial pre-fill.
**Chosen:** A. With B, a conflict the user deliberately resolved as both sides
(`b`) would look untouched and block the save forever. A keeps the state in
the pure core (architecture.md §5 rule 2) and makes the gate a plain count.

### 2.2 Domain changes

```
Conflict (modified)                              src/conflict.rs
  resolved     : bool, false after parse; set true only by App
  current_text : String, now starts as local_text + remote_text
                 (was remote_text + local_text)
```

```
conflict::count_unresolved(file: &ConflictFile) -> usize
  Number of conflicts with resolved == false; 0 for a file with none.

App.replace_current(text)         (modified, private)    src/app.rs
  Also sets resolved = true on the selected conflict.

App.edit_current(..)              (modified, private)    src/app.rs
  Also sets resolved = true on the selected conflict.

App.first_unresolved() -> usize   (new, private)         src/app.rs
  Index of the first conflict with resolved == false, or 0.

App.dispatch(Action::Save) -> Outcome
  When count_unresolved > 0: selects first_unresolved(), moves the cursor to
  the end of its resolution, sets Message::Error("<n> of <N> conflicts
  unresolved"), returns Continue. Otherwise as before (marker validation,
  then SaveRequested).

App.dispatch(Action::TakeBoth)
  Resolution = local then remote.
```

### 2.3 Data and migrations
None — no database. The saved file's format is unchanged; only the default
resolution order differs, and an untouched file can no longer be saved.

### 2.4 Services, jobs and side effects
None — the gate is decided in `src/app.rs` before the shell's write.

### 2.5 Routes

```
src/ui/mod.rs   main TUI screen
  New state: "<n> unresolved" in the status line while n > 0.
```

### 2.6 Authorization and ownership
None — OS file permissions only (architecture.md §8). No new file path.

### 2.7 HTTP boundary

```
s (Normal mode)
  count_unresolved > 0 -> jump + status-line error, no write, Continue
  else validate_save   -> Err: status-line error / Ok: SaveRequested

r / l / b (Normal mode)
  replace_current -> resolved = true, dirty = true

printable / Enter / Backspace / Delete / Tab (Edit mode)
  edit_current -> resolved = true, dirty = true
```

### 2.8 Views and components

```
src/ui/mod.rs  status_text
  "conflict <i> of <N>" + (" · <n> unresolved" when n > 0)
  + " · <remote> | <local>" + (" · modified" when dirty) + (" · <message>").

src/ui/mod.rs  draw_panel
  Panel::Current only: lines whose region is a Conflict with !resolved get
  Modifier::DIM | Modifier::ITALIC on top of their style. Remote and local
  panels are unchanged. No empty state (an empty file never opens the TUI).
```

### 2.9 Client-side behavior
None.

### 2.10 External dependencies
None — no change to the `git` contract.

### 2.11 Configuration
None.

---

## 3. Test data and fixtures
Existing inline constants: `ONE_CONFLICT_BASIC`, `TWO_CONFLICTS_WITH_TEXT`
(`src/app.rs` and `src/conflict.rs`), `CRLF_SIDES` and
`NO_TRAILING_NEWLINE` (`src/conflict.rs`). The save round trip writes a
`udiff-test-<pid>-<counter>` file under `std::env::temp_dir()`.

---

## 4. Tests

```
src/conflict.rs
  parses_one_conflict_with_labels               [AC1]  (updated)
  parses_crlf_sides                             [AC7]  (updated)
  preserves_missing_trailing_newline            [AC7]  (updated)
  render_resolved_keeps_text_runs_byte_exact    [AC7]  (updated)

src/app.rs
  starts_on_first_conflict                      [AC1]  (updated)
  take_both_concatenates_local_then_remote      [AC4]  (renamed from
                                                       take_both_concatenates_remote_then_local)
  save_after_resolving_all_writes_resolution    [AC5]  (renamed from
                                                       save_text_equals_original_when_untouched)
  save_refused_while_unresolved                 [AC2]
  edit_marks_resolved_navigation_does_not       [AC3]
```

AC6 is proven by manual acceptance check 1 in `PLAN.md` (`s` right away is
refused and the status line shows `unresolved`); the dim/italic styling has no
unit test because rendering has no test harness in this project.

---

## 5. Build order

1. `resolved`, local-first `current_text`, `count_unresolved`, local-first
   `TakeBoth`, resolve-on-mutate, the save gate, the status-line count, the
   dim/italic styling, and the test updates (commit `6c0f4a0`)
   -> `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
2. `cargo fmt --check && cargo check --all-targets && cargo clippy --all-targets -- -D warnings && cargo test` -> all green
3. `architecture.md` §3 (`Conflict`) and §6 (status line) updated

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

None — executed verbatim. (The plan let the builder choose the shape of the
save gate "to fit the surrounding match arm"; it is a match guard,
`Action::Save if count_unresolved(&self.file) > 0`.)

---

## 8. Drifts

None — no deviations.
