# rverilog: Rust 製 Verilog-2001 サブセット シミュレータ 実装計画

## Context

`/Users/zosan/github/rverilog` をゼロから立ち上げ、Rust 製の Verilog シミュレータを構築する。動機は、OSS の既存実装（Icarus Verilog: C++、Verilator: 2 値中心）に対し、4 値ロジックを保ちつつ Rust の型安全性とモジュール性を活かした保守しやすいシミュレータを得ること。最初のマイルストーン (M1) では「階層モジュール構造を持つ実用 RTL（同期 FIFO + 4bit カウンタ）が、testbench 込みでイベント駆動シミュレーションされ、$display 出力と VCD 波形を生成する」状態を目指す。

実装中のメッセージは日本語。

進捗を随時、DIARY.mdに記録すること。

## 確定した方針

| 項目 | 選択 |
|---|---|
| 言語仕様 | Verilog-2001 サブセット（合成可能サブセット＋$display/$finish/$time/$dumpfile/$dumpvars/$monitor） |
| シミュ戦略 | イベント駆動インタプリタ（Icarus 風） |
| 値モデル | 4 値ロジック (0/1/X/Z)、aval/bval 2 平面表現 |
| パーサ | `sv-parser` クレート（dalance）を採用、自前実装はしない |
| 構成 | Cargo workspace で複数クレート分割 |
| M1 デモ | 同期 FIFO ＋ 4bit カウンタ |
| M1 受入れ | サンプルが動き VCD が生成されれば合格（iverilog との bit-exact 一致は不要） |

## アーキテクチャ概観

```
.v files
  → sv-parser (preproc + parse)        : SyntaxTree
  → frontend lowering                   : SyntaxTree → HIR、サブセット外の検出
  → elaboration                         : param 解決 / instance 展開 / 名前解決 / 幅推論
  → MIR                                 : フラット化された実行用 IR + LogicVal
  → event-driven simulator              : active / inactive / NBA / monitor リージョン
  → VCD writer + $display sink
```

依存方向は単方向（cli → vcd_out → sim → mir ← elab ← hir ← frontend）。循環なし。

## クレート分割

ワークスペースルート: `/Users/zosan/github/rverilog/Cargo.toml`

| クレート | パス | 責務 |
|---|---|---|
| `rverilog-frontend` | `crates/frontend/` | sv-parser 呼び出し、SyntaxTree → HIR 変換、サブセット外ノード検査 |
| `rverilog-hir` | `crates/hir/` | 階層展開前 IR（Module 定義 / 宣言 / 文 / 式） |
| `rverilog-elab` | `crates/elab/` | パラメータ解決、インスタンス展開、名前解決、ビット幅・符号推論、HIR→MIR |
| `rverilog-mir` | `crates/mir/` | 階層展開後 IR、`LogicVal` 型、シンボルテーブル |
| `rverilog-sim` | `crates/sim/` | スケジューラ、プロセス実行、システムタスク（$display 等） |
| `rverilog-vcd` | `crates/vcd_out/` | VCD writer ラッパ、$dumpfile/$dumpvars 制御 |
| `rverilog-cli` | `crates/cli/` | バイナリ。clap 引数解析、ドライバ |

## 主要データ構造

### 4 値ロジック `LogicVal` (mir/src/logicval.rs)

aval/bval 2 平面表現（IEEE 1364 準拠）:
- `(a,b)=(0,0)→0`, `(1,0)→1`, `(0,1)→Z`, `(1,1)→X`

```rust
pub enum LogicVal {
    Small { width: u16, a: u64, b: u64 },                    // ≤64 bit
    Large { width: u32, a: SmallVec<[u64;4]>, b: SmallVec<[u64;4]> }, // それ以上
}
```

実装する演算: 算術 (+ − × ÷ %)、論理 (`&& || !`)、ビット (`& | ^ ~ ~^`)、比較 (`== != === !== < > <= >=`)、シフト (`<< >> <<< >>>`)、三項、リダクション、連結 `{a,b}`、リピート `{N{a}}`、ビット選択、部分選択、符号拡張・ゼロ拡張。

