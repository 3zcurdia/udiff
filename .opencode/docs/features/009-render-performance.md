# Feature: Render and input performance

<!--
From the /code-review audit of commit 21312ae (2026-11-02), findings 2, 3, 4,
7, 8 and 9. Depends on 008-review-bug-fixes.md: build 008 first. Finding 2
(style only the visible lines) is already done by 008 B1; this doc removes
the rest of the per-frame whole-file work.
-->

**Status:** done
**Author:** plan agent (from the 2026-11-02 code review), approval pending (luis)
**Date:** 2026-11-02
**Architecture version:** commit 4fe21a7 of `.opencode/docs/architecture.md`

---

## 1. Product requirements (PRD)

### 1.1 Problem
udiff is sold on "instant" (architecture.md §1), but every key press does
work that grows with the size of the whole file, not with the screen:

- **P1:** each frame builds the three panel documents twice (once in
  `clamp_scroll`, once in `draw_panel`). Each build concatenates the whole
  file into a new `String`, splits it and copies every line into its own
  `String`. That is 6 full-file copies per key press.
- **P2:** `clamp_scroll` runs `expand_tabs` (an allocation) and
  `Line::width` on every line of every panel on every frame, just to find the
  widest line. That value changes only when a resolution's text changes.
- **P3:** every event redraws: unbound keys (`Action::Ignored`), and
  non-key events such as focus or mouse.
- **P4:** in Edit mode, every cursor move splits the whole resolution into
  owned `String`s and then throws them away.
- **P5:** finding the n-th conflict (`App::conflict_at`, `conflict_at_mut`,
  `ui::region_of_conflict`) scans the regions on every call, several times
  per frame. The Save arm counts unresolved conflicts twice.
- **P6:** startup always spawns 1–2 `git` processes for the branch label,
  even when no conflict has a `HEAD` or empty local label, so the result is
  never shown.

In a 100,000-line file (a lockfile or generated code) this shows up as input
lag on held arrow keys and as slower startup in large repositories.

### 1.2 Users and roles
Single role: the developer at the terminal; no permission model. Nothing new
is visible: the same screens, faster. Outsider and unauthenticated visitor:
not applicable.

### 1.3 User stories
- As a developer resolving a conflict in a 100,000-line file, I want held
  PgDn and typing to keep up with the key repeat so that udiff feels instant.
- As a developer in a large repository, I want udiff to open without extra
  git calls when it does not need them.

### 1.4 Acceptance criteria
1. Given one frame, then each panel document is built exactly once and is
   shared by `clamp_scroll` and `render`.
2. Given any file, then the lines each panel shows are identical to the
   lines before this feature (same text, same owning region, same
   `selected_start`), including the empty-side placeholder from 008 and a
   file with no trailing newline.
3. Given no change to any resolution text since the last frame, then the
   widest-line scan is not run again. Given a text change, then the next
   frame's horizontal clamp uses the new widest line.
4. Given a key that maps to `Action::Ignored`, then the app state does not
   change (the status message is kept too) and no redraw happens.
5. Given a non-key event, then only `Event::Resize` causes a redraw.
6. Given Edit mode, then every cursor movement gives the same result as
   before (the existing tests pass unchanged).
7. Given any session, then `App` finds the n-th conflict without scanning the
   regions, and the conflict-to-region mapping matches document order.
8. Given a file in which no conflict has an empty or `HEAD` local label, then
   no git subprocess runs at startup. Given at least one such label, then the
   branch is resolved as before.
9. Given the 100,000-line timing fixture, then the average frame time in a
   release build is at least 10× lower than the baseline recorded in build
   step 1, and both numbers are written in §7.
10. Given every existing test, then it passes, with only the signature
    changes listed in §4.

### 1.5 Out of scope
- A persistent edit buffer (keeping the resolution as `Vec<String>` during
  Edit mode). Typing stays O(conflict size) per key. Conflicts are usually
  small, and a buffer would make `current_text` and the screen two sources of
  truth. Plan it separately if a profile ever shows typing as the bottleneck.
- Caching the panel documents across frames. With borrowed lines (P1) one
  build per frame is a single pass with no per-line allocation, so a cache
  (and its invalidation bugs) is not worth it. Revisit only if the timing in
  AC9 misses its target.
- Any change to what is drawn, to keys, or to the theme check interval.
- Benchmark crates (criterion etc.): architecture.md §5 rule 1 forbids
  dev-dependencies. Timing uses an `#[ignore]` test (§4).

