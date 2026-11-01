# udiff

A micro clone of opendiff / kdiff3 with a single purpose: **resolving merge conflicts**.

Nothing else. Not a general diff tool, not a merge driver, not an editor — just a
tiny terminal app that shows the three sides of a conflict and lets you pick or
edit the resolution.

## Layout

Three panels, FileMerge (opendiff) style:

```
| remote | local  |
|-----------------|
|     current     |
```

- **remote** and **local** are the two conflicting versions of the file.
- **current** is the merged result you edit and save.

Each panel shows the whole file; the selected conflict is highlighted, and only
its **current** text is editable — everything outside a conflict stays
byte-identical on save.

## Status

Implemented: `udiff` opens one conflicted file, steps through its conflicts,
takes remote/local/both or hand-edits the resolution, then writes the
marker-free file and stages it (`git add`) so the ongoing merge or rebase can
continue. Saving is refused until every conflict has been resolved (taken or
edited); unresolved conflicts are dimmed in the **current** panel and counted
on the status line. `git mergetool` works via the wiring below; branch labels (including
the detached-HEAD short SHA mid-rebase) come from the markers plus git.

## Build & run

Toolchain is pinned via [mise](https://mise.jdx.dev) (`mise.toml`):

```sh
mise install
cargo build --release
target/release/udiff path/to/conflicted-file
```

`target/release/udiff --help` prints the usage line and the keymap;
`--version` prints `udiff 0.1.0`.

### Keymap

```
Normal mode
  n / p        next / previous conflict (stops at the ends)
  r / l / b    take remote / local / both (both = local, then remote)
  Enter / i    enter Edit mode
  ↑ / ↓        scroll all panels one line
  PgUp / PgDn  scroll all panels ten lines
  ← / →        scroll all panels sideways
  Home         scroll back to the selected conflict
  s            save: refused while any conflict is unresolved; writes the
               file, stages it (udiff <file> form), exits 0
  q            quit without saving, exit 1 (press twice if there are edits)

Edit mode
  printable    insert at the cursor          Tab: insert a tab
  Enter        newline
  Backspace    delete before the cursor      Delete: delete at the cursor
  arrows       move the cursor               Home / End: line start / end
  Esc          back to Normal mode
```

### Exit codes

| Code | Meaning |
|---|---|
| 0 | resolved file written (and staged, in file form) |
| 1 | quit without saving, or could not read/parse/write |
| 2 | usage error |
| 3 | file form only: written, but `git add` failed |

### Conflict styles

`merge.conflictStyle` `merge`, `diff3` and `zdiff3` are all supported; the
base section of diff3/zdiff3 is not shown (the layout has no base panel). A
custom `conflict-marker-size` gitattribute is not supported.

### git mergetool wiring

One-time, run on your machine (udiff must be on `PATH`):

```sh
git config --global merge.tool udiff
git config --global mergetool.udiff.cmd 'udiff "$REMOTE" "$LOCAL" "$BASE" "$MERGED"'
git config --global mergetool.udiff.trustExitCode true
```

`trustExitCode` matters: without it git ignores udiff's exit code and would
continue after a failed save. In mergetool form udiff does not run `git add`
itself — git stages the file after udiff exits 0.

## Theme

- With no Omarchy theme present, udiff uses its built-in palette (named
  terminal colors only).
- On Omarchy, it reads `colors.toml` from
  `~/.local/state/omarchy/current/theme`, else
  `~/.config/omarchy/current/theme`, and follows theme switches live (checked
  once a second while idle; the file is re-read only when its modification
  time changes).
- `UDIFF_OMARCHY_DIR=<dir>` overrides the location. A directory without a
  `colors.toml` forces the built-in palette — that is how to turn it off.