X/Z 伝搬:
- ビット演算は IEEE 表通り（`x & 0 = 0`, `x & 1 = x`）
- 算術は M1 では「任意ビットが X/Z なら全ビット X」で割り切る（M2 で精度向上）
- `==` は X 含むと X、`===` は X/Z リテラル比較

### MIR ノード (mir/src/ir.rs)

```rust
pub struct ElaboratedDesign {
    pub nets: Arena<NetInfo>,
    pub processes: Arena<Process>,
    pub conts: Vec<ContAssign>,
    pub scopes: Arena<Scope>,
    pub top: ScopeId,
}
pub struct NetInfo { pub width: u32, pub kind: NetKind, pub scope: ScopeId, pub name: SmolStr }
pub enum Stmt {
    Block(Vec<StmtId>),
    If(ExprId, StmtId, Option<StmtId>),
    Case { sel: ExprId, arms: Vec<(Vec<ExprId>, StmtId)>, default: Option<StmtId>, kind: CaseKind },
    BlockingAssign(LValue, ExprId),
    NbaAssign(LValue, ExprId, Option<DelayCtl>),
    Delay(u64, StmtId),
    EventCtl(Sensitivity, StmtId),
    SysCall(SysTask, Vec<ExprId>),
}
pub enum Expr {
    Const(LogicVal),
    Net(NetId),
    BitSel(NetId, ExprId),
    PartSel(NetId, Range),
    Concat(Vec<ExprId>),
    Repeat(ExprId, Vec<ExprId>),
    Bin(BinOp, ExprId, ExprId),
    Un(UnOp, ExprId),
    Cond(ExprId, ExprId, ExprId),
}
```

Arena は `id-arena` を採用。ID 化により再帰借用問題を回避。

### イベントスケジューラ (sim/src/scheduler.rs)

IEEE 1364 stratified event queue を 4 リージョンで実装:

```rust
pub struct Scheduler {
    now: u64,
    future: BinaryHeap<Reverse<(u64 time, u32 seq, Event)>>,
    active: VecDeque<Event>,
    inactive: VecDeque<Event>,
    nba: VecDeque<NbaUpdate>,
    monitor: Vec<MonitorTask>,
    seq: u32,
}
pub enum Event {
    ResumeProcess(ProcessId, Pc),
    Update(NetId, LogicVal),
    EvalCont(ContId),
    Wake(ProcessId),
}
```

動作: `active` が空になるまで処理 → `inactive` を `active` に流し込み再度処理 → `nba` を全適用（更新を `active` キューに登録） → `active` が完全に空になったら `monitor` 評価 → 次の時刻へ進む。

プロセス実行は **継続/PC 方式の明示ステートマシン**。`#delay`/`@event` で yield、再開時に PC を進める。`Box<dyn FnMut>` ではなく `Vec<Stmt>` の index を保存することで決定論性を確保。

ネット値変化に対応するプロセスは、エラボ時に構築する `NetId → Vec<ProcessId>` の sensitivity 逆引きテーブルから引く。

## パイプライン詳細

### Frontend (`rverilog-frontend`)

```rust
pub fn parse_files(
    paths: &[PathBuf],
    includes: &[PathBuf],
    defines: &[(String, String)],
) -> Result<hir::Design, FrontendError>;
```

- `sv_parser::parse_sv` を呼び出してプリプロセッサ込みでパース
- SyntaxTree を 2 段階で訪問:
  1. 収集パス: `ModuleDeclaration` を全部 `HirModuleDef` に収集
  2. 下げパス: 各モジュール内の `ContinuousAssign` / `AlwaysConstruct` / `InitialConstruct` / `ModuleInstantiation` / `NetDeclaration` / `RegDeclaration` を訪問して HIR 構築
- **受理ホワイトリスト方式**でサブセット外ノード（`generate`/`function`/`task`/`interface`/`class` 等）に遭遇したら `FrontendError::UnsupportedConstruct { kind, span }` を返す

### Elaboration (`rverilog-elab`)

入力: `hir::Design` ＋ top module name ＋ パラメータオーバライド  
出力: `mir::ElaboratedDesign`

