# Feature: Synchronized panel scrolling

<!--
Split from PLAN.md Phase 3 ("Scrolling — each panel shows the whole file").
Built on branch `complete-plan`.
-->

**Status:** done
**Author:** plan agent (split from `PLAN.md`), approved by luis
**Date:** 2026-10-31
**Architecture version:** commit 7dc3b40 of `.opencode/docs/architecture.md`

---

## 1. Product requirements (PRD)

### 1.1 Problem
Each panel was pinned to the start of the selected conflict. When a conflict
side was taller than its panel, the developer could not read the rest of it,
and lines wider than the panel were cut off with no way to see the end. They
had to leave udiff and open the file elsewhere to decide on a resolution,
which defeats the point of a merge tool.

### 1.2 Users and roles
Single role: the developer at the terminal; no permission model. With this
feature they can scroll all three panels together, vertically and sideways,
and jump back to the selected conflict. Outsider and unauthenticated visitor:
not applicable.

### 1.3 User stories
- As a developer, I want to scroll a long conflict side so that I can read all
  of it before choosing.
- As a developer, I want the three panels to scroll together with their
  conflict tops aligned so that I can compare the sides line by line.
- As a developer, I want to scroll sideways so that I can read long lines.
- As a developer typing a resolution, I want the current panel to follow the
  caret so that I always see where I type.

### 1.4 Acceptance criteria
1. Given Normal mode, when the user presses ↑/↓, PgUp/PgDn (10 lines), ←/→
   (8 columns) or Home, then all three panels scroll by that amount, or return
   to the selected conflict for Home.
2. Given a panel, its top line is its own conflict start minus 2 context
   lines, shifted by the shared scroll and clamped so it never scrolls past
   either end.
3. Given a position outside the visible window, `follow` moves the window
   just enough to show it.
4. Given any stored scroll, before every draw it is clamped to the range in
   which at least one panel still moves, and clamping twice gives the same
   result (no dead zone when holding a key at either end).
5. Given a scrolled view, when the user presses `n` or `p`, then the scroll
   resets to the new conflict.
6. Given any scroll action, the session's dirty flag and the conflicts'
   resolved flags do not change.
7. Given a 100-line conflict side in an 80×24 terminal, every line and the
   right edge of every line are reachable in all three panels.
8. Given Edit mode, the current panel follows the caret vertically and
   horizontally; the other two keep the shared scroll.

### 1.5 Out of scope
- Line wrapping (it breaks the line alignment between panels).
- Independent per-panel scrolling.
- Mouse scrolling (the mouse is disabled; architecture.md §6).
- Scrollbars or a position indicator.

### 1.6 Open questions resolved

| # | Question | Answer |
| --- | --- | --- |
| A1 | One scroll for all panels, or one per panel? | One shared `(down, right)`, with each panel anchored at its own conflict start (kdiff3-style synchronized scroll). Assumption (not asked). |
| A2 | Where does clamping happen, given `App` has no panel size? | In the draw closure in `src/main.rs`, through the pure `ui::clamp_scroll`, before every draw. Assumption (not asked). |
| A3 | Step sizes? | 1 line, 10 lines per page, 8 columns sideways. Assumption (not asked). |

---

## 2. Engineering requirements (ERD)

### 2.1 Design considered
**Option A:** `App` stores a raw `(isize, usize)` scroll and does only
saturating arithmetic; `ui::clamp_scroll(app, area)` clamps it right before
each draw, using the same `panel_areas` layout the renderer uses.
**Option B:** pass the terminal size into `App` and clamp inside `dispatch`.
**Chosen:** A. B puts terminal geometry into the pure core, which
architecture.md §4 forbids for `src/app.rs` ("no terminal"). Because every key
is followed by a draw, A never stores an out-of-range scroll.

### 2.2 Domain changes

```
App (modified)                                   src/app.rs
  scroll : (isize, usize), starts (0, 0)
           lines down from each panel's anchor, columns right
           reset to (0, 0) by NextConflict, PrevConflict and ScrollReset

Action (modified)                                src/app.rs
  + Scroll { down: isize, right: isize }
      scroll.0 saturating_add(down); scroll.1 saturating_add_signed(right)
  + ScrollReset
  Neither touches dirty or resolved.
```

```
ui::clamp_scroll(app: &App, area: Rect) -> (isize, usize)        (new, pub)
  app.scroll clamped to [min over panels of -base, max over panels of
  max_top - base] vertically (always including 0), and to
  widest_line - inner_width horizontally; inner size taken from the remote
  panel of panel_areas(area).

ui::panel_areas(area: Rect) -> [Rect; 3]                          (new)
  Remote, local and current areas below the status line; the single layout
  shared by render and clamp_scroll.

ui::panel_top(selected_start, down, len, rows) -> usize           (new)
  (selected_start - CONTEXT_LINES + down) clamped to [0, len - rows].

ui::follow(pos, start, size) -> usize                             (new)
  A view start that makes pos visible in size cells.
```

