# CLAUDE.md

`cless` は tree-sitter ベースのシンタックスハイライト付き less クローン。Rust 単一バイナリ (Cargo, edition 2024)。リリースビルド ~10 MB、C コンパイラ必要 (各 tree-sitter パーサが C で書かれている)。

エンドユーザ向けには `cargo binstall cless` で GitHub Releases の prebuilt を引く運用にしている (`.github/workflows/release.yml` がタグ push で 5 ターゲットをビルドして公開)。

## プロジェクト構造

```
src/
  main.rs        エントリ。引数処理 → highlight → pager
  highlight.rs   言語判定 + tree-sitter で Vec<Line> を構築
  pager.rs       crossterm で raw mode、入力ループ、検索、描画
  bin/dump.rs    非対話デバッグヘルパ。ANSI を stdout に流す
.github/workflows/release.yml  タグ push で prebuilt をビルド・公開
```

## 主要コマンド

```sh
cargo build                                       # debug
cargo build --release                             # 初回は ~15 秒 (15 言語の C パーサをビルド)
cargo test                                        # pager::tests に 3 件
cargo run --bin dump -- src/main.rs               # ハイライト出力を目視確認
cargo run --bin dump -- src/main.rs main          # マッチ行のみ列挙 (検索ロジック検証用)
./target/release/cless <file>
```

## リリース手順

```sh
# 1. version を Cargo.toml で更新
# 2. commit & tag
git tag v0.1.0
git push origin v0.1.0
# 3. GitHub Actions が 5 ターゲットのバイナリをビルドして Releases にアップロード
# 4. cargo binstall cless で取得可能になる
```

## アーキテクチャの要点

### `highlight.rs`

- `detect_language(path, content)` — 拡張子 → 特殊ファイル名 (`.bashrc` 等) → shebang (`#!/usr/bin/env python` 等)
- `build_config(lang)` — 各言語 crate の `LANGUAGE` / `HIGHLIGHTS_QUERY` / `INJECTIONS_QUERY` で `HighlightConfiguration` を生成し、`HIGHLIGHT_NAMES` (26 個) で `.configure()`
- `highlight_file(content, path)` — `Highlighter::highlight` のイベントをスタックで追って、改行で span を切り `Vec<Line>` に変換
- パレットは citruszest ([zootedb0t/citruszest.nvim](https://github.com/zootedb0t/citruszest.nvim))。`FG_*` 定数と `color_for(name)` で対応

### `pager.rs`

- `Pager` 構造体: `top`, `left`, `count`, `mode`, `search`, `message`, `cols`, `rows`
- 状態機械: `Mode::Normal | SearchInput | Help`
- 毎フレームでフル再描画 (差分描画なし)。各行 `Clear(ClearType::CurrentLine)` してから書く
- `render_line` は `\x1b[0;7;38;2;R;G;Bm` 形式の完全 SGR を毎チャンクで吐く (古い端末でも反転確実)
- 検索は `regex` クレート、smart-case (全小文字パターンなら `case_insensitive`)
- `max_top = lines - body_rows`。less と同じく短いファイルでは検索しても画面は動かない (反転表示のみ)

## 開発上の規約

- **less 仕様に忠実に**。独自キーバインドは足さない (vim 流 `h`/`l` 移動は意図的に外した)
- **エンドユーザの install 体験を壊さない**: 依存を増やすときは prebuilt 配布で C 依存を END USER から隠せるかを確認。`cargo binstall cless` で完結する状態を維持
- 依存追加は事前に確認 (tree-sitter 言語の追加は OK、その他は要相談)
- 色変更は `FG_*` 定数と `color_for` のみで完結する。テーマ切替フラグはまだない
- コメントはなぜが非自明な時だけ書く

## 検証の進め方

raw mode が必要なため対話的なページャの動作は headless 環境では確認できない。代わりに:

- `cargo test` で `render_line` の SGR 出力をテスト
- `cargo run --bin dump -- <file>` で ANSI 出力を目視 (パイプして `cat -v` でエスケープも見える)
- 検索ロジックは `dump <file> <pattern>` でマッチ行が出るか確認

## 未実装

- stdin パイプ入力
- 行番号表示 (`-N`)
- ファイル末尾追従 (`F`)
- マーク (`m`/`'`)
- 複数ファイル (`:n`)
- テーマ切替フラグ / 設定ファイル