段階:
1. パラメータ評価（コンパイル時定数式の畳み込み、`mir::eval_const` を共有）
2. インスタンスツリー再帰展開、各スコープに `ScopeId` 採番
3. 名前解決 (`HierPath → NetId`)、`indexmap::IndexMap` でスコープ毎テーブル
4. ビット幅／符号推論（IEEE 1364 self-determined / context-determined ルール、bottom-up max width → top-down context width の 2 パス）
5. ポート接続を内部ネット結線に変換

### Sim & VCD

- システムタスク `$display`/`$write`/`$monitor`/`$finish`/`$time` は sim 内で直接実装
- `$dumpfile`/`$dumpvars` は VCD writer のセットアップを行う制御コマンドとして発火
- `vcd` クレート（kmcallister）を VCD 出力に使用、書込のみで API は十分

## M1 で対応する Verilog 機能

- `module` / `endmodule`、ANSI/non-ANSI ポートスタイル
- `parameter`、`localparam`、`#(...)` 順序・名前両方のオーバライド
- `input`/`output`/`inout`
- `wire`、`reg`、`integer`（32bit signed reg として扱う）、ベクタ宣言 `[7:0]`
- 連続代入 `assign`（M1 は遅延無し）
- `initial`、`always @(...)`（`posedge`/`negedge`/`*`/レベル）
- `begin`-`end`、`if`-`else`、`case`/`casez`/`casex`/`default`
- `#delay`、`@(event)`、`@*`
- ブロッキング `=`、ノンブロッキング `<=`（タイミング制御付き含む）
- 階層インスタンス、順序ポート接続、名前付きポート接続 `.port(net)`、`.port()`（オープン）
- 演算子: 算術、論理、ビット、比較、シフト、三項、リダクション、連結、リピート、部分選択、ビット選択
- システムタスク: `$display`, `$write`, `$monitor`, `$finish`, `$time`, `$dumpfile`, `$dumpvars`
- 数値リテラル: `8'hAB`, `4'b10x1`, `'d10`、signed/unsigned

### M1 では明示的にやらない

`function`/`task`/`generate`/`genvar`/`defparam`/`specify` block/`force`/`release`/`disable`/`fork`-`join`/UDP/ゲートプリミティブ/SystemVerilog 全機能（`logic`/`always_ff`/struct/interface/class/package/assertion）/`real`/`$readmemh`/`$random`/`$fopen`/PLI/VPI

## 推奨依存クレート

| クレート | 用途 | 備考 |
|---|---|---|
| `sv-parser` | パーサ | 確定 |
| `vcd` | VCD writer | デファクト、不足時は自前 writer に切り替え可（VCD 仕様は単純） |
| `thiserror` | error 型 | 各クレートのドメインエラー |
| `anyhow` | CLI 層のみ | ライブラリでは使用しない |
| `clap` v4 (derive) | CLI 引数 | 標準 |
| `indexmap` | 順序保持マップ | 宣言順保持、VCD 出力順安定化 |
| `smallvec` | スタック格納 | LogicVal::Large、文ベクタ |
| `smol_str` | 識別子 | 短い識別子インライン化 |
| `id-arena` | IR ノード ID | 借用回避 |
| `tracing` + `tracing-subscriber` | ログ | 階層ログが elaboration デバッグに有用 |
| `insta` | スナップショットテスト | $display／VCD 比較 |
| `rstest` | パラメタライズドテスト | 演算子テーブル網羅 |
| `proptest` | プロパティテスト | LogicVal 演算の代数的性質 |

不採用:
- `bitvec`: 4 値の 2 平面管理は自前で書く方が明快

## 重要ファイル一覧（M1 で実装）

- `/Users/zosan/github/rverilog/Cargo.toml`（workspace）
- `/Users/zosan/github/rverilog/crates/mir/src/logicval.rs`（4 値型と全演算）
- `/Users/zosan/github/rverilog/crates/mir/src/ir.rs`（MIR ノード）
- `/Users/zosan/github/rverilog/crates/frontend/src/lower.rs`（SyntaxTree → HIR）
- `/Users/zosan/github/rverilog/crates/frontend/src/error.rs`（サブセット外検査）
- `/Users/zosan/github/rverilog/crates/elab/src/instance.rs`（インスタンス展開）
- `/Users/zosan/github/rverilog/crates/elab/src/param.rs`（パラメータ解決）
- `/Users/zosan/github/rverilog/crates/elab/src/width.rs`（ビット幅・符号推論）
- `/Users/zosan/github/rverilog/crates/sim/src/scheduler.rs`（イベントキュー）
- `/Users/zosan/github/rverilog/crates/sim/src/interp.rs`（プロセスインタプリタ）
- `/Users/zosan/github/rverilog/crates/sim/src/systask.rs`（$display 等）
- `/Users/zosan/github/rverilog/crates/vcd_out/src/dump.rs`（VCD 出力）
- `/Users/zosan/github/rverilog/crates/cli/src/main.rs`（バイナリ）

