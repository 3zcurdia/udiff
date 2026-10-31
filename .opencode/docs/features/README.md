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