### 1.6 Open questions resolved

| # | Question | Answer |
| --- | --- | --- |
| A1 | Cache the widest line where? | A cheap `revision: u64` counter on `App`, bumped on every resolution change. `main` keeps `(revision, widest)` and asks `ui::widest_line` only when the revision changed. Widths are a UI concept (tabs, unicode width), so `App` must not compute them (architecture.md §4). Assumption (not asked). |
| A2 | Should an ignored key still clear the status message? | No. `Action::Ignored` returns before any state change. An error message then stays until the next real action, instead of disappearing on, for example, a stray F-key. That is what makes skipping the redraw correct. Assumption (not asked). |
| A3 | How do we prove the speed-up without a benchmark crate? | An `#[ignore]` test that draws 50 frames of a 100,000-line file on `TestBackend` and prints the average. Run it in release before and after, and record both numbers in §7. Assumption (not asked). |
| A4 | Where does the "does any label need git?" rule live? | `git::label_needs_branch(&str)`, the rule `branch_label_display` already applies (empty or `HEAD`). `main` asks it over the conflicts before calling `current_branch_label`. One rule, one place. Assumption (not asked). |

---

## 2. Engineering requirements (ERD)

### 2.1 Design considered

**Panel documents (P1, P2)**
**Option A:** a per-frame `ui::View<'a>` borrowing `&'a App`: three
`PanelDocument<'a>` whose `lines` are `&'a str` slices into the regions.
`main` builds it once inside the draw closure and passes it to
`clamp_scroll` and `render`. The widest line is cached in `main` by
`App::revision`.
**Option B:** a long-lived cache of owned panel documents in `main`,
invalidated by `dispatch`.
**Chosen:** A. It needs no invalidation (the view lives for one frame), it
makes no per-line allocation, and the pure core stays free of rendering
concerns (architecture.md §4, §5 rule 2). B saves only one linear pass and
adds a whole class of stale-cache bugs.

**Redraw gating (P3)**
**Option A:** `dispatch` returns whether anything changed
(`Outcome::Continue { changed: bool }`).
**Option B:** treat `Action::Ignored` as "no change" by contract (early
return in `dispatch`), and let a pure `step_for(event, mode)` in `main` skip
ignored keys and non-resize events.
**Chosen:** B. It is a smaller change to the `Outcome` contract, and every
other action can visibly change something (at least the status message), so
it really needs a redraw (architecture.md §5 rule 8).

### 2.2 Domain changes

```
App (modified)                                             src/app.rs
  + revision : u64, starts 0
      wrapping_add(1) on every change to any Conflict::current_text
      (replace_current, edit_current). Never changes for navigation,
      scroll, cursor moves or mode changes.
  + conflict_regions : Vec<usize> (private)
      region index of each conflict, in document order; built once in
      App::new, inside the struct literal (before the end_cursor call that
      uses selected_conflict). Invariant: regions never change shape after parse (only
      current_text and resolved mutate), so this never goes stale.

App.conflict_count(&self) -> usize                          (new, pub)
  conflict_regions.len().
App.selected_region(&self) -> usize                         (new, pub)
  conflict_regions[selected]; replaces ui::region_of_conflict.
App.conflict_at / conflict_at_mut (modified, private)
  Index regions[conflict_regions[index]]; the match on Region::Conflict
  stays and keeps its expect message.
App.dispatch (modified)
  First statement: if action == Action::Ignored { return Outcome::Continue }
  before the message is cleared. Save arm restructured so
  count_unresolved runs once:
    Action::Save => {
        let unresolved = count_unresolved(&self.file);
        if unresolved > 0 { ...same body...; return Outcome::Continue; }
        match self.validate_save() { ...unchanged... }
    }
  Every count_conflicts(&self.file) in app.rs becomes self.conflict_count().

app::edit_line_slices(text: &str) -> Vec<&str>              (new, private)
  The body of edit_lines without to_string: one trailing terminator
  removed, split on '\n', one trailing '\r' stripped per line. "" -> [""].
app::edit_lines (modified)
  edit_line_slices(text).into_iter().map(str::to_string).collect().
app::move_cursor, app::end_cursor (modified)
  Use edit_line_slices; no String allocated. clamp_cursor becomes generic
  over the line type (fn clamp_cursor<S: AsRef<str>>(.., lines: &[S])).
```

