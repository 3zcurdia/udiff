# Feature: Strict marker grammar and diff3 / zdiff3

<!--
Split from PLAN.md items 1.2 ("Strict marker grammar, one classifier") and
1.3 ("diff3 / zdiff3 conflict style"). Built on branch `complete-plan`.
-->

**Status:** done
**Author:** plan agent (split from `PLAN.md`), approved by luis
**Date:** 2026-10-31
**Architecture version:** commit 7dc3b40 of `.opencode/docs/architecture.md`

---

## 1. Product requirements (PRD)

### 1.1 Problem
The parser treated any line of 7 or more `=` as a separator and any run of 7
or more `<` or `>` as a marker. A Markdown or reStructuredText file with a
`==========` heading underline could not be opened at all, and typing such a
line into a resolution blocked saving. Developers who set
`merge.conflictStyle=diff3` or `zdiff3` (common, and recommended by git) were
worse off: the `|||||||` base marker and the base lines landed in the local
side, and from there were saved and staged into the repository.

### 1.2 Users and roles
Single role: the developer at the terminal; no permission model. With this
feature they can resolve conflicts in files that contain setext headings, and
use any of git's three conflict styles. Outsider and unauthenticated visitor:
not applicable.

### 1.3 User stories
- As a developer resolving a README conflict, I want `=====` underlines to be
  ordinary text so that the file opens and saves.
- As a developer using `diff3` or `zdiff3`, I want the base section left out
  of both panels and of the saved file so that I never commit it.
- As a developer, I want a malformed base marker reported with its line number
  so that I can fix the file by hand.

### 1.4 Acceptance criteria
1. Given a file with `==========` and `=======` underlines outside conflicts,
   when it is parsed, then those lines are part of the text runs and the file
   opens.
2. Given lines such as `<<<<<<<<<<`, `<<<<<<<x` and `>>>>>>>>` outside
   conflicts, when the file is parsed, then they are text.
3. Given a single line, `is_marker_line` is true exactly for git's marker
   shapes: 7 marker bytes followed by end of line or a space (`<`, `>`, `|`),
   or exactly `=======`.
4. Given a resolution containing a 10-character `==========` line, when the
   user saves, then save-time validation accepts it.
5. Given a resolution containing a real marker line, when the user saves, then
   validation still rejects it.
6. Given a `diff3` conflict, when it is parsed, then the local side is the
   lines before `|||||||`, the base lines are dropped, and the remote side is
   the lines after `=======`.
7. Given a `zdiff3` conflict, when it is parsed, then the same holds and the
   common lines git moved out of the block stay in the text runs.
8. Given a parsed `diff3` or `zdiff3` file, when it is rendered, then no line
   of the output is a marker.
9. Given a `|||||||` line after `=======`, when the file is parsed, then
   loading fails with `line 5: ||||||| outside the local side of a conflict`.

### 1.5 Out of scope
- Showing the base section (the layout is fixed: remote | local over current).
- A custom `conflict-marker-size` gitattribute (markers longer than 7).
- Nested conflict markers from recursive merges.
- Detecting conflict markers in binary files.

### 1.6 Open questions resolved

| # | Question | Answer |
| --- | --- | --- |
| A1 | What happens to the diff3 base lines? | Dropped; there is no base panel in the fixed layout (AGENTS.md). Assumption (not asked). |
| A2 | Is `conflict-marker-size` supported? | No; documented in README § Conflict styles. Assumption (not asked). |
| A3 | Which strings are the fixtures? | Real git 2.56 output generated in a scratch repo (base `a b c d e`), not hand-written. Assumption (not asked). |

---

## 2. Engineering requirements (ERD)

### 2.1 Design considered
**Option A:** one strict classifier (`classify` / `marker_label`) in
`src/conflict.rs` that matches git's exact marker shape, exposed as
`is_marker_line` and reused by save-time validation in `src/app.rs`.
**Option B:** keep the loose 7-or-more rule and special-case lines made only
of `=` outside conflicts.
**Chosen:** A. B still disagrees with git on `<<<<<<<x` and keeps two copies of
the rule (`app.rs` had its own `starts_with_marker`). architecture.md §4: any
decision about what a file *means* lives in `src/conflict.rs`.

For the base section, **Option A:** a `Section` state (`Local`, `Base`,
`Remote`) in the open conflict that drops base lines. **Option B:** keep base
lines and show them in a fourth panel. **Chosen:** A; B breaks the fixed
layout (AGENTS.md hard constraint).

### 2.2 Domain changes

```
Marker (modified, private)                       src/conflict.rs
  Open(&str) | Base | Separator | Close(&str)    (Base added)

Section (new, private)                           src/conflict.rs
  Local | Base | Remote
  Local -> Base on |||||||; Local or Base -> Remote on the first =======

OpenConflict (modified, private)                 src/conflict.rs
  section : Section                              (replaces seen_separator: bool)

ParseErrorKind (modified)                        src/conflict.rs
  + MisplacedBase    a ||||||| line in the Base or Remote section
```

```
conflict::is_marker_line(content: &str) -> bool
  True when a line (without its terminator) is a git conflict marker.

conflict::marker_label(content: &str, marker: u8) -> Option<&str>   (private)
  The trimmed label after exactly 7 marker bytes followed by end of line or a
  space; None otherwise. Replaces strip_marker.
```

Parse rules inside an open conflict:

| Line | Local | Base | Remote |
| --- | --- | --- | --- |
| ordinary | `local_text` | dropped | `remote_text` |
| `\|\|\|\|\|\|\|` | → Base | `MisplacedBase` | `MisplacedBase` |
| `=======` | → Remote | → Remote | remote content |
| `>>>>>>>` | `MissingSeparator` | `MissingSeparator` | close |
| `<<<<<<<` | `NestedOpen` | `NestedOpen` | `NestedOpen` |