### 2.3 Data and migrations
None — no database, no file format change.

### 2.4 Services, jobs and side effects

```
src/main.rs  open_and_run, draw closure
  app.scroll = ui::clamp_scroll(&app, frame.area()); ui::render(frame, &app, &theme)
  Pure and idempotent: clamping an already clamped scroll returns it unchanged.
```

### 2.5 Routes

```
src/ui/mod.rs   main TUI screen, Normal mode
  ↑ / ↓        Scroll { down: ∓1, right: 0 }
  PgUp / PgDn  Scroll { down: ∓PAGE_LINES (10), right: 0 }
  ← / →        Scroll { down: 0, right: ∓SCROLL_COLUMNS (8) }
  Home         ScrollReset
Edit mode is unchanged: arrows move the cursor.
```

### 2.6 Authorization and ownership
None — OS file permissions only (architecture.md §8). No new file path.

### 2.7 HTTP boundary

```
scroll keys (Normal mode)
  App::dispatch(Scroll / ScrollReset) -> scroll updated -> Continue; never fails
next draw
  clamp_scroll -> render; never fails
```

### 2.8 Views and components

```
src/ui/mod.rs  draw_panel
  top = panel_top(selected_start, scroll.0, lines, rows); x = scroll.1.
  Edit mode, current panel only: top = follow(cursor_line, top, rows);
  x = follow(width of the expanded text before the caret, x, cols).
  Paragraph::scroll((top, x)) replaces the old pinned offset. No line
  wrapping. Empty and error states unchanged.
```

### 2.9 Client-side behavior
None.

### 2.10 External dependencies
None.

### 2.11 Configuration
None. `PAGE_LINES` and `SCROLL_COLUMNS` are constants.

---

## 3. Test data and fixtures
- `src/app.rs`: existing `TWO_CONFLICTS_WITH_TEXT`.
- `src/ui/mod.rs`: `clamp_scroll_saturates` builds its fixture in the test —
  one conflict whose remote side is 30 lines (`remote 0` … `remote 29`),
  opened with `App::new` over `Rect::new(0, 0, 80, 24)`.

---

## 4. Tests

```
src/ui/mod.rs
  arrows_scroll_in_normal_mode     [AC1]
  panel_top_cases                  [AC2]
  follow_cases                     [AC3, AC8]
  clamp_scroll_saturates           [AC4]

src/app.rs
  next_conflict_resets_scroll      [AC5]
  scroll_never_dirties             [AC6]
```

AC7 and AC8 are also proven by manual acceptance check 7 in `PLAN.md`
(100-line side at 80×24 reachable with ↑/↓/PgDn and → in all three panels).

---

## 5. Build order

1. `App.scroll`, `Action::Scroll` / `ScrollReset`, scroll reset on `n`/`p`,
   Normal-mode keys, `panel_areas`, `panel_top`, `follow`, `clamp_scroll`,
   scroll-aware `draw_panel`, the clamp in the draw closure, and the tests
   (commit `2f88766`)
   -> `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
2. `cargo fmt --check && cargo check --all-targets && cargo clippy --all-targets -- -D warnings && cargo test` -> all green
3. `architecture.md` §3 (`App`, `Action`) updated

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
| D1 | PLAN.md Phase 3 tests | No test of the Normal-mode key mapping | Added ui `arrows_scroll_in_normal_mode` (PgDn → `Scroll { down: PAGE_LINES, .. }`, Home → `ScrollReset`). |
| D2 | PLAN.md Phase 3, `clamp_scroll_saturates` | Assert the result is finite and idempotent | Also asserts the vertical value is between 0 and 1000, the horizontal value is 0 (every line fits), and `(-1000, 0)` clamps to 0. |

---

## 8. Drifts

```
D1
  Cause: plan-gap
  Evidence: PLAN.md Phase 3 adds seven Normal-mode keys to translate_key and
    lists no test for any of them.
  Why the plan missed it: the tests covered the scroll arithmetic, not the
    input mapping.
  Fix applied: arrows_scroll_in_normal_mode in src/ui/mod.rs tests.
  Follow-up: none.

D2
  Cause: plan-gap
  Evidence: "finite" is always true for an isize, so the plan's assertion
    could not fail.
  Why the plan missed it: it did not state the expected bounds.
  Fix applied: concrete bound assertions in clamp_scroll_saturates.
  Follow-up: none.
```