```
ui::View<'a> (new, pub)                                    src/ui/mod.rs
  panels : [PanelDocument<'a>; 3]   remote, local, current, in that order
View::new(app: &'a App) -> View<'a>                          (new, pub)
  Builds the three PanelDocuments once.

ui::PanelDocument<'a> (modified)
  lines        : Vec<&'a str>        (was Vec<String>)
  line_region  : Vec<Option<usize>>  unchanged
  selected_region, selected_start    unchanged
ui::panel_document(app: &'a App, panel) -> PanelDocument<'a> (modified)
  Per region, no concatenation:
    piece = the region's text for this panel;
    if Region::Conflict and piece is "" -> push "" (008 placeholder);
    else for raw in piece.split_inclusive('\n'):
      line = raw without one '\n', then without one '\r'; push it.
    Each pushed line gets line_region Some(region index).
  selected_start = number of lines pushed before app.selected_region().
  Why per-region splitting is equivalent: every region except the last ends
  with '\n' (text runs end where a marker line starts, and conflict sides and
  join_lines output are terminated), so no line ever spans two regions.

ui::widest_line(view: &View) -> usize                        (new, pub)
  Max display width over all lines of all three panels. Per line: when it
  contains '\t', Line::from(expand_tabs(line)).width(); otherwise
  Span::raw(line).width(), which borrows and does not allocate.

ui::clamp_scroll(app: &App, view: &View, widest: usize, area: Rect)
    -> (isize, usize)                                        (modified)
  Same algorithm. Uses view.panels instead of building documents, and the
  given widest instead of scanning.

ui::render(frame, app: &App, view: &View, scroll: (isize, usize),
           theme: &Theme)                                    (modified)
  draw_panels/draw_panel take &PanelDocument from the view and the given
  scroll instead of app.scroll. Drawing rules are unchanged (008 slicing
  included).

ui::region_of_conflict, ui::spans_start                      (deleted)
```

```
git::label_needs_branch(marker_label: &str) -> bool        (new, pub)  src/git.rs
  marker_label.trim() is empty or equals "HEAD".
  branch_label_display uses it in place of its inline condition.
```

```
main::Step (new, private)                                  src/main.rs
  enum Step { Dispatch(Action), Redraw, Skip }
main::step_for(event: &Event, mode: Mode) -> Step           (new)
  Event::Key(key): ui::translate_key(*key, mode); Ignored -> Skip,
                   anything else -> Dispatch(action).
  Event::Resize(..) -> Redraw.
  Any other event -> Skip.
```

### 2.3 Data and migrations
None — no database, no file format change.

### 2.4 Services, jobs and side effects

```
src/main.rs  open_and_run
  Startup:
    let needs_branch = file.regions.iter().any(|region| matches!(region,
        Region::Conflict(conflict) if git::label_needs_branch(&conflict.local_label)));
    let local_branch = if needs_branch { git::current_branch_label() } else { None };
  Draw (replaces the current closure):
    let mut clamped = app.scroll;
    terminal.draw(|frame| {
        let view = ui::View::new(&app);
        if widest_cache.0 != Some(app.revision) {
            widest_cache = (Some(app.revision), ui::widest_line(&view));
        }
        clamped = ui::clamp_scroll(&app, &view, widest_cache.1, frame.area());
        ui::render(frame, &app, &view, clamped, &theme);
    })?;
    app.scroll = clamped;
    widest_cache : (Option<u64>, usize), starts (None, 0).
  Event loop: match step_for(&event, app.mode) {
      Step::Skip => continue,
      Step::Redraw => redraw = true,
      Step::Dispatch(action) => match app.dispatch(action) { ...unchanged... },
  }
  Pure and idempotent: clamping an already clamped scroll returns it
  unchanged (feature 005), so storing it after the draw is the same as
  storing it before.
```

### 2.5 Routes

```
src/ui/mod.rs   main TUI screen
  No new keys or states. Unbound keys no longer clear the status message.
```

### 2.6 Authorization and ownership
None — OS file permissions only (architecture.md §8). No new file path.

### 2.7 HTTP boundary

```
key event
  step_for -> Skip (Ignored: no state change, no draw)
           / Dispatch(action) -> App::dispatch -> Continue -> redraw
resize event
  step_for -> Redraw
other events (focus, mouse, paste)
  step_for -> Skip
```

### 2.8 Views and components

```
src/ui/mod.rs
  View built once per frame; draw_panel reads its PanelDocument. No visual
  change. Empty and error states unchanged.
```

