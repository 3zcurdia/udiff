# Feature: Safe quit and exit-code contract

<!--
Split from PLAN.md items 1.1 ("q must not report success") and 1.5
("Exit-code contract and staging"). Built on branch `complete-plan`.
-->

**Status:** done
**Author:** plan agent (split from `PLAN.md`), approved by luis
**Date:** 2026-10-31
**Architecture version:** commit 7dc3b40 of `.opencode/docs/architecture.md`

---

## 1. Product requirements (PRD)

### 1.1 Problem
The README wires udiff into `git mergetool` with
`mergetool.udiff.trustExitCode true`, so git reads exit 0 as "resolved" and
stages `$MERGED`. Before this feature, `q` (quit without saving) exited 0:
git staged the file with its conflict markers still in it, and the developer
only found out when the build broke or the markers reached a commit. A second
problem: when the write succeeded but `git add` failed, udiff exited 1, the
same code as "nothing written", so a script could not tell the two apart. In
mergetool form udiff also ran its own `git add`, which git already does.

### 1.2 Users and roles
Single role: the developer at the terminal. There is no permission model. With
this feature they can quit without fear of staging a broken file, and scripts
(including git) can tell "written" from "not written" by exit code alone.
Outsider and unauthenticated visitor: not applicable — no network, no
accounts, only files the OS already lets the user write.

### 1.3 User stories
- As a developer in `git mergetool`, I want `q` to report failure so that git
  never stages a file I did not resolve.
- As a developer who typed edits, I want `q` to ask once more so that a stray
  key does not discard my work.
- As a script author, I want distinct exit codes for "written and staged",
  "not written", "usage error" and "written but not staged" so that I can react
  to each.

### 1.4 Acceptance criteria
1. Given a session with no edits, when the user presses `q`, then udiff exits 1
   and the file is unchanged.
2. Given a session with edits, when the user presses `q`, then the status line
   reads `unsaved changes — press q again to discard` and udiff keeps running;
   a second `q` exits 1 and the file is byte-identical to before.
3. Given a dirty session where `q` was pressed once, when the user presses any
   key that maps to an action other than `Quit` or `Ignored`, then the next
   `q` asks again instead of quitting.
4. Given the mergetool form (4 arguments), when the user saves, then udiff
   writes `$MERGED`, does **not** run `git add`, prints
   `udiff: saved <path>` and exits 0.
5. Given the file form (1 argument), when the user saves and `git add`
   succeeds, then udiff prints `udiff: saved and staged <path>` and exits 0.
6. Given the file form, when the write succeeds but `git add` fails, then the
   file holds the resolution, stderr carries `udiff: warning: <message>`, and
   udiff exits 3.
7. Given a path whose directory does not exist, when the user saves, then
   udiff reports `udiff: cannot write <path>: <os error>`, stages nothing and
   exits 1.
8. Given `git mergetool` with `trustExitCode true`, when the user quits
   without saving, then git prints `merge of <file> failed` and the file stays
   unstaged and unchanged.

### 1.5 Out of scope
- A confirmation dialog or popup (the status line carries the prompt).
- Saving automatically on quit.
- Handling `Ctrl-C` or other signals differently from today.
- Changing the usage-error code (2) or the `--help` / `--version` codes (0).
- Pushing, committing, or continuing the merge or rebase for the user.

### 1.6 Open questions resolved

| # | Question | Answer |
| --- | --- | --- |
| A1 | Which code does quit-without-saving use? | 1, shared with read/parse/write errors: to git both mean "not resolved". Assumption (not asked). |
| A2 | How does a dirty quit confirm? | Press `q` twice; any other action disarms. No dialog (keeps the fixed layout). Assumption (not asked). |
| A3 | Which code does "written, but `git add` failed" use? | 3, so it differs from both 0 and 1. Assumption (not asked). |
| A4 | Should udiff stage in mergetool form? | No. git stages `$MERGED` itself after exit 0 with `trustExitCode`. Assumption (not asked), checked against git 2.56 in manual check 1. |

---

## 2. Engineering requirements (ERD)

### 2.1 Design considered
**Option A:** `q` always exits 1; on a dirty session the pure core arms a
`quit_armed` flag and asks for a second `q`. Staging becomes a `stage: bool`
the shell passes per invocation form.
**Option B:** keep `q` at exit 0 and tell users to drop `trustExitCode`, so
git asks "Was the merge successful?" after every run.
**Chosen:** A. B puts the safety on user configuration and contradicts the
README wiring. A keeps the decision (ask again or quit) in `src/app.rs` and the
exit-code mapping in `src/main.rs` — architecture.md §5 rule 2 (pure core,
thin shell) and rule 9 (errors are status-line messages inside the TUI).

### 2.2 Domain changes

```
App (modified)                                   src/app.rs
  quit_armed : bool, starts false
  set by Action::Quit on a dirty session; cleared by every action except
  Quit and Ignored

Outcome (modified)                               src/app.rs
  Quit                       (was Quit { saved: bool }; the field had no reader)

Message (modified)                               src/app.rs
  Error(String)              (Info(String) removed: its only user, the
                              "saved and staged" note, moved to the shell)
```

