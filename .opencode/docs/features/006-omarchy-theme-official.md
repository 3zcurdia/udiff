# Feature: Omarchy theme following, made official

<!--
Split from PLAN.md Phase 4 ("Omarchy theme: make it official") and the
Omarchy bullet of item 0. Built on branch `complete-plan`.
-->

**Status:** building
**Author:** plan agent (split from `PLAN.md`), approved by luis
**Date:** 2026-10-31
**Architecture version:** commit 7dc3b40 of `.opencode/docs/architecture.md`

> Waiting on one owner check: manual acceptance check 10 (live theme switch on
> an Omarchy desktop). Set `Status: done` after it passes.

---

## 1. Product requirements (PRD)

### 1.1 Problem
udiff already followed the active Omarchy theme (built in
`000-project-setup.md`), but `AGENTS.md` said "conflict resolution only, zero
dependencies" and `README.md` never mentioned themes. An agent following
`AGENTS.md` could reasonably delete the theme code as feature creep, and a user
had no way to learn where colors come from or how to turn the behavior off.

### 1.2 Users and roles
Single role: the developer at the terminal; no permission model. With this
feature they can read how udiff picks its colors and turn theme following
off. Outsider and unauthenticated visitor: not applicable.

### 1.3 User stories
- As a developer on Omarchy, I want udiff to match my desktop theme and follow
  switches live so that it looks like the rest of my tools.
- As a developer, I want to know how to force the built-in palette so that I
  can turn theme following off.
- As a maintainer, I want the theme feature marked as sanctioned so that no
  agent removes it, and as the only exception so that no one uses it as
  precedent.

### 1.4 Acceptance criteria
1. `README.md` has a `## Theme` section that states: with no Omarchy theme,
   udiff uses its built-in palette of named terminal colors; on Omarchy it
   reads `colors.toml` from `~/.local/state/omarchy/current/theme`, else
   `~/.config/omarchy/current/theme`, and follows switches live (checked once
   a second while idle, re-read only when the modification time changes);
   `UDIFF_OMARCHY_DIR=<dir>` overrides the location, and a directory without
   `colors.toml` forces the built-in palette.
2. `AGENTS.md` names Omarchy theme following (`src/ui/theme/omarchy.rs`) as
   the one sanctioned non-conflict feature and says not to treat it as
   precedent.
3. The theme code and the 1-second event poll are unchanged by this feature.
4. The existing theme unit tests pass.
5. Given an Omarchy desktop with udiff open, when the user switches the
   theme, then udiff's colors change within about a second; with
   `UDIFF_OMARCHY_DIR=/nonexistent`, udiff uses the built-in palette.

### 1.5 Out of scope
- Any theme source other than Omarchy.
- A udiff config file or a theme command-line flag.
- Changing `OmarchyWatch::check` or the 1-second poll interval.
- Adding new color roles.

### 1.6 Open questions resolved

| # | Question | Answer |
| --- | --- | --- |
| Q1 | Keep the Omarchy theme code, or remove it as out of scope? | Keep; it is a planned feature. Document it. Owner decision, 2026-10-31 ("it will be added keep it"). |

---

## 2. Engineering requirements (ERD)

### 2.1 Design considered
**Option A:** keep the code as built and make the docs match it: README
section plus an `AGENTS.md` exception that is explicitly not a precedent.
**Option B:** remove theme following to match `AGENTS.md`'s old "conflict
resolution only" wording.
**Chosen:** A, by owner decision Q1. architecture.md §5 rule 7 ("One look,
OS-followed") already describes the feature; only `AGENTS.md` and `README.md`
disagreed with the code.

### 2.2 Domain changes
None — no type or module changes. `Theme`, `Role` and `OmarchyWatch` are as
described in architecture.md §3 and §6.

### 2.3 Data and migrations
None — no database. The file read is Omarchy's `colors.toml`; its handling is
unchanged.

### 2.4 Services, jobs and side effects
None changed. `OmarchyWatch::check` (stat of `colors.toml` once per second
while idle, re-read on mtime change) stays as built in `000-project-setup.md`.

### 2.5 Routes
None — no CLI or screen change.

### 2.6 Authorization and ownership
None — OS file permissions only (architecture.md §8). Paths read, unchanged:
`$UDIFF_OMARCHY_DIR/colors.toml`,
`~/.local/state/omarchy/current/theme/colors.toml`,
`~/.config/omarchy/current/theme/colors.toml`.

### 2.7 HTTP boundary
None — no new action.

### 2.8 Views and components
None — rendering is unchanged.

### 2.9 Client-side behavior
None.

### 2.10 External dependencies

```
Omarchy theme directory
  contract: the active theme is a directory containing colors.toml with flat
    key = "#RRGGBB" entries (architecture.md §6).
  failures: missing or unreadable -> built-in palette; no error shown.
  owned by: src/ui/theme/omarchy.rs
```

### 2.11 Configuration

| Name | Default | When absent |
| --- | --- | --- |
| `UDIFF_OMARCHY_DIR` | unset (probe the two home paths) | the home paths are probed; set to a directory without `colors.toml` to force the built-in palette |

---

## 3. Test data and fixtures
None new. The existing theme tests create temp theme directories under
`std::env::temp_dir()` (`temp_theme_dir` in `src/ui/theme/omarchy.rs`).

---

## 4. Tests

```
src/ui/theme/mod.rs (existing, unchanged)
  default_theme_resolves_every_role                [AC4]
  map_palette_maps_roles_with_contrast_guard       [AC4]

src/ui/theme/omarchy.rs (existing, unchanged)
  parse_colors_toml_reads_named_keys               [AC4]
  parse_colors_toml_reads_legacy_color_keys        [AC4]
  theme_dir_prefers_env_override                   [AC4]
  check_returns_some_only_on_change                [AC4]
```

Proven by command:
- AC1: `grep -n -A12 "^## Theme" README.md`
- AC2: `grep -n "sanctioned non-conflict feature" AGENTS.md`
- AC3: `git diff main..complete-plan --stat -- src/ui/theme/` prints nothing,
  and `grep -n "event::poll(Duration::from_secs(1))" src/main.rs` matches.
- AC5: manual acceptance check 10 in `PLAN.md` — **pending, for the owner**
  (needs an Omarchy desktop).

---

## 5. Build order

1. `AGENTS.md` sanctioned-feature bullet (part of commit `e36dee9`, plan item
   0) -> `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
2. `README.md` `## Theme` section (commit `1de042b`) -> same command
3. `cargo fmt --check && cargo check --all-targets && cargo clippy --all-targets -- -D warnings && cargo test` -> all green
4. `architecture.md`: no change needed (§5 rule 7 and §6 already describe the
   feature)

---

## 6. Verification checklist

- [x] Every file in §2 exists at the exact path written there
- [x] Every test in §4 exists with the exact name and passes
- [x] `cargo clippy --all-targets -- -D warnings` reports no errors
- [x] `cargo check --all-targets` exits zero
- [ ] Every acceptance criterion in §1.4 is referenced by a passing test or the
      command named for it — AC5 waits on manual check 10
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