### 2.9 Client-side behavior
None.

### 2.10 External dependencies

```
git CLI
  contract: unchanged (git branch --show-current, then
            git rev-parse --short HEAD).
  change:   called only when some conflict's local label is empty or HEAD.
            Otherwise local_branch is None, which branch_label_display
            never reads for a non-HEAD label.
  owned by: src/git.rs
```

### 2.11 Configuration
None. The widest-line cache lives for one process and is invalidated by
`App::revision`.

---

## 3. Test data and fixtures

- `src/ui/mod.rs`:
  - existing `EMPTY_SIDES_MIDDLE` and `EMPTY_SIDES_AT_END` (from 008);
  - `const NO_TRAILING_NEWLINE: &str = "a\n<<<<<<< ours\nl\n=======\nr\n>>>>>>> theirs\nz";`
  - `const TWO_CONFLICTS_WITH_TEXT` (copy of the app.rs fixture; fixtures
    are module-private);
  - `const TABBED_LINE: &str = "<<<<<<< ours\n\tx\n=======\nr\n>>>>>>> theirs\n";`
  - `fn large_file(lines: usize) -> String`: `lines` text lines
    `"line {n} lorem ipsum dolor\n"`, with a conflict
    (`"<<<<<<< ours\nL\n=======\nR\n>>>>>>> theirs\n"`) inserted after
    every `lines / 3` lines (3 conflicts).
- `src/app.rs`: existing `ONE_CONFLICT_BASIC` and `TWO_CONFLICTS_WITH_TEXT`.
- `src/main.rs`: events built inline (`Event::Key(KeyEvent::from(..))`,
  `Event::Resize(80, 24)`, `Event::FocusGained`).

---

## 4. Tests

```
src/ui/mod.rs
  render_large_file_timing   #[ignore]                 [AC9]
    View/render 50 frames of large_file(100_000) on TestBackend 120x40,
    dispatching Scroll { down: PAGE_LINES } between frames; prints
    "avg frame: {x} µs". Not asserted (machine-dependent); the numbers go
    into §7.
  view_lines_match_expected                            [AC2]
    TWO_CONFLICTS_WITH_TEXT, remote panel: lines ==
    ["before line", "remote a", "between line", "remote b", "after line"],
    line_region == [0, 1, 2, 3, 4] (as Some), selected_start == 1
  view_handles_no_trailing_newline                     [AC2]
    NO_TRAILING_NEWLINE, current panel: last line == "z"; len == 4
  view_keeps_empty_side_placeholder                    [AC2]
    EMPTY_SIDES_AT_END: every panel's lines == ["before", ""]
  widest_line_expands_tabs                             [AC3]
    TABBED_LINE: widest_line == 5 ("    x")
  clamp_scroll_saturates                 (existing; new signature) [AC1]
  render_shows_conflict_past_line_65535  (008; new signature)      [AC2]

src/app.rs
  revision_bumps_on_text_change_only                   [AC3]
    NextConflict, Scroll, MoveCursor, EnterEdit leave revision 0;
    TakeLocal -> 1; InsertChar -> 2
  ignored_action_changes_nothing                       [AC4]
    set message via a refused Save, then dispatch(Ignored): message,
    quit_armed, scroll, selected, revision unchanged
  conflict_regions_match_document_order                [AC7]
    TWO_CONFLICTS_WITH_TEXT: conflict_count() == 2; selected_region() == 1;
    after NextConflict selected_region() == 3
  every existing move/edit test                        [AC6]
    (home_end_move_within_line, crlf_edits_keep_crlf, ... unchanged)

src/git.rs
  label_needs_branch_cases                             [AC8]
    "" -> true, "  " -> true, "HEAD" -> true, " HEAD " -> true,
    "ours" -> false, "feature/x" -> false, "head" -> false

src/main.rs
  step_for_cases                                       [AC4, AC5]
    unbound key in Normal mode (KeyCode::F(5)) -> Skip;
    'n' -> Dispatch(NextConflict); Resize -> Redraw; FocusGained -> Skip
```

AC1 is also proven by command:
`sed '/#\[cfg(test)\]/,$d' src/ui/mod.rs | grep -c "panel_document("` prints 2
(the definition and the one call in `View::new`; test calls are excluded).
AC10 is proven by `cargo test`.

---

## 5. Build order

Every verify command runs through mise: `mise exec -- <command>`.
Precondition: 008 is `done`.

