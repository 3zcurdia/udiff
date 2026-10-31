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

## Status

Early scaffold — `src/main.rs` is a placeholder.

## Build & run

Toolchain is pinned via [mise](https://mise.jdx.dev) (`mise.toml`):

```sh
mise install
cargo run
```
