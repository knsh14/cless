# CLESS 環境変数によるデフォルトオプション

## 背景と目的

`cless` を使うたびに `-N`（行番号）や `-S`（行折り返し無効）を手で打たずに済むよう、
デフォルトの起動オプションを指定できるようにする。

`less` はデフォルトオプションを設定ファイルではなく環境変数 `LESS` で持たせる設計になっている
（`lesskey` の設定ファイルは主にキーバインド用）。`cless` もこの流儀に倣い、環境変数
`CLESS` を読む。XDG 設定ファイルは、フラグでは表現しきれない構造化設定（テーマ切り替え等）が
必要になった段階で別途検討する。現状のオプションは実質 `-S` / `-N` / `-p` / `+cmd` のみで、
環境変数で十分。

## 挙動仕様

### 読み取りと分割

- 起動時に環境変数 `CLESS` を読む。未設定または空文字列なら何もしない（no-op）。
- 値は `split_whitespace()` でトークン分割する。シェル的なクォート処理はしない
  （`less` の `LESS` と同じ割り切り）。

### 認識するトークン

`CLESS` は**オプション専用**。認識するのは既存のコマンドラインオプションと同じ集合:

- `-S` — 行折り返しを無効化（chop）
- `-N` — 行番号を表示
- `-p pattern` — 起動時に pattern へジャンプ（`+/pattern` のエイリアス）
- `+cmd` — 起動位置指定（`+G` / `+/pat` / `+N`、`parse_plus` が解釈）

### 優先順位

`CLESS` を先に適用し、その後コマンドライン引数を適用する。スカラ状態
（`wrap` / `numbers` / `start`）は後勝ちなので、コマンドライン引数が最終値を決める。

- `CLESS="-N"` + `cless file` → 行番号 ON
- `CLESS="-N"` + `cless +G file` → 行番号 ON かつ末尾から開始（`start` は argv 側が勝つ）

### 無効トークンの扱い

無効なトークンは**警告して無視**する（即終了しない）。env の typo でツールが全く使えなくなる
事態を避けるため。対象:

- 未知オプション（例 `-Q`）
- 値欠落（例 末尾の `-p` で pattern が続かない）
- 不正な `+cmd`（`parse_plus` が `None` を返す）
- パスらしき裸のトークン（`CLESS` はオプション専用なので、非オプション語は無視）

コマンドライン引数側の挙動は**変更しない**（未知オプションは従来どおりエラーで即終了、
裸のトークンはファイルパス）。無効トークンの寛容な扱いは `CLESS` 由来のトークンにのみ適用する。

### 警告の表示先

`cless` は起動時に代替スクリーン（alternate screen）へ切り替えるため、その前に stderr へ出した
警告は画面切り替えで消える恐れがある。よって警告は**ページャの message 行**に出す。

- 既存の `Pager.message` フィールドを流用し、初期化時にセットして初回描画で表示する。
- 複数の警告はまとめて1行にする。例: `CLESS: ignored -Q; -p missing value`
- 警告が無ければ message は従来どおり空。

## 実装方針

### env と argv は別パスで解析する

env と argv を1つのトークン列に連結して解析しない。連結すると、env 末尾の `-p` が argv 先頭の
ファイル名を pattern として消費する不具合が出るため。それぞれ独立に解析する。

### env 解析を純粋関数として切り出す

```rust
struct EnvSettings {
    wrap: Option<bool>,
    numbers: Option<bool>,
    start: Option<StartAction>,
}

fn parse_cless_env(value: &str) -> (EnvSettings, Vec<String> /* warnings */)
```

- `value` を `split_whitespace()` で走査し、認識したオプションを `EnvSettings` に記録する。
  同じオプションが複数回来たら後勝ち（`Option` を上書き）。
- 無効トークンは warnings に人間可読なメッセージを push して読み飛ばす。
- `-p` は次トークンを pattern として消費する。次トークンが無ければ warning。

### main のフロー

1. デフォルト値を用意（`wrap = true`, `numbers = false`, `start = None`）。
2. `std::env::var("CLESS")` を取得できたら `parse_cless_env` にかけ、`EnvSettings` の
   `Some` 値でデフォルトを上書きする。warnings を保持する。
3. 既存の argv 解析ループをそのまま回し、同じ状態変数を上書きする（argv が後勝ち）。
4. warnings が空でなければ `"; "` で結合して `Option<String>` を作る。
5. `pager::run(sources, wrap, numbers, start, warning)` に渡す。

### pager 側の配線

- `pager::run` のシグネチャに `warning: Option<String>` を追加する。
- `Pager` 初期化時に `self.message = warning.unwrap_or_default()`（既存 message フィールドの型に合わせる）。

## 割り切り（YAGNI）

- boolean をコマンドラインから**打ち消す手段は追加しない**。`CLESS="-N"` を単発で無効化する
  逆フラグ（`less` の再指定トグル相当）は今回入れない。必要になった段階で別途検討する。
- クォート未対応。スペースを含む `-p` パターンを `CLESS` に入れるのは非対応
  （現実的ユースケースは `-N -S` 程度なので許容）。
- XDG 設定ファイルは今回のスコープ外。

## テスト

`parse_cless_env` の純粋関数テストを追加する（既存の `parse_plus` テストと同様の位置づけ）:

- `""` → no-op（すべて `None`、warnings 空）
- `"-N -S"` → `numbers = Some(true)`, `wrap = Some(false)`, warnings 空
- `"-p foo"` → `start = Some(StartAction::Search("foo"))`, warnings 空
- `"-Q"` → 該当 setting は `None`、warnings に未知オプションが1件
- `"-p"`（末尾で値欠落）→ warnings に値欠落が1件
- `"somefile"`（裸のトークン）→ warnings に1件、settings は `None`
- `"+G"` → `start = Some(StartAction::End)`（`parse_plus` 経由）

インタラクティブ挙動（message 行への警告表示、実際の env 反映）は既存方針どおり
実端末での手動確認とする。

## ドキュメント

- `CLAUDE.md` の該当箇所（Startup positioning 近辺、または新規小節）に `CLESS` 環境変数の
  説明を1段落追加する。
- `USAGE` 文字列は変更不要（`CLESS` は環境変数でありコマンドライン構文ではないため）。