## マイルストーン

### M0: 足場
1. workspace スケルトン（全クレート空 + `lib.rs`）
2. `LogicVal` Small バリアント + 主要演算 + 単体テスト
3. CLI スケルトン（`rverilog --top NAME file.v` でパースし AST ノード数表示のみ）
4. CI 設定（fmt/clippy/test）

### M1: 階層 RTL + VCD（メイン）
1. HIR 定義 ＋ frontend lowering（モジュール / 宣言 / assign / always / initial）
2. LogicVal Large バリアント ＋ 全演算 ＋ proptest
3. Elaboration（top 展開、parameter override、instance flatten、name resolve）
4. ビット幅・符号推論
5. MIR 構築 ＋ sensitivity 逆引きテーブル
6. Scheduler 骨格（active/nba/inactive/monitor）
7. プロセス実行インタプリタ（Stmt/Expr 全部）
8. システムタスク（$display/$finish/$time/$monitor/$dumpfile/$dumpvars）
9. VCD writer 統合
10. `samples/counter4` 通過 → `samples/fifo_sync` 通過

### M2 以降

完了済み（2026-07-02 時点）:
- `function`/`task`、`generate`/`genvar`、ゲートプリミティブ
- `disable`/`fork`-`join`
- `$readmemh`/`$readmemb`/`$random`、部分 X 伝搬の精度向上（$display 系フォーマッタ）
- iverilog との出力比較を CI 導入

残タスク（優先順は「実装レビューと課題」参照）:
1. signed 対応（幅・符号推論の再設計とセット）
2. エッジ検出の IEEE 準拠化（X→1 posedge 等）
3. monitor リージョン実装 + `#0` (inactive) 順序修正（イベントループ再構成として一括）
4. 連続代入の sensitivity 駆動化
5. SystemVerilog 拡張（`logic`/`always_ff`/`always_comb`/struct/typedef）

## サンプル & テスト戦略

### サンプル
- `samples/counter4/`: 4bit カウンタ + 階層 testbench（最小確認）
- `samples/fifo_sync/`: 同期 FIFO（パラメタライズド WIDTH/DEPTH、書込/読出 state machine）— **M1 受入れデモ**

### テスト
- 単体テスト: 各クレート内 `#[cfg(test)]`
  - `mir/logicval.rs`: 真理表ベース ＋ proptest（結合則・X 抜き同値性等）
  - `sim/scheduler.rs`: ダミーイベント注入による順序保証
  - `elab/param.rs`: パラメータオーバライドの定数畳み込み
- 統合テスト: `tests/integration/cases/<name>/{dut.v, tb.v, expected.stdout}`
  - ハーネスは `rverilog-cli` をライブラリ呼び出しし、`$display` 出力と VCD 値変化シーケンスを `insta::assert_snapshot!` で比較
  - VCD はタイムスタンプ等の volatile 部分を正規化する pre-processor を通す
- iverilog 並行検証は **M1 範囲外**（M2 で導入）

## 検証方法（M1 完了判定）

### コマンドライン形
```bash
cargo run -p rverilog-cli --release -- \
    --top tb \
    -I rtl/include \
    -D SIM=1 \
    -o build/fifo.vcd \
    samples/fifo_sync/rtl/*.v samples/fifo_sync/tb/tb.v
```

CLI 引数:
- `--top <name>`: トップモジュール（必須）
- `-I/--include <dir>`: インクルードパス（複数）
- `-D/--define KEY[=VAL]`: マクロ定義（複数）
- `-o/--output <vcd>`: VCD 出力先
- `--max-time <T>`: タイムアウト
- `-v/--verbose`: tracing レベル