Outside a conflict: `<<<<<<<` opens, `>>>>>>>` is `MissingOpen`, and
`=======` / `|||||||` lines are text.

Removed from `src/app.rs`: `MARKER_LEN` and `starts_with_marker`;
`validate_save` calls `crate::conflict::is_marker_line`.

### 2.3 Data and migrations
None — no database. The file format read is git's conflict-marker format in
the `merge`, `diff3` and `zdiff3` styles; the format written is unchanged.

### 2.4 Services, jobs and side effects
None — parsing is pure; no change to the write in `src/main.rs` or to
`src/git.rs`.

### 2.5 Routes

```
src/main.rs   udiff <file> | udiff <remote> <local> <base> <merged>
  Unchanged forms. Files that used to fail to load (setext underlines) now
  open; diff3/zdiff3 files open with the base dropped.
```

### 2.6 Authorization and ownership
None — OS file permissions only (architecture.md §8). No new file path.

### 2.7 HTTP boundary

```
startup load
  read file -> conflict::parse -> Ok: TUI opens
                               -> Err: "udiff: line N: <message>", exit 1

parse_error_message (src/main.rs), changed texts:
  MissingOpen   -> ">>>>>>> outside a conflict"
  MisplacedBase -> "||||||| outside the local side of a conflict"

s (save-time validation)
  any current_text line with is_marker_line -> status-line error, no write
```

### 2.8 Views and components
None — panels render the parsed sides as before; base lines never reach them.

### 2.9 Client-side behavior
None.

### 2.10 External dependencies

```
git conflict-marker output (git 2.56)
  contract: markers are exactly 7 bytes; <<<<<<<, ||||||| and >>>>>>> carry
    an optional label after one space; ======= stands alone. diff3 adds
    "||||||| <base label>" and the base lines before =======; zdiff3 also moves
    common leading/trailing lines out of the block.
  failures: a non-default conflict-marker-size produces longer markers that
    udiff treats as text; the file then has "no conflicts" or fails to parse.
  owned by: src/conflict.rs
```

### 2.11 Configuration
None. Works with every value of `merge.conflictStyle` (`merge`, `diff3`,
`zdiff3`) without configuration.

---

## 3. Test data and fixtures
Inline constants in the `src/conflict.rs` test module:

```rust
const SETEXT_OUTSIDE: &str = "Title\n==========\n\
     <<<<<<< ours\nlocal\n=======\nremote\n>>>>>>> theirs\n\
     Sub\n=======\n";
const LOOSE_MARKERS_ARE_TEXT: &str = "<<<<<<<<<<\n<<<<<<<x\n>>>>>>>>\n\
     <<<<<<< ours\nlocal\n=======\nremote\n>>>>>>> theirs\n";
// Real git 2.56 output: base `a b c d e`; ours `c d` → `Y1 shared`,
// theirs `c d` → `X1 shared`.
const DIFF3: &str = "a\nb\n<<<<<<< HEAD\nY1\nshared\n||||||| 00c6642\nc\nd\n\
     =======\nX1\nshared\n>>>>>>> feature\ne\n";
const ZDIFF3: &str = "a\nb\n<<<<<<< HEAD\nY1\n||||||| 00c6642\nc\nd\n\
     =======\nX1\n>>>>>>> feature\nshared\ne\n";
const MISPLACED_BASE: &str = "<<<<<<< ours\nlocal\n=======\nremote\n\
     ||||||| base\n>>>>>>> theirs\n";
```

`src/app.rs` tests use the existing `ONE_CONFLICT_BASIC`.

---

## 4. Tests

```
src/conflict.rs
  setext_underlines_outside_conflicts_are_text   [AC1]
  loose_marker_runs_are_text                     [AC2]
  is_marker_line_matches_git_shapes              [AC3]
  parses_diff3_dropping_base                     [AC6]
  parses_zdiff3_dropping_base                    [AC7]
  diff3_render_has_no_markers                    [AC8]
  rejects_misplaced_base_with_line               [AC9]

src/app.rs
  save_accepts_long_equals_line                  [AC4]
  typed_marker_line_is_rejected_on_save          [AC5]  (existing)
```

Also proven by manual acceptance (`PLAN.md`): check 3 (zdiff3 → no
`|||||||` or base lines in any panel or in the saved file) and check 4
(setext underlines inside and outside a conflict → opens and saves).

---

## 5. Build order

1. `Marker::Base`, `marker_label`, strict `classify`, `is_marker_line`, text
   handling of `=======` / `|||||||` outside conflicts, `MissingOpen` text,
   `validate_save` on `is_marker_line`, and the 1.2 tests (commit `403aee3`)
   -> `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
2. `Section`, `MisplacedBase`, base lines dropped, and the 1.3 tests (commit
   `bd2783f`) -> same command
3. `cargo fmt --check && cargo check --all-targets && cargo clippy --all-targets -- -D warnings && cargo test` -> all green
4. `architecture.md` §3 (marker grammar) updated

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
| D1 | PLAN.md 1.3 tests | Tests read the conflict from `regions[1]` | Added a test helper `conflict_at(file, index) -> &Conflict` in the `src/conflict.rs` test module that panics with the region's text when it is not a conflict. |

---

## 8. Drifts

```
D1
  Cause: plan-gap
  Evidence: parses_diff3_dropping_base and parses_zdiff3_dropping_base both
    need the Conflict inside regions[1]; the existing only_conflict helper
    accepts only single-region files.
  Why the plan missed it: the plan named the assertions but not how to reach
    a conflict that has text regions around it.
  Fix applied: conflict_at helper, test module only.
  Follow-up: none.
```
