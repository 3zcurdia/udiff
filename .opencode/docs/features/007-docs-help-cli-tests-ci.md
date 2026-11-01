# Feature: Docs, help, CLI tests and CI

<!--
Split from PLAN.md items 0 ("Update AGENTS.md"), 5.1 ("README and --help"),
5.2 ("CLI integration tests"), 5.3 ("CI") and 5.4 ("Close out"). Built on
branch `complete-plan`.
-->

**Status:** done
**Author:** plan agent (split from `PLAN.md`), approved by luis
**Date:** 2026-10-31
**Architecture version:** commit 7dc3b40 of `.opencode/docs/architecture.md`

---

## 1. Product requirements (PRD)

### 1.1 Problem
The README promised a keymap and behavior the binary did not have, `--help`
listed six keys, `AGENTS.md` still said "zero dependencies" and described
`src/main.rs` as a Hello-World placeholder, and nothing tested the binary end
to end or ran the checks on push. A user could not trust the docs, and an
agent reading `AGENTS.md` would plan against a codebase that no longer
existed.

### 1.2 Users and roles
Single role: the developer at the terminal (and the agents that work on the
repo); no permission model. With this feature they can rely on `README.md`,
`--help` and `AGENTS.md` matching the binary, and on CI catching
regressions. Outsider and unauthenticated visitor: not applicable.

### 1.3 User stories
- As a developer, I want `--help` to list every key and exit code so that I do
  not need the README in the middle of a merge.
- As a developer, I want the README to document exit codes and supported
  conflict styles so that I can wire udiff into scripts and git.
- As a maintainer, I want the binary's command-line paths tested and CI to run
  fmt, clippy and tests so that regressions are caught before merge.
- As an agent, I want `AGENTS.md` to describe the real module map so that my
  plan starts from the code as it is.

### 1.4 Acceptance criteria
1. `AGENTS.md` says the crate has exactly one dependency, `ratatui`, lists the
   module map (`main.rs`, `conflict.rs`, `app.rs`, `git.rs`, `ui/mod.rs`,
   `ui/theme/`), and says tests live in `src/` and `tests/cli.rs`.
2. `--help` contains every key group (`n / p`, `r / l / b`, `Enter / i`,
   `PgUp`, `Home`, `Tab`, `Esc`) and an `exit codes` section, and `README.md`
   § Keymap has the same block.
3. `README.md` has `### Exit codes` (the 0/1/2/3 table), `### Conflict
   styles` (merge, diff3, zdiff3; no custom `conflict-marker-size`), a Status
   line saying unresolved conflicts block save, and a mergetool sentence
   saying git stages the file itself.
4. `udiff --help` exits 0 and prints `exit codes`.
5. `udiff --version` exits 0 and prints `udiff 0.1.0`.
6. `udiff a b` exits 2 and stderr starts with `usage:`.
7. `udiff /nonexistent/x` exits 1 and stderr contains `cannot read`.
8. `udiff <file without markers>` exits 1 and stderr contains `no conflicts`.
9. `udiff <file "<<<<<<< a\nx\n">` exits 1 and stderr contains `line 1:`.
10. `.github/workflows/ci.yml` runs on push and pull request: checkout, mise,
    then `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and
    `cargo test`.
11. `PLAN.md` records the status of every item and the manual acceptance
    results.

### 1.5 Out of scope
- Pushing the branch and the first CI run on GitHub (the owner's call).
- A man page, shell completions, or release builds in CI.
- CLI tests that open the TUI (they would hang without a terminal).
- Caching in CI.

### 1.6 Open questions resolved

| # | Question | Answer |
| --- | --- | --- |
| Q1 | Keep `ratatui`, contradicting `AGENTS.md`'s "zero dependencies"? | Keep; it is the only dependency, add no others. Owner decision, 2026-10-31. |
| A1 | Where do end-to-end tests live, given they need the built binary? | `tests/cli.rs` (Cargo integration test), using `env!("CARGO_BIN_EXE_udiff")`, no dependencies. Assumption (not asked); see D1. |
| A2 | Which CI runner? | GitHub Actions on `ubuntu-latest` with `jdx/mise-action@v2`, so CI uses the `mise.toml` toolchain pin. Assumption (not asked). |

---

## 2. Engineering requirements (ERD)

### 2.1 Design considered
**Option A:** black-box CLI tests in `tests/cli.rs` that run the real binary,
limited to paths that exit before the terminal starts.
**Option B:** test only `parse_args` and the load path in-process from
`src/main.rs`.
**Chosen:** A for the exit codes and messages a user sees, alongside the
existing in-process `parse_args_*` tests. B cannot catch a wrong
`ExitCode` mapping in `finish`. A needs a test outside `src/`, which
architecture.md §5 rule 10 did not allow — recorded as D1, and the rule is
updated.

### 2.2 Domain changes
None — no type changes. `HELP` (const `&str` in `src/main.rs`) gains the
full keymap and the exit codes.

### 2.3 Data and migrations
None — no database.

### 2.4 Services, jobs and side effects
None.

### 2.5 Routes

```
src/main.rs   udiff --help | -h
  Prints HELP (usage, one-line summary, Normal/Edit keymap, exit codes), exit 0.