### 動作確認手順
1. `cargo test --workspace` 全通過
2. `cargo run -p rverilog-cli -- --top tb -o build/counter4.vcd samples/counter4/**/*.v` が exit 0、VCD ファイル生成
3. `cargo run -p rverilog-cli -- --top tb -o build/fifo.vcd samples/fifo_sync/**/*.v` が exit 0、VCD ファイル生成
4. 生成された VCD を `gtkwave` で開いて期待挙動を目視（ユーザ判定）
5. `$display` 出力が `insta` スナップショットと一致

## リスク・未解決事項

- **sv-parser AST 変換工数**: ノード網羅が想定より重い可能性。緩和策は受理ホワイトリスト＋未対応即エラーで M1 範囲を絞ること
- **NBA 領域の正確性**: 同時刻イベントの順序（active→inactive→nba→monitor）が正しくないとレース条件が後で発覚。M1 はスナップショットベースで担保、M2 で iverilog diff 導入
- **ビット幅推論**: IEEE の context-determined ルールは罠が多い。expression ごとの 2 パス推論で対処、proptest 候補
- **\`include / \`define の解決順**: sv-parser プリプロセッサ API の細部を M0 で実機確認
- **vcd クレートの十分性**: 書込のみなので恐らく問題なし、不足時は自前 writer 化
- **決定論性**: HashMap 順序による非決定論を避けるため `IndexMap` または `Vec` 経由で走査

## 実装課題（2026-07-09 マージ後）

M1 マージ後のコードベース全体の棚卸し結果。2026-07-02 レビュー（旧 `docs/implementation-review`
ブランチ）と 2026-07-09 の再調査を統合。`feat/func-task-gates` / `feat/generate-disable-fork` /
`docs/implementation-review` の 3 ブランチは本セクション執筆時点で master にマージ済み
（function/task/gate primitive/generate/genvar/disable/fork-join/$readmemh/$readmemb/$random/
iverilog 出力比較 CI 導入済み）。

### A. 正確性（シミュレーション結果が誤りになる）

| # | 課題 | 箇所 | 状態 |
|---|---|---|---|
| A1 | signed 演算 | `hir::design.rs`/`mir::ir.rs`（`signed`/`is_signed`フィールド）、`elab::elaborate.rs`（`expr_signed`伝搬）、`mir::logicval.rs`（`*_signed`演算群） | **対応済み（2026-07-09）**。net/reg/port/integer/function/task 引数の `signed` 宣言、符号無し10進即値と `'s` 基数リテラルの既定signed扱い、signed比較（`<`/`>`/`<=`/`>=`）・signed除算/剰余・`>>>`の左辺signedness依存・signed `%d` 表示を実装。iverilogとのbit-exact比較テスト`tests/integration/cases/signed/`で検証済み。既知の残課題: 式の最終signednessは子ExprIdからの単純な機械的伝搬（IEEE 4.5.1のcontext-determined規則の一部簡略化）、`width.rs`自体は未着手のまま |
| A2 | エッジ検出が IEEE 非準拠 | `sim/src/interp.rs` `trigger_sensitivity` | **対応済み（2026-07-14）**。aval/bval 両プレーンから `{0,1,X,Z}` を分類し、IEEE 表（posedge: `0→1`/`0→X`/`X→1`、negedge: `1→0`/`1→X`/`X→0`、Z は edge 判定上 X 相当）で判定するよう修正。iverilogとのbit-exact比較テスト`tests/integration/cases/edge_x/`で検証済み |
| A3 | `$monitor` が `$display` と同一動作 | `sim/src/interp.rs` | monitor リージョンがなく値変化時の再表示なし |
| A4 | `#0` のリージョン順序が逆 | `sim/src/interp.rs` `run` | `#0` が future ヒープ（同時刻）経由のため NBA 適用の後に再開される。IEEE の inactive→NBA 順と逆 |
| A5 | 64bit 超ネットへの部分書き込みが壊れている | `sim/src/interp.rs` `write_lvalue`・初期化 | ビット/部分選択パスが u64 前提。LogicVal 側は Large 対応済みなのに書き込み側が未対応 |
| A6 | 算術/比較の X 伝搬が粗い | `mir/src/logicval.rs` | 任意 1bit でも X/Z なら結果全体が X（M1 の割り切りだが IEEE より粗い。`===`/`!==` は正しくビット比較） |
| A7 | 64bit 超の乗除算・剰余が常に X | `mir/src/logicval.rs` | multi-word の mul/div/mod が未実装 |
| A8 | inout が実質 input | `elab/src/elaborate.rs` | 親→子の単方向結線のみ。双方向・tri-state・多重ドライバ解決・strength モデリングなし（Z は表現できるがネット上で解決されない） |
| A9 | 連続代入の `#delay` が無視される | `frontend/src/lower.rs` `lower_continuous_assign` | 遅延指定が黙って捨てられる。手続き文の `#delay` のみ有効 |

