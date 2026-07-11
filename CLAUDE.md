# CLAUDE.md

`cless` is a `less`-like terminal pager with tree-sitter syntax highlighting. Single Rust binary (Cargo, edition 2024). Release build is ~10 MB and requires a C compiler at build time (each tree-sitter parser is written in C).

End users install via `cargo binstall cless`, which pulls a prebuilt binary from GitHub Releases. `.github/workflows/release.yml` builds and publishes binaries for five targets on every `v*` tag push.

## Project layout

```
src/
  main.rs        Entry point. Arg parsing → highlight → pager.
  highlight.rs   Language detection + tree-sitter; builds Vec<Line>.
  pager.rs       Terminal control (crossterm), input loop, search, rendering.
.github/workflows/release.yml  Builds and publishes prebuilt binaries on tag push.
```

## Key commands

```sh
cargo build                                       # debug
cargo build --release                             # first run ~15s (15 C parsers to compile)
cargo test                                        # pager::tests (render SGR, wrap_ranges, clamp_pos, gutter, marks, files, follow, build_view, visible accessors, filtered info, parse_plus)
./target/release/cless <file>...                  # wraps long lines; -S to chop, -N for line numbers
./target/release/cless +/pat -p pat +G +N <file>  # startup positioning (search / line / end)
cat file | ./target/release/cless                 # stdin (keys still come from /dev/tty)
```

## Release procedure

```sh
# 1. Bump version in Cargo.toml
# 2. Commit and tag
git tag v0.1.0
git push origin v0.1.0
# 3. GitHub Actions builds binaries for 5 targets and uploads them to Releases
# 4. cargo binstall cless picks them up automatically
```

## Version control

This repo is managed with **jj (Jujutsu)** in a **colocated** layout (`.git` and `.jj` coexist; `.jj` is git-ignored). GitButler is no longer used. Standard git tooling (`cargo`, CI, `git tag`) still works unchanged.

Standard daily loop (trunk-only: `main` bookmark + `v*` tags, no feature branches):

```sh
jj new main                       # start work on top of main
# ...edit; jj auto-snapshots the working copy (no add/stage)...
jj st / jj diff                   # inspect the working-copy commit (@)
jj describe -m "feat(pager): …"   # set the commit message
jj new                            # finish; the completed change is now @-
jj bookmark set main -r @-        # advance main to the completed change
jj git push                       # push to origin (jj uses bookmarks, not branches)
```

Always finish with `jj new` *before* advancing main: commits reachable from `main@origin` are immutable, so if `main` is set to `@` and pushed, the working-copy commit itself becomes immutable and the next edit errors out. Keeping `@` as an empty scratch change on top avoids this.

```sh
jj git fetch                      # local main auto-updates (main@origin is tracked)
jj undo                           # undo the last operation (jj op log for history)
```

Releases still use git tags directly: `git tag v0.1.0 && git push origin v0.1.0` (jj does not create tags).

## Architecture notes

### `highlight.rs`

