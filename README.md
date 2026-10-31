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
continue. `git mergetool` works via the wiring below; branch labels (including
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
  r / l / b    take remote / local / both
  Enter / i    enter Edit mode
  s            save: write the file, stage it, exit 0
  q            quit without saving (the file stays untouched)

Edit mode
  printable    insert at the cursor
  Enter        newline
  Backspace    delete before the cursor    Delete: delete at the cursor
  arrows       move the cursor within the text
  Esc          back to Normal mode
```

### git mergetool wiring

One-time, run on your machine (udiff must be on `PATH`):

```sh
git config --global merge.tool udiff
git config --global mergetool.udiff.cmd 'udiff "$REMOTE" "$LOCAL" "$BASE" "$MERGED"'
git config --global mergetool.udiff.trustExitCode true
```

`trustExitCode` matters: without it git ignores udiff's exit code and would
continue after a failed save.