### B. 未対応の言語機能

- **サイレントスキップ（診断なしで捨てられる — 最も危険）**: `defparam`、`specify`、UDP は
  `lower.rs` の `_ => {}` catch-all で無言スキップされる。最低限 `UnsupportedConstruct` エラーに
  すべき
- **明示エラーになるもの**: `while`/`repeat`/`forever`（`for` のみ対応）、`**` 演算子、
  式中の関数呼び出し
- `real`/`realtime` が型検査なしで 1bit reg として解釈される
- `disable` は同一プロセス内のみ対応、関数内 `fork`/`disable` は無視（コード内コメントで明記済み）

### C. システムタスク・関数の不足

- 未実装: `$stop`、`$strobe`、`$fopen`/`$fclose`/`$fwrite` 等ファイル I/O、
  `$value$plusargs`/`$test$plusargs`、`$realtime`、`$signed`/`$unsigned`、`$dumpoff`/`$dumpon`
- 式の中の `$time` が未対応（`eval_expr` に SysFunc 分岐がない。`$clog2` は elaboration 時の
  定数畳み込みでのみ動作）
- `$random` は iverilog と数値非互換（固定シード xorshift64*、bit-exact 一致は目標外）

### D. 設計と実装の乖離・死コード

- `sim/src/scheduler.rs` が TODO スタブのまま死コード。実イベントループは `Interpreter::run` に
  別実装されており、未使用の `future: VecDeque` は時刻順ソートされないバグも内包。
  削除するか interp ループを移設して一本化する
- `sim/src/systask.rs` も全関数 TODO スタブで `interp.rs::exec_syscall` と重複（`lib.rs` で
  pub use されたまま）
- 連続代入が計画の「NetId→プロセス逆引きテーブル」方式でなく総当たり固定点ループ
  （`interp.rs` `eval_conts`、最大 200 回打ち切り）。規模で性能劣化し、発振回路で黙って誤結果
  （最低限、打ち切り時に警告を出す）
- `elab/src/width.rs` が実質スタブ（14 行）。IEEE の context-determined 幅推論が体系実装されず
  幅処理が elaborate.rs に分散。A1（signed 対応）の障害になる

### E. テスト・CI

- iverilog 出力比較 CI 導入済み（`.github/workflows/ci.yml`、`crates/cli/tests/iverilog_compare.rs`）
- fmt/clippy ジョブは未導入
- 統合テストは 8 ケース（counter4/fifo_sync/disable_fork/format_xz/func_task/gates/generate/
  readmem_random）。サブセット外構文のエラーを確認する負パステストはまだない

### F. 軽微

- `$dumpvars` の深さ・スコープ引数未対応（常に全ダンプ）
- `casez`/`casex` のワイルドカードマッチが `case` と同一実装の可能性（要確認）
- `$display("%s", "文字列")` が動作しない（StringLit の eval が ZERO を返す）
- 連結 lvalue `{a,b} = ...` は先頭要素のみ代入され残りは無言で捨てられる（`frontend/src/lower.rs`）
  → $signed 対応作業（2026-07-12 節参照）の Task 2d として対応中
- リポジトリの CLAUDE.md が空、`tests/rtl/fifo_counter.v` が未使用

### 推奨着手順