1. **Baseline timing.** Add `large_file` and `render_large_file_timing`
   against the current API. Run
   `cargo test --release -- --ignored --nocapture render_large_file_timing`
   and write the average into §7 as "baseline".
   -> `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
   Commit: `009 1: large-file timing harness`
2. **P5, conflict index.** Add `conflict_regions`, `conflict_count`,
   `selected_region`; index `conflict_at`/`conflict_at_mut`; restructure the
   Save arm; replace `count_conflicts(&self.file)` in app.rs and
   `ui::status_text`; replace `region_of_conflict` in `panel_document` with
   `app.selected_region()` and delete it. Test:
   `conflict_regions_match_document_order`.
   -> same command
   Commit: `009 2: O(1) conflict lookup`
3. **P1, View with borrowed lines.** Add `View`, make `PanelDocument`
   borrow, rewrite `panel_document` per region, delete `spans_start`,
   change the `clamp_scroll` and `render` signatures (keep the widest scan
   inside `clamp_scroll` for this step), update `main`'s draw closure to
   build one `View`. Tests: `view_lines_match_expected`,
   `view_handles_no_trailing_newline`, `view_keeps_empty_side_placeholder`;
   update the call sites in existing tests.
   -> same command, plus
   `sed '/#\[cfg(test)\]/,$d' src/ui/mod.rs | grep -c "panel_document("` == 2
   Commit: `009 3: build panel documents once per frame, borrowed`
4. **P2, widest-line cache.** Add `App::revision` and its bumps, add
   `widest_line`, pass `widest` into `clamp_scroll`, add the cache in
   `main`. Tests: `revision_bumps_on_text_change_only`,
   `widest_line_expands_tabs`.
   -> same command
   Commit: `009 4: cache the widest line by text revision`
5. **P3, redraw gating.** Early return for `Ignored` in `dispatch`; add
   `Step` and `step_for`; use them in the event loop. Tests:
   `ignored_action_changes_nothing`, `step_for_cases`.
   -> same command
   Commit: `009 5: redraw only on real actions and resize`
6. **P4, cursor moves without allocation.** Add `edit_line_slices`; make
   `clamp_cursor` generic; switch `move_cursor` and `end_cursor`. No new
   test: the existing move and edit tests are the contract (AC6).
   -> same command
   Commit: `009 6: borrow lines for cursor moves`
7. **P6, lazy branch lookup.** Add `label_needs_branch`, use it in
   `branch_label_display` and in `open_and_run`. Test:
   `label_needs_branch_cases`.
   -> same command, plus
   `grep -n "current_branch_label()" src/main.rs` shows it guarded by
   `needs_branch`
   Commit: `009 7: resolve the branch only for HEAD labels`
8. **Timing after.** Re-run the step 1 command; write the "after" number in
   §7 and check AC9 (at least 10× lower). If it misses, stop and report;
   do not add caching on your own (§1.5).
9. `cargo fmt --check && cargo check --all-targets && cargo clippy --all-targets -- -D warnings && cargo test`
   -> all green
10. Update `architecture.md`:
    - §3 Session: `App` gains `revision`; `Action::Ignored` changes nothing.
    - §3 Git boundary and §5 rule 4: "read-only label resolution at
      startup, only when a conflict's local label is `HEAD` or empty".
    - §6 Rendering strategy: "redraw on key events that map to an action,
      on resize, plus a theme-change redraw".
    - Add a build note to §1; flip the 009 row in `features/README.md` to
      `done`.
    -> `grep -n "revision\|only when a conflict" .opencode/docs/architecture.md`

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
- [x] Baseline and after timings are recorded in §7, and after ≤ baseline / 10
      (62309 µs → 3504 µs = 17.8×)
- [x] `architecture.md` updated if any name, table or rule changed

---

## 7. Deviations
Filled by the builder. Record the timings here as well:

- Baseline (build step 1): `avg frame: 62309 µs` (release, 100,000 lines, 120×40)
- After (build step 8): `avg frame: 3504 µs` (repeat runs: 3299 µs, 3291 µs)
- Ratio: 62309 / 3504 = **17.8×** lower — AC9 needs at least 10×.

| # | Plan section | Plan said | Built instead |
| --- | --- | --- | --- |
| D1 | §2.2 `View::new` + §4's AC1 command | `View::new` builds the three `PanelDocument`s; the AC1 command prints 2 | Built with one `panel_document(` call site (`.map` over the three panels) and an elided lifetime on the definition (`fn panel_document(app: &App, …) -> PanelDocument<'_>`) — three separate calls printed 3 |
| D2 | §2.2 `draw_panel` | "draw_panels/draw_panel take `&PanelDocument` from the view and the given scroll" | `draw_panel` takes `view: &View`, reads `view.panels[panel.index()]` itself and formats its own title; the planned shape (frame, area, app, theme, panel, title, document, scroll) is 8 arguments and clippy's `too_many_arguments` fails at 8 under `-D warnings` |
| D3 | §4 `render_large_file_timing` | "View/render 50 frames … dispatching `Scroll { down: PAGE_LINES }` between frames" | Same, and the frame mirrors `main`: the widest line is measured once and reused while `App::revision` is unchanged, so the printed number is the app's real per-frame cost after P2 |
| D4 | §5 step 10 | "§3 Session: `App` gains `revision`; `Action::Ignored` changes nothing." | Also documented `conflict_regions` and the now-conditional `local_branch` in §3 — the file's own header says the code wins and the doc is fixed in the same change |
| D5 | §5 precondition "008 is `done`" | Build 009 on top of 008 | 008 was `done` but its 10 files were staged and uncommitted; per the owner's choice they were committed first (`008: review bug fixes (B1-B4)`, 3403682) and 009 started on a clean tree |

---

## 8. Drifts
Filled by the builder, one entry per deviation in §7.

```
D1
  Cause: plan-gap
  Evidence: `sed '/#\[cfg(test)\]/,$d' src/ui/mod.rs | grep -c "panel_document("`
    printed 3 on the first step-3 build (the definition plus three calls in
    `View::new`), against the 2 §4 requires.
  Why the plan missed it: §2.2 says `View::new` builds the three documents and
    §4 fixes the expected grep count separately; nothing checked that three
    panels mean three call sites.
  Fix applied: `View::new` builds them with
    `.map(|panel| panel_document(app, panel))` — one call site — and
    `panel_document` elides its lifetime so the definition itself matches
    `panel_document(`. The command prints 2.
  Follow-up: none.

D2
  Cause: convention-conflict
  Evidence: `cargo clippy --all-targets -- -D warnings` failed at
    `src/ui/mod.rs:278`: "this function has too many arguments (8/7)" for
    `draw_panel` shaped exactly as §2.2 listed it.
  Why the plan missed it: §2.2 enumerated `draw_panel`'s parameters without
    running them past the lint gate architecture §2 requires on every build.
  Fix applied: `draw_panel` takes `view: &View` instead of a document and
    derives both its `PanelDocument` and its title from `panel`, landing at 7
    arguments; `draw_panels` became three one-line calls.
  Follow-up: update architecture.md §2 — done in this run: the Lint and
    formatting line now names `clippy::too_many_arguments`' 7-argument cap as
    part of the contract.

D3
  Cause: plan-gap
  Evidence: §4 defined the harness as "View/render 50 frames … dispatching
    Scroll between frames" while step 4 moved the widest-line measurement
    behind `App::revision` in `main`; the plan never says which workload the
    harness measures.
  Why the plan missed it: the harness was written in step 1, before the cache
    existed, and step 8 re-ran it without re-deciding what it times.
  Fix applied: the harness mirrors `main`'s draw closure exactly (build view →
    reuse `widest_cache` while the revision is unchanged → clamp → render), so
    the baseline and the after number measure the same work.
  Follow-up: none.

D4
  Cause: plan-gap
  Evidence: step 10 named only `revision` and `Action::Ignored` for §3, but
    `App` also gained the private `conflict_regions` index and `local_branch`
    became conditional on the labels.
  Why the plan missed it: step 10 was written per headline change, while
    architecture.md's header requires the whole file to match the code.
  Fix applied: §3's `App` line now lists `conflict_regions` and `revision`,
    and `local_branch` says it resolves only when a conflict's local label
    needs it.
  Follow-up: none.

D5
  Cause: user-decision
  Evidence: `git status --short` at step 0 showed 10 staged, uncommitted files
    (008's build, 80 + 6 tests green on top of them); 009's §5 precondition is
    "008 is `done`".
  Why the plan missed it: the plan tracks feature-doc status, not the
    repository's commit state.
  Fix applied: the owner chose "Commit 008 first"; the staged work was
    committed as `008: review bug fixes (B1-B4)` (3403682) and 009 started on
    a clean tree.
  Follow-up: none.
```
