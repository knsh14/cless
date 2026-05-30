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
cargo test                                        # 3 tests in pager::tests
./target/release/cless <file>
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

## Architecture notes

### `highlight.rs`

- `detect_language(path, content)` — extension → special filenames (`.bashrc` etc.) → shebang (`#!/usr/bin/env python` etc.)
- `build_config(lang)` — pulls `LANGUAGE` / `HIGHLIGHTS_QUERY` / `INJECTIONS_QUERY` from each language crate, builds a `HighlightConfiguration`, calls `.configure()` with the 26 entries in `HIGHLIGHT_NAMES`
- `highlight_file(content, path)` — walks the `Highlighter::highlight` event stream with a scope stack, splits spans on newlines, returns `Vec<Line>`
- Palette is citruszest ([zootedb0t/citruszest.nvim](https://github.com/zootedb0t/citruszest.nvim)). Mapping lives in the `FG_*` constants and `color_for(name)`.

### `pager.rs`

- `Pager` struct fields: `top`, `left`, `count`, `mode`, `search`, `message`, `cols`, `rows`
- State machine: `Mode::Normal | SearchInput | Help`
- Full redraw every frame (no diffing). Each row is cleared with `Clear(ClearType::CurrentLine)` before writing.
- `render_line` emits a complete `\x1b[0;7;38;2;R;G;Bm`-form SGR per chunk so older terminals render inverse video reliably.
- Search uses the `regex` crate with smart-case (`case_insensitive` when the pattern is all lowercase).
- `max_top = lines - body_rows`; matches `less` behaviour — short files do not scroll on search, only the matches are highlighted.

## Development conventions

- **Stay faithful to `less`**. Do not invent new keybindings (vim-style `h` / `l` motion was removed on purpose).
- **Do not break end-user install UX**. Before adding a dependency, verify that any C/native dependency can be hidden behind the prebuilt-binary distribution so `cargo binstall cless` keeps working.
- Confirm dependency additions in advance (adding tree-sitter languages is fine, anything else needs a chat).
- Color changes should be confined to the `FG_*` constants and `color_for`. No theme-switch flag yet.
- Comments only when the *why* is non-obvious.

## Verification workflow

Raw mode is required, so the interactive pager cannot be exercised in a headless environment. Run `cargo test` to cover the `render_line` SGR output. For visual / interactive checks, run the binary in a real terminal.

## Unimplemented

- stdin / pipe input
- Line numbers (`-N`)
- Tail-follow mode (`F`)
- Marks (`m` / `'`)
- Multiple files (`:n`)
- Theme-switch flag / configuration file