```

### 2.6 Authorization and ownership
None — OS file permissions only (architecture.md §8). CLI tests write their
fixtures under `std::env::temp_dir()` only.

### 2.7 HTTP boundary

```
argv (pre-TUI paths, all covered by tests/cli.rs)
  --help          -> stdout HELP, exit 0
  --version       -> stdout "udiff 0.1.0", exit 0
  wrong arity     -> stderr usage, exit 2
  unreadable file -> stderr "udiff: cannot read <path>: …", exit 1
  no markers      -> stderr "udiff: no conflicts in <path>", exit 1
  malformed       -> stderr "udiff: line N: …", exit 1
```

### 2.8 Views and components
None — the TUI is unchanged.

### 2.9 Client-side behavior
None.

### 2.10 External dependencies

```
GitHub Actions
  contract: .github/workflows/ci.yml, on push and pull_request, one job on
    ubuntu-latest: actions/checkout@v4, jdx/mise-action@v2 (installs the
    toolchain from mise.toml), then the three cargo commands.
  failures: any command non-zero fails the job; nothing is published.
  owned by: .github/workflows/ci.yml
```

### 2.11 Configuration
None new for udiff. CI reads `mise.toml` for the toolchain.

---

## 3. Test data and fixtures
`tests/cli.rs` helpers: `udiff(args) -> Output` (runs
`env!("CARGO_BIN_EXE_udiff")` with stdin set to null), `fixture(name,
contents) -> PathBuf` (a unique file under `std::env::temp_dir()`),
`stdout(&Output)` and `stderr(&Output)`. Fixture contents: `"plain\n"` and
`"<<<<<<< a\nx\n"`.

---

## 4. Tests

```
src/main.rs
  help_lists_every_key            [AC2]

tests/cli.rs
  help_exits_0                    [AC4]
  version_exits_0                 [AC5]
  bad_arity_exits_2               [AC6]
  missing_file_exits_1            [AC7]
  no_conflicts_exits_1            [AC8]
  malformed_exits_1_with_line     [AC9]
```

Proven by command:
- AC1: `grep -n "exactly one dependency\|tests/cli.rs" AGENTS.md`
- AC3: `grep -n "^### Exit codes\|^### Conflict styles\|unresolved\|stages the file" README.md`
- AC10: `cat .github/workflows/ci.yml`
- AC11: `sed -n 1,20p PLAN.md` (Status section)

---

## 5. Build order

1. `AGENTS.md` scope bullet and module map (commit `e36dee9`)
   -> `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
2. `README.md` keymap, exit codes, conflict styles, status, mergetool
   sentence; `HELP`; `help_lists_every_key` (commit `2a6034a`) -> same command
3. `tests/cli.rs` (commit `4f6454f`) -> same command
4. `.github/workflows/ci.yml` (commit `5b1449d`) -> same command
5. `PLAN.md` status and acceptance results (commit `eb5810a`) -> same command
6. `cargo fmt --check && cargo check --all-targets && cargo clippy --all-targets -- -D warnings && cargo test` -> all green
7. `architecture.md` §2, §4, §5 rule 10 and §7 updated for `tests/cli.rs`
   and CI

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
| D1 | PLAN.md 5.2 vs architecture.md §5 rule 10 | Create `tests/cli.rs`, running the binary with `std::process::Command` | Built as planned; architecture.md §4, §5 rule 10 and §7 say tests live only in `src/`, so those sections were updated to allow `tests/cli.rs`. |
| D2 | PLAN.md rule 4 (per-item loop) / architecture.md §2 commands | `mise exec -- cargo …` runs as written | On this machine linking failed with "You have not agreed to the Xcode license agreements". The commands were run with `DEVELOPER_DIR=/Library/Developer/CommandLineTools` set. |

---

## 8. Drifts

```
D1
  Cause: convention-conflict
  Evidence: architecture.md §5 rule 10 ("Tests live in #[cfg(test)] modules
    in src/"), §4 (unit tests row) and §7; PLAN.md 5.2 puts black-box tests
    in tests/cli.rs, which also runs Command::new outside src/git.rs.
  Why the plan missed it: PLAN.md was written against AGENTS.md, not
    architecture.md; item 0 updated AGENTS.md for tests/cli.rs but not
    architecture.md.
  Fix applied: architecture.md §2, §4, §5 rule 10 and §7 now name
    tests/cli.rs as the one place outside src/ for tests, limited to pre-TUI
    paths; rule 4's exec boundary applies to product code in src/.
  Follow-up: none.

D2
  Cause: infeasible
  Evidence: link step of every cargo build mid-session: "You have not agreed
    to the Xcode license agreements" (macOS, after an Xcode update).
  Why the plan missed it: environment change on the build machine, not a
    repo issue; the baseline build passed earlier in the same session.
  Fix applied: DEVELOPER_DIR=/Library/Developer/CommandLineTools for every
    cargo run.
  Follow-up: the owner runs `sudo xcodebuild -license` once; no doc change.
```