1. ~~A1: signed 対応~~ 完了（2026-07-09）
2. ~~A2: エッジ検出の IEEE 準拠化~~ 完了（2026-07-14）
3. A3・A4: monitor リージョン実装 + `#0`（inactive）順序修正（イベントループ再構成として一括）
4. D: 連続代入の sensitivity 駆動化（scheduler.rs/systask.rs の死コード整理はどのタイミングでも安価）
5. B: サイレントスキップの診断化（`_ => {}` を `UnsupportedConstruct` エラーに置換）
6. E: fmt/clippy ジョブの CI 追加、負パステストの拡充

補足: A1 で `width.rs` 自体の context-determined 幅推論再設計は見送った（signedness 伝搬のみ
`ElabCtx.expr_signed` として別経路で実装し、幅計算は既存の elaborate.rs 分散実装のまま）。
width.rs のスタブ化（D 参照）は依然未解消。

修正前に対応するテストケース（X→1 posedge、`$monitor`、`#0` レース、発振検出）を
iverilog 比較 CI へ追加してから直すこと（テスト先行）。

## SystemVerilog 準拠度調査（2026-07-10）

コードベース全体（`crates/frontend/src/lower.rs` の SV 構文分岐、`crates/mir/src/ir.rs` の
`SysTask`/`BinOp`/`Stmt` 列挙、`crates/sim/src/interp.rs` のシステムタスク実装）を実地調査した
結果。「SystemVerilog 準拠シミュレータ」としての評価であり、本プロジェクトの目標である
Verilog-2001 サブセットとしての評価とは分けて記録する。

### 結論

このプロジェクトは **SystemVerilog(IEEE 1800) 準拠を目標にしていない**（Context 節に明記の
通り目標は Verilog-2001 サブセット）。SV 固有機能はほぼ全て未対応で、SV 準拠度としては
5% 未満。パーサに `sv-parser` を使うため SV 構文の**構文解析自体は通る**ことがあるが、
frontend lowering 段でサブセット外として弾かれる・無言スキップされる・`reg` に縮退する、
のいずれかになる。Verilog-2001 サブセットとしては M1+M2 で主要機能を実装済みで実用域にある。

### SystemVerilog 固有機能の対応状況

| 機能 | 状態 | 根拠 |
|---|---|---|
| `logic`/`bit` 型 | ❌ 非対応 | 専用分岐なし。`integer` 判定（`has_integer_type`）以外は `reg` 相当に縮退 |
| `always_ff`/`always_comb`/`always_latch` | ❌ 非対応 | `lower_always` は `always @(...)` のみ想定 |
| `typedef`/`struct`/`union`/`enum` | ❌ 非対応 | lowering に分岐なし |
| `interface`/`modport`/`class`/`package`/`import` | ❌ 非対応 | Context 節で明示的に対象外 |
| assertion（`assert`/`property`/`sequence`） | ❌ 非対応 | 同上 |
| `unique`/`priority` case、`final` block | ❌ 非対応 | 分岐なし |
| `$signed`/`$unsigned`/`$stop`/`$strobe`/ファイル I/O | ❌ 非対応 | `SysTask` 列挙に存在せず |

実装済みシステムタスクは `SysTask` 列挙 (`crates/mir/src/ir.rs`) と
`crates/sim/src/interp.rs::exec_syscall` の実装ベースで
`$display`/`$write`/`$monitor`（実体は `$display` と同一動作）/`$finish`/`$time`/
`$dumpfile`/`$dumpvars`/`$readmemh`/`$readmemb`/`$random`/`$clog2`（elaboration 時定数畳込みのみ）
の11個のみ。

### Verilog-2001 サブセットとしての対応状況（本来の評価軸）

対応済み: `module`/ANSI・non-ANSI ポート、`parameter`/`localparam`、`wire`/`reg`/`integer`、
`assign`/`initial`/`always`、`if`/`case`/`casez`/`casex`、`for` ループ、階層インスタンス、
主要演算子群、signed 演算、`function`/`task`、`generate`/`genvar`、ゲートプリミティブ、
`disable`/`fork`-`join`（いずれも M1/M2 で実装済み、上記マイルストーン節参照）。

Verilog-2001 の範囲でも未対応:
- `while`/`repeat`/`forever`（`for` のみ。`lower_loop_stmt` で `LS::For` 以外は
  `unsupported("loop statement variant")` として明示エラー）