- `detect_language(path, content)` — extension → special filenames (`.bashrc` etc.) → shebang (`#!/usr/bin/env python` etc.)
- `build_config(lang)` — pulls `LANGUAGE` / `HIGHLIGHTS_QUERY` / `INJECTIONS_QUERY` from each language crate, builds a `HighlightConfiguration`, calls `.configure()` with the 26 entries in `HIGHLIGHT_NAMES`
- `highlight_file(content, path)` — walks the `Highlighter::highlight` event stream with a scope stack, splits spans on newlines, returns `Vec<Line>`
- Palette is citruszest ([zootedb0t/citruszest.nvim](https://github.com/zootedb0t/citruszest.nvim)). Mapping lives in the `FG_*` constants and `color_for(name)`.

### `pager.rs`

- `Pager` struct fields: `sources`, `index`, `name`, `lines`, `top`, `sub`, `left`, `wrap`, `numbers`, `following`, `follow_len`, `count`, `pending`, `marks`, `prev_pos`, `filters`, `view`, `start`, `mode`, `search`, `message`, `cols`, `rows`
- State machine: `Mode::Normal | SearchInput | FilterInput | Help`
- Multi-key prefixes use one `Pending` enum (`None`/`Z`/`Dash`/`Mark`/`Quote`/`Colon`) so only one can be live at once; the follow-up key is resolved at the top of `handle_normal`.
- Full redraw every frame (no diffing). Each row is cleared with `Clear(ClearType::CurrentLine)` before writing.
- `render_line` emits a complete `\x1b[0;7;38;2;R;G;Bm`-form SGR per chunk so older terminals render inverse video reliably.
- Search uses the `regex` crate with smart-case (`case_insensitive` when the pattern is all lowercase).
- **Line filter (`&`, `filters`/`view` fields)** — `&pat` shows only matching lines, `&!pat` excludes, empty `&` clears, multiple `&` combine (AND); smart-case like search. `filters: Vec<Filter>` is the source of truth; `view: Option<Vec<usize>>` is the derived cache of visible logical-line indices (`None` = all lines, no allocation). `build_view` and the pure accessors `first/last/next/prev_visible` + `snap_visible` (over `Option<&[usize]>`) are the single routing point — navigation, `draw`, and `do_search` all step through the view, so `top` stays a *logical* line index (marks, `-N` gutter numbers, and jumps compose unchanged). `clamp_view` snaps `top` onto a visible line; a zero-match filter parks `top` past the end so the body renders all `~`. Filters reset on `:n`/`:p` (`load`) and on `F` (`start_follow`).
- **File info (`=`, `^G`, `:f`)** — `info_string()` shows `name  lines a-b/total  pct%`, plus ` [filtered N/total]` when a filter is active.
- **Startup positioning (`+cmd`, `-p`, `StartAction`)** — `main.rs` parses `+N` / `+G` / `+/pat` (and `-p pat` as an alias of `+/`) into a `StartAction`; `apply_start` runs it once after the first `refresh_size` (so `+G`/search see real dimensions). `parse_plus` is the pure parser. Only these limited forms are supported (no general `+command` or `++` all-files). To open a file whose name starts with `+`, use a path that doesn't (`cless ./+file`), matching `less`.
- **Line wrapping (`wrap` field)** — on by default, like `less`; `-S` (flag or the `-S` key) chops long lines instead. Scroll position is `(top, sub)`: source line `top`, plus the wrap-segment `sub` at the top of the screen (`sub` is always 0 in chop mode). `wrap_ranges(line, cols)` is the single source of truth for where a line breaks (returns byte ranges over the concatenated span text); `render_segment` renders one such range and never re-decides the break. Navigation (`j`/`k`/page/half) moves by *display* rows via `pos_down`/`pos_up`; `g`/`G`/`%`/search jump to a logical line (`sub = 0`). Horizontal scroll (`left`, ←/→) is chop-mode only.
- `max_scroll()` is the wrap-aware analog of the old `max_top` — the position that puts the final screenful at the bottom; short files stay at the top (matches `less`: they do not scroll on search, only matches are highlighted). `clamp_pos` (pure, unit-tested) guards `sub` against pointing past a line's segment count after a resize widens the screen.
- **Line numbers (`numbers` field, `-N`)** — a gutter of `digit_count(lines) + 1` columns; the number prints on a line's first display row, continuation rows get a blank gutter. Wrapping and all position math run on `content_cols()` (screen width minus the gutter) so wrapped lines fit beside the numbers.
- **Inputs (`Source`)** — `Source::File` is read + highlighted lazily on switch; `Source::Memory` holds pre-highlighted stdin (single source, can't be re-read). `:n`/`:p` call `load()` which resets view/marks/follow. stdin is detected in `main.rs` via `IsTerminal` and can't be mixed with files.
- **Marks** — `m<c>` stores `(top, sub)` in `marks`; `'<c>` restores via `jump_to` (which records `prev_pos`), `''` returns to `prev_pos`. All jumps go through `clamp_pos`, so they compose with wrap/gutter changes.
- **Tail-follow (`F`)** — `start_follow` jumps to the end and sets `following`; the run loop then `event::poll`s with a timeout and `poll_growth` re-stats the file size on idle, reloading + re-highlighting the whole file (std-only, no `notify` dep). The loop redraws only on input/resize/new-data so an idle follow doesn't flicker. Files only; any key stops.

## Development conventions

- **Stay faithful to `less`**. Do not invent new keybindings (vim-style `h` / `l` motion was removed on purpose).
- **Do not break end-user install UX**. Before adding a dependency, verify that any C/native dependency can be hidden behind the prebuilt-binary distribution so `cargo binstall cless` keeps working.
- Confirm dependency additions in advance (adding tree-sitter languages is fine, anything else needs a chat).
- Color changes should be confined to the `FG_*` constants and `color_for`. No theme-switch flag yet.
- Comments only when the *why* is non-obvious.

## Verification workflow

Raw mode is required, so the interactive pager cannot be exercised in a headless environment. `cargo test` covers only the pure logic — `render_line`/`render_segment` SGR, `wrap_ranges`, `clamp_pos`, the line-number gutter, mark jumps, file switching, and `poll_growth` (a real temp-file growth check). Interactive behaviour (wrapped rendering, resize reflow, `-S`/`-N` toggles, `:n`/`:p`, and `F` follow) must be checked by running the binary in a real terminal.

## Unimplemented

- Theme-switch flag / configuration file
