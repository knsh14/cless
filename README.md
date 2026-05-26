# cless

シンタックスハイライト付きの less クローン。tree-sitter でファイルをパースし、less のキーバインドに準拠したターミナルページャを提供する。

## 特徴

- **less 互換のキーバインド**: 数字プレフィクス (`5j`, `100G`, `50%`)、移動・ページ・半ページ・先頭/末尾・パーセント指定・横スクロール
- **検索**: `/pattern` / `?pattern`、`n` / `N` で反復、smart-case (全小文字なら大文字無視)、マッチを反転表示
- **15 言語の tree-sitter ハイライト**: Rust, Python, JavaScript, TypeScript (TSX 含む), JSON, Go, Bash, TOML, YAML, HTML, CSS, C, Markdown
- **citruszest パレット** ([zootedb0t/citruszest.nvim](https://github.com/zootedb0t/citruszest.nvim) 由来) を truecolor で出力
- 言語検出は拡張子 → 特殊ファイル名 (`.bashrc` 等) → shebang の順

## インストール

### 推奨: `cargo binstall` (prebuilt バイナリをダウンロード、C コンパイラ不要)

```sh
cargo binstall cless
```

GitHub Releases の prebuilt バイナリを引いてくるので即終わる。対応ターゲット:
`x86_64-unknown-linux-gnu` / `aarch64-unknown-linux-gnu` /
`x86_64-apple-darwin` / `aarch64-apple-darwin` /
`x86_64-pc-windows-msvc`

`cargo-binstall` が無い場合は先に: `cargo install cargo-binstall`

### ソースからビルド

```sh
cargo install --path .
# or
cargo build --release   # target/release/cless
```

ソースビルドには C コンパイラが必要 (各 tree-sitter パーサが C で書かれているため。macOS: `xcode-select --install`、Ubuntu: `apt install build-essential`)。リリースビルドは約 10 MB。

## 使い方

```sh
cless <file>
```

### キーバインド

```
 移動
   j, ↓, ENTER, ^E, ^N       1 行下
   k, ↑, ^Y, ^P               1 行上
   SPACE, f, ^F, ^V, PgDn    1 画面下
   b, ^B, PgUp                1 画面上
   d, ^D                      半画面下
   u, ^U                      半画面上
   g, <, HOME                 先頭   ([N]g で N 行目)
   G, >, END                  末尾   ([N]G で N 行目)
   p, %                       [N] パーセント位置
   ←, →                       横スクロール (半画面)

 検索
   /pattern                   前方検索
   ?pattern                   後方検索
   n                          次のマッチ
   N                          逆方向に次のマッチ

 その他
   =, ^G                      現在位置を表示
   r, R, ^L                   画面を再描画
   h, H                       ヘルプ画面
   q, Q, ZZ, ^C               終了
```

数字プレフィクス: ほぼ全ての移動キーで `5j` / `100G` / `30%` のように繰り返し回数や行番号を指定できる。

## ハイライトのカスタマイズ

色は `src/highlight.rs` 先頭の `FG_*` 定数で定義し、`color_for(name)` が tree-sitter のキャプチャ名 (`keyword`, `function`, `string`, ...) から色を選ぶ。差し替えはここを編集して再ビルド。

対応キャプチャ名は `HIGHLIGHT_NAMES` の 26 種で、tree-sitter / Neovim の標準命名規則に沿う。

## アーキテクチャ

| ファイル | 役割 |
| --- | --- |
| `src/main.rs` | 引数処理とエラーハンドリング |
| `src/highlight.rs` | 言語判定 + tree-sitter で `Vec<Line>` を構築 |
| `src/pager.rs` | 端末制御 (crossterm)、入力ループ、検索、描画 |
| `src/bin/dump.rs` | 非対話デバッグヘルパ。`dump <file> [pattern]` でハイライト ANSI / マッチ行を stdout に出す |

## 制限

- stdin / パイプ入力には未対応 (ファイル引数のみ)
- バイナリ / 非 UTF-8 ファイルは開けない
- `F` (tail -f) 風モード、`m`/`'` マーク、`-N` 行番号表示、複数ファイル (`:n`) は未実装
- テーマ切替フラグなし (色変更は再ビルドが必要)

## ライセンス

MIT または Apache-2.0 のデュアルライセンス。利用者はどちらかを選択できる (Rust エコシステム標準)。

- [LICENSE-MIT](LICENSE-MIT)
- [LICENSE-APACHE](LICENSE-APACHE)

SPDX 識別子: `MIT OR Apache-2.0`

### コントリビューション

明示的に別の表明をしない限り、本リポジトリへのコントリビューションは Apache-2.0 ライセンス条項に従って、上記のとおりデュアルライセンスで提供されたものとみなす (追加の条件は付帯しない)。