- `**`（べき乗）演算子、式中の関数呼び出し
- `defparam`/`specify`/UDP が `lower.rs` 内の複数箇所の `_ => {}` catch-all で
  診断なく無言スキップされる（実装課題 B 節と同一問題。最優先で診断化すべき）

### 推奨

SV 対応そのものを追うより、実装課題節の推奨着手順（A2 エッジ検出 → A3/A4 monitor・`#0` →
D 連続代入 sensitivity 化 → B 無言スキップの診断化）を優先する方が費用対効果が高い。
`logic`/`always_ff` 等の SV 拡張受け入れは M2 残タスク5「SystemVerilog 拡張」として
ロードマップ上に既に位置づけられており、着手する場合は `width.rs` の
context-determined 幅推論再設計（実装課題 D 節）とセットで行う必要がある。

## $signed/$unsigned 対応と部分選択/連結バグ修正（2026-07-12、進行中）

picorv32.v（`$signed` を25箇所使用）のシミュレーションを目標とした対応。
設計書: `docs/superpowers/specs/2026-07-12-signed-support-design.md`
実装プラン: `docs/superpowers/plans/2026-07-12-signed-support.md`
作業ブランチ: `feat/signed-impl`

### 完了（コミット済み）

- **Task 1**: `signed_cast` 統合テストケース追加（`tests/integration/cases/signed_cast/`）。
  期待値は iverilog 実出力から作成。TDD アンカーとして**意図的に FAIL 状態**
  （符号拡張の実装完了で全行一致する設計）
- **Task 2**: `$signed`/`$unsigned` のパイプライン貫通。`SysFuncKind::Signed/Unsigned`
  追加、frontend パース（引数1個検証）、elab は inner のトップ `Expr` を
  `alloc_expr_signed` で複製登録（アプローチB: MIR ノード追加なし）
- **Task 2a**: 連結 lowering のバグ修正。`{imm[11], x}` で全子孫走査により
  インデックス式が parts に混入していた（`P::Concatenation` を直接の子のみ走査に修正）
- **Task 2b**: RHS 式の部分選択を実装。`imm[10:5]` が**全ビット値に化けていた**
  （式コンテキストの `PartSel` lowering が未実装だった）。`ConstantRange` を
  `HirExpr::PartSel` へ lowering、`+:`/`-:` は明示エラー

### 作業中に発見した既存バグ（$signed とは独立、いずれも黙って誤値になる）

| # | 問題 | 状態 |
|---|---|---|
| 1 | RHS 式の部分選択が全ビット値に化ける | ✅ 修正済み（Task 2b） |
| 2 | 連結 parts にネストした式が混入 | ✅ 修正済み（Task 2a） |
| 3 | LHS 部分選択 `q[31:20] <= x` が全ビット代入になる | ⏳ Task 2c（frontend のみ、elab/sim は対応済み） |
| 4 | LHS 連結 `{a,b} <= x` が先頭要素のみ（F 節既知） | ⏳ Task 2d（HIR/MIR に `LValue::Concat` 追加＋sim 分割書き込みが必要） |

### 残タスク

- **Task 2c**: LHS 部分選択（frontend `lower_var_lvalue` の part-select 対応）
- **Task 2d**: LHS 連結（HIR/MIR `LValue::Concat`、sim `write_lvalue` 分割書き込み）
- **Task 3**: 代入時の符号拡張（`write_lvalue` に `rhs_signed` 追加、NBA キューにフラグ。
  picorv32 の `decoded_imm <= $signed(...)` の本丸）
- **Task 4**: 演算オペランドの符号拡張（`apply_binop` で幅が異なる signed 同士を
  max 幅へ `extend_sign`）
- **Task 5**: picorv32.v parse/elab スモークチェック、PR 仕上げ

### 検証状態（2026-07-12 時点）

- 既存テスト全パス（iverilog 比較 8/8 含む）、リグレッションなし
- `signed_cast` テストのみ FAIL（意図的な TDD アンカー。値は全行正しく、
  符号拡張のみ未対応: 例 `cat=00000040` ← 期待 `ffffffc0`）
