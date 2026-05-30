# cless

A `less`-like terminal pager with tree-sitter syntax highlighting.

## Features

- **`less`-compatible keybindings**: number-prefix counts (`5j`, `100G`, `50%`), line / page / half-page motion, top / bottom / percent jumps, horizontal scroll
- **Search**: `/pattern` and `?pattern`, repeat with `n` / `N`, smart-case (all-lowercase pattern is case-insensitive), matches shown in reverse video
- **Tree-sitter highlighting for 15 languages**: Rust, Python, JavaScript, TypeScript (incl. TSX), JSON, Go, Bash, TOML, YAML, HTML, CSS, C, Markdown
- **citruszest palette** (from [zootedb0t/citruszest.nvim](https://github.com/zootedb0t/citruszest.nvim)) emitted as truecolor ANSI
- Language detection: file extension → special filenames (`.bashrc` etc.) → shebang

## Installation

### Recommended: `cargo binstall` (downloads a prebuilt binary, no C toolchain needed)

```sh
cargo binstall cless
```

This pulls a prebuilt binary from GitHub Releases. Supported targets:
`x86_64-unknown-linux-gnu` / `aarch64-unknown-linux-gnu` /
`x86_64-apple-darwin` / `aarch64-apple-darwin` /
`x86_64-pc-windows-msvc`.

If you don't have `cargo-binstall` yet: `cargo install cargo-binstall`.

### Building from source

```sh
cargo install --path .
# or
cargo build --release   # target/release/cless
```

Building from source requires a C compiler because each tree-sitter parser is written in C
(macOS: `xcode-select --install`, Ubuntu: `apt install build-essential`).
The release binary is ~10 MB.

## Usage

```sh
cless <file>
```

### Keybindings

```
 Movement
   j, ↓, ENTER, ^E, ^N       one line down
   k, ↑, ^Y, ^P               one line up
   SPACE, f, ^F, ^V, PgDn    one window down
   b, ^B, PgUp                one window up
   d, ^D                      half-window down
   u, ^U                      half-window up
   g, <, HOME                 first line   ([N]g jumps to line N)
   G, >, END                  last line    ([N]G jumps to line N)
   p, %                       [N] percent into the file
   ←, →                       horizontal half-screen scroll

 Searching
   /pattern                   search forward
   ?pattern                   search backward
   n                          repeat last search
   N                          repeat in the reverse direction

 Other
   =, ^G                      show current file position
   r, R, ^L                   repaint the screen
   h, H                       help screen
   q, Q, ZZ, ^C               quit
```

Number prefix: most movement keys accept a repeat count or line number
(e.g. `5j`, `100G`, `30%`).

## Customising the palette

Colors are defined as `FG_*` constants at the top of `src/highlight.rs`, and
`color_for(name)` maps tree-sitter capture names (`keyword`, `function`,
`string`, ...) to those colors. Edit and rebuild to change the theme.

The 26 supported capture names live in `HIGHLIGHT_NAMES` and follow the
standard tree-sitter / Neovim naming conventions.

## Architecture

| File | Role |
| --- | --- |
| `src/main.rs` | Arg parsing and error handling |
| `src/highlight.rs` | Language detection + tree-sitter; builds `Vec<Line>` |
| `src/pager.rs` | Terminal control (crossterm), input loop, search, rendering |

## Limitations

- No stdin / pipe input (file argument only)
- Cannot open binary or non-UTF-8 files
- Not implemented: `F` (tail -f mode), `m`/`'` marks, `-N` line numbers, multi-file (`:n`)
- No theme-switch flag (color changes require a rebuild)

## License

Dual-licensed under MIT or Apache-2.0; users may choose either (Rust ecosystem standard).

- [LICENSE-MIT](LICENSE-MIT)
- [LICENSE-APACHE](LICENSE-APACHE)

SPDX identifier: `MIT OR Apache-2.0`

### Contributions

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this repository shall be dual-licensed as above, without any
additional terms or conditions.