```
App.dispatch(Action::Quit) -> Outcome
  Outcome::Quit when the session is clean or quit_armed is set; otherwise
  sets quit_armed, sets Message::Error("unsaved changes — press q again to
  discard") and returns Outcome::Continue.
```

### 2.3 Data and migrations
None — udiff has no database. The conflicted file is written in place, byte
for byte as `render_resolved` produces it; this feature does not change its
format.

### 2.4 Services, jobs and side effects

```
src/main.rs
  save(path: &Path, text: &str, stage: bool) -> Result<i32, String>
    Writes `text` to `path`; Err("udiff: cannot write <path>: <error>") when
    the write fails (nothing staged). When !stage prints "udiff: saved <path>"
    and returns Ok(0). When stage runs git::stage(path): Ok -> prints
    "udiff: saved and staged <path>", Ok(0); Err(message) -> eprints
    "udiff: warning: <message>", Ok(3). Retrying is safe: the write replaces
    the file with the same bytes and `git add` of an already-staged file is a
    no-op.

  open_and_run(path: &Path, stage: bool) -> Result<i32, String>
    Unchanged except: forwards `stage` to save, and maps Outcome::Quit to
    Ok(1).
```

`src/git.rs` is unchanged.

### 2.5 Routes

```
src/main.rs   udiff <file>
  open_and_run(path, stage = true)

src/main.rs   udiff <remote> <local> <base> <merged>
  open_and_run(merged, stage = false)  -- git mergetool stages after exit 0
```

Exit codes (final):

| Code | Meaning |
| --- | --- |
| 0 | resolved file written (and staged, in file form) |
| 1 | quit without saving, or could not read/parse/write |
| 2 | usage error |
| 3 | file form only: written, but `git add` failed |

### 2.6 Authorization and ownership
None — OS file permissions only (architecture.md §8). No new file path is
touched: udiff still writes only the path on its command line.

### 2.7 HTTP boundary

```
q (Normal mode)
  App::dispatch(Quit) -> clean or armed: Outcome::Quit -> exit 1, no write
                      -> dirty and not armed: status line message, Continue

s (Normal mode), after the save gates pass
  Outcome::SaveRequested { text } -> save(path, text, stage)
    -> success: exit 0 (stdout line per §2.4)
    -> write failure: stderr message, exit 1
    -> stage failure: stderr warning, exit 3
```

### 2.8 Views and components

```
src/ui/mod.rs  status_text
  Message::Info arm removed; Message::Error renders as before. The armed-quit
  prompt is an ordinary status-line error. No new widget, no empty state
  change, no error state change.
```

### 2.9 Client-side behavior
None.

### 2.10 External dependencies

```
git mergetool (git 2.56)
  contract: with mergetool.<tool>.trustExitCode true, exit 0 means resolved
    and git runs `git add $MERGED`; any other code makes git print
    "merge of <file> failed" and leave the file unstaged.
  failures: none new — udiff only changes which code it returns.
  owned by: src/main.rs (exit-code mapping)

git add -- <file>
  contract: unchanged (architecture.md §3 Git boundary); now called only in
    the file form.
  failures: not a repository / path outside the repo -> exit 3 with warning.
  owned by: src/git.rs
```

### 2.11 Configuration
None new. The user-side wiring (`mergetool.udiff.trustExitCode true`) is
unchanged and documented in `README.md` § git mergetool wiring.

---

## 3. Test data and fixtures
- `src/app.rs` tests: existing `ONE_CONFLICT_BASIC` and
  `TWO_CONFLICTS_WITH_TEXT`.
- `src/main.rs` tests: unique files under `std::env::temp_dir()`
  (`udiff-test-<pid>-<counter>`), removed after each test; the unwritable
  path is `temp_dir().join("udiff-missing-dir/x")`.

---

## 4. Tests

```
src/app.rs
  quit_without_save_produces_no_write      [AC2]
  clean_session_quits_on_first_q           [AC1]
  other_key_disarms_quit                   [AC3]

src/main.rs
  save_without_stage_writes_and_exits_0    [AC4]
  save_with_failed_stage_exits_3           [AC6]
  save_to_unwritable_path_errors           [AC7]

src/git.rs (existing, unchanged)
  stage_command_args_are_the_git_add_command   [AC5]
```

Proven by command (manual acceptance in `PLAN.md`, run through tmux):
- AC5 and AC8: manual check 1 (save → staged, `git diff --cached --check`
  clean) and manual check 2 (`q` → `merge of f.txt failed`).
- AC6: manual check 9 (`udiff f.txt` outside a repo → warning, exit 3).

---

## 5. Build order
Built as plan items 1.1 and 1.5; items 1.2–1.4 landed between them.

1. `quit_armed`, `Outcome::Quit`, `Ok(1)` on quit, and the three app tests
   (commit `c392cb1`) -> `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
2. `save(path, text, stage)`, `open_and_run(path, stage)`, `Message::Info`
   removed, and the three main tests (commit `1153991`) -> same command
3. `cargo fmt --check && cargo check --all-targets && cargo clippy --all-targets -- -D warnings && cargo test` -> all green
4. `architecture.md` §3 (Session, Git boundary) and §6 (status line, exit
   codes) updated

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

None — executed verbatim.

---

## 8. Drifts

None — no deviations.
