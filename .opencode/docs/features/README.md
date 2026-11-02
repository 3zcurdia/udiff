# Features — index and lifecycle

Every feature of udiff is planned and documented here before code is written.
A feature doc is the contract between the planning session and the building
session: `plan-feature` writes it, `implement-feature` executes it verbatim.

Docs root: `.opencode/docs/`. Template every feature copies:
`.opencode/docs/feature-template.md`.

## Numbering rule

- The file name is `NNN-<kebab-name>.md`: three zero-padded digits, then a
  short kebab-case name (`001-merge-tool-entry.md`).
- **`000-project-setup.md` is always the project scaffold.** It is never
  deleted and never renumbered.
- Feature numbers start at **`001`** and count up: the next number is the
  highest existing number plus one. Numbers are never reused, even for a
  feature that was abandoned.
- **Docs are never deleted.** When a doc is replaced by a newer one, the old
  file keeps its place and gains a `Superseded by NNN-name.md` line as the
  first line of the file.

## Lifecycle

1. **`plan-feature <request>`** interviews the product side, then the
   engineering side, and writes `features/NNN-name.md` with `Status: draft`.
2. **Review.** The user reads the doc. Running `implement-feature NNN` implies
   approval (a human may also flip `Status: draft` to `Status: approved`
   explicitly first).
3. **`implement-feature NNN`** executes the doc top to bottom, tests first,
   verifying after every build-order step. While building it holds
   `Status: building`.
4. **Close-out.** The builder fills §7 Deviations and §8 Drifts, ticks every
   §6 box, updates `architecture.md` if names, tables or rules changed, and
   only then sets `Status: done`.
5. A doc with `Status: done` is never edited except to add a
   `Superseded by …` line or to fix `architecture.md` cross-references.

Status values: `draft` → `approved` → `building` → `done`.

## Index

| File | Status | Summary |
| --- | --- | --- |
| `000-project-setup.md` | draft | Project setup and v1 build spec: the ratatui scaffold, marker parser, session state machine, three-panel TUI, save-and-stage flow, git mergetool entry, and the OS-followed theme — the complete micro binary. |
| `001-safe-quit-and-exit-codes.md` | done | `q` exits 1 and asks twice on edits; exit codes 0/1/2/3; mergetool form leaves staging to git. |
| `002-strict-marker-grammar.md` | done | git's exact marker shape (setext underlines are text) and diff3/zdiff3 support with the base dropped. |
| `003-unresolved-save-gate.md` | done | Per-conflict `resolved` flag; save refused while any conflict is unresolved; `b` = local then remote. |
| `004-editing-correctness.md` | done | One line model for edits (trailing newline, CRLF), real tabs drawn at 4-column stops, Home/End in Edit mode. |
| `005-synchronized-scrolling.md` | done | Shared vertical and horizontal scroll for all three panels, clamped before every draw. |
| `006-omarchy-theme-official.md` | building | Omarchy theme following documented in README and sanctioned in AGENTS.md; waits on the owner's live-switch check. |
| `007-docs-help-cli-tests-ci.md` | done | AGENTS.md module map, full `--help` and README keymap, `tests/cli.rs`, GitHub Actions CI. |
| `008-review-bug-fixes.md` | done | 2026-11-02 review fixes: no u16 scroll limit (draw only visible lines), placeholder line for empty conflict sides, Omarchy theme removal reverts to built-in, atomic save. |
| `009-render-performance.md` | draft | 2026-11-02 review speed-ups (after 008): one borrowed panel build per frame, widest-line cache, redraw only on real actions/resize, O(1) conflict lookup, git branch lookup only for HEAD labels. |

Features 001–007 were split from `PLAN.md` (2026-10-31) after it was built on
branch `complete-plan`.
