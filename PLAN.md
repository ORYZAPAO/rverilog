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
| A3 | `$monitor` が `$display` と同一動作 | `sim/src/interp.rs` | **対応済み（2026-07-14）**。`$monitor`はシミュレーション全体で1つだけアクティブ（IEEE 1364通り、新規呼び出しが前の登録を置き換える）とし、`monitor_args`/`monitor_last`で登録・前回印字値を保持。呼び出し時は登録のみ行い、実際の印字は新設のmonitorリージョン（`flush_monitor`、active/inactive/NBA完全収束後・時刻前進前に1回）で、前回印字時から値が変化していた場合（初回登録直後を含む）のみ行うよう修正。iverilogとのbit-exact比較テスト`tests/integration/cases/monitor/`で検証済み |
| A4 | `#0` のリージョン順序が逆 | `sim/src/interp.rs` `run` | **対応済み（2026-07-14）**。`run()`のメインループにinactiveリージョンを新設し、同時刻(`#0`)で待っているプロセスをNBA適用より前に再開するよう修正（IEEE 1364のactive→inactive→NBA順に準拠）。iverilogとのbit-exact比較テスト`tests/integration/cases/delay0/`で検証済み |
| A5 | 64bit 超ネットへの部分書き込みが壊れている | `sim/src/interp.rs` `write_lvalue`・初期化 | ビット/部分選択パスが u64 前提。LogicVal 側は Large 対応済みなのに書き込み側が未対応 |
| A6 | 算術/比較の X 伝搬が粗い | `mir/src/logicval.rs` | 任意 1bit でも X/Z なら結果全体が X（M1 の割り切りだが IEEE より粗い。`===`/`!==` は正しくビット比較） |
| A7 | 64bit 超の乗除算・剰余が常に X | `mir/src/logicval.rs` | multi-word の mul/div/mod が未実装 |
| A8 | inout が実質 input | `elab/src/elaborate.rs` | 親→子の単方向結線のみ。双方向・tri-state・多重ドライバ解決・strength モデリングなし（Z は表現できるがネット上で解決されない） |
| A9 | 連続代入の `#delay` が無視される | `frontend/src/lower.rs` `lower_continuous_assign` | 遅延指定が黙って捨てられる。手続き文の `#delay` のみ有効 |
| A10 | 二項演算子の結合順序が壊れている（重大） | `frontend/src/lower.rs` `lower_expression`、根本原因は `sv-parser` クレート側 | **対応済み（2026-07-29）**。`sv_parser::Expression::Binary` が返す右結合の木を `lower_expression` 側で修正。`flatten_binary_chain` で木を in-order にフラットなオペランド列・演算子列へ展開し、`build_binop_tree` で IEEE 1364-2001 Table 5-4 の優先順位表（`binop_precedence`）に基づく演算子優先順位法（shunting-yard、全演算子左結合）で正しい二分木を再構築するよう変更。回帰テスト `tests/integration/cases/binop_precedence/` を追加し、iverilog実出力とbit-exact一致を確認 |
| A11 | `localparam`宣言がモジュール本体途中にあると名前解決されない | `frontend/src/lower.rs`（`parse_simple_const_expr`）、`elab/src/elaborate.rs`（param登録パス） | **対応済み（2026-07-29、`cpu_state_*`パターンの範囲。2026-07-30に複雑な定数式の残課題も解消）**。宣言位置は無関係で、実際の原因は2つ: (1) `lower_localparam`が値をraw text経由の簡易パーサ`parse_simple_const_expr`で解釈しており、`8'b01000000`のようなサイズ付き基数リテラルを認識できず識別子（`Expr::Net`）として誤扱いしていた（elabで`UnresolvedName`エラーとなり黙って登録スキップ）。(2) 修正後も、名前解決されたparam/localparamの参照が`lower_expr`の`HirExpr::Net`アームで常に32bit固定（`LogicVal::new(32, val, 0)`）で復元されていたため、8bit `state`レジスタとのcase比較（`case_eq`は幅不一致だと即ZERO）が常にdefault分岐に落ちていた。`scope_params`にvalとwidthのタプルを保持し、宣言側HIR式（`Const`/`SignedConst`直書きの場合のみ）から幅を推定する`hir_const_width`を追加して解決。回帰テスト`tests/integration/cases/localparam_midmodule/`（picorv32.vと同じ「alwaysブロックの後でone-hot状態localparamを宣言→case文で比較」パターン）でiverilogとのbit-exact一致を確認。**2026-07-30追加対応**: `localparam integer irqregs_offset = ENABLE_REGS_16_31 ? 32 : 16;`等、三項演算子・`\|\|`・`*`・括弧を含む複雑な定数式が`parse_simple_const_expr`で解釈できず`unresolved net/param`になっていた残課題（`WITH_PCPI`/`regfile_size`/`regindex_bits`/`irqregs_offset`）を解消。原因は専用パーサの不在ではなく配線漏れ——localparam/parameter宣言値は文法上すでにsv-parserの`ConstantParamExpression`としてパースされており、genvar/generate文脈向けに実装済みの`lower_constant_expr`/`lower_constant_primary`（Bin/Un/Ternary対応）が既に存在していたが、localparam宣言・parameterポートのデフォルト値・インスタンスのparam override計5箇所がこれを使わず`parse_simple_const_expr`に丸投げしていた。`lower_constant_param_expr`/`lower_param_expr`等のブリッジ関数を新設して5箇所を配線し直し、`lower_constant_expr`のCE::BinaryアームにA10と同型の優先順位バグがあったためこちらも`flatten_constant_binary_chain`で修正、elabの`eval_const_hir_with`に`LogAnd`/`LogOr`/`CaseEq`/`CaseNe`/`BitNand`/`BitNor`/`BitXnor`の畳み込みを追加。回帰テスト`tests/integration/cases/localparam_const_expr/`でiverilogとのbit-exact一致を確認。picorv32.vスモークチェックで`unresolved net/param`警告が0件になったことを確認（パース8モジュール成功・elaboration完了）。**未解決の残課題**: elaboration完了後のフルシミュレーションが`--max-time`指定でも停止せずハングする（A10・A11とは独立の未特定原因、次の課題） |
| A12 | `always @*` が完了後にイベント待ちへ戻らず無限ビジーループする（重大） | `sim/src/interp.rs`（`exec_proc`の`StepResult::Done`ハンドリング） | **対応済み（2026-08-03）**。`always`プロセス完了時のハンドリングが`Sensitivity::Items`かつ非空の場合のみ`event_waiters`に登録してreturnしており、`Sensitivity::All`（`always @*`）はどの分岐にもマッチせず素通りして本体を無条件に即再実行し続ける完全なビジーループになっていた（トリガー条件を一切見ない）。picorv32.vのようにcombinational `always @*`を含む設計で確実にハングする直接原因だった。デバッグ計装（`eval_expr`呼び出し数カウンタ等）で実際に無限ビジーループしていることを確認。修正は2段構え: (1) elaboration時（`elab/src/elaborate.rs`の`collect_sensitivity_stmt`/`collect_sensitivity_expr`/`collect_sensitivity_lvalue`を新設）に`always @*`本体のStmt/Expr木を再帰走査し、右辺値として読み出されるネット集合を収集して`Sensitivity::Items`へ変換（自動センシティビティリスト化、収集不能時は`Sensitivity::All`へフォールバック）。(2) `exec_proc`側も`Sensitivity::All`の場合（フォールバック時）に必ず`event_waiters`へ登録してreturnするよう修正（元のバグの最小修正）。付随して`eval_conts`の変化判定をRHS評価値の幅ではなくLHS書き込み後の実値比較に変更（幅差による見せかけの非収束を防止）。独立した既存バグだった`--max-time`未配線（`cli/src/cli.rs`の`max_time`フィールドがどこにも使われていなかった）も同時に修正し安全弁として機能させた。回帰テスト`tests/integration/cases/always_star/`（iverilog実出力とbit-exact一致確認済み）・`tests/integration/cases/max_time/`を追加。picorv32.vスモークチェックが0.55秒で正常終了することを確認（従来はtimeoutまでハングしていた） |

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

- ~~`sim/src/scheduler.rs`・`sim/src/systask.rs` が TODO スタブのまま死コード~~
  **対応済み（2026-08-12）**。両ファイルとも `interp.rs` に実装が重複しており
  ワークスペース全体を検索してもこの2ファイル以外から一切参照されていなかったため削除。
  `lib.rs` から該当 `pub mod`/`pub use` を除去
- ~~連続代入が計画の「NetId→プロセス逆引きテーブル」方式でなく総当たり固定点ループ
  （`interp.rs` `eval_conts`、最大 200 回打ち切り）。規模で性能劣化し、発振回路で黙って誤結果
  （最低限、打ち切り時に警告を出す）~~ **対応済み（2026-08-12）**。`always @*` の自動
  センシティビティリスト化（A12）と同じ `collect_sensitivity_expr` を再利用し、
  `ElaboratedDesign` に `cont_sensitivity: IndexMap<u32, Vec<u32>>`（net.0 → \[cont_id\]、
  cont_idは`design.conts`のインデックス）を新設。`elaborate()` で各`ContAssign`のRHS式が
  読み出すネット集合を収集して構築。sim側は `Interpreter` に `cont_dirty: VecDeque<u32>` +
  `cont_queued: Vec<bool>`（重複enqueue防止）のdirtyワークリストを追加し、
  `trigger_sensitivity` が対象ネットの実変化（`any_change`）を検知した際に該当contを
  dirty化するよう変更。`eval_conts` は全conts総当たりではなくdirtyワークリストのみを
  drainする実装に置換。無限発振（組み合わせループ、例: `assign a = ~a;`）検出のため
  pop回数上限（`conts.len() * 64`、最低1000）を設け、超過時は`eprintln!`で警告を出して
  打ち切るよう変更（従来は無警告で200回スイープ後に黙って打ち切っていた）。回帰テスト
  `tests/integration/cases/cont_loop/`（組み合わせループがハングせず警告を出しつつ
  `$finish`まで到達することを確認、iverilog比較は対象外—未定義動作に近いため）を追加。
  `cargo test --workspace`（39テスト）全通過、`samples/counter4`・`samples/fifo_sync`・
  picorv32.vスモークチェック（0.57秒、A12時点の0.55秒から性能劣化なし）で回帰なしを確認
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
- ~~連結 lvalue `{a,b} = ...` は先頭要素のみ代入され残りは無言で捨てられる（`frontend/src/lower.rs`）~~
  **対応済み（2026-07-16）**。$signed 対応作業（2026-07-12 節参照）の Task 2d として修正
- リポジトリの CLAUDE.md が空、`tests/rtl/fifo_counter.v` が未使用

### 推奨着手順

1. ~~A1: signed 対応~~ 完了（2026-07-09）
2. ~~A2: エッジ検出の IEEE 準拠化~~ 完了（2026-07-14）
3. ~~A3・A4: monitor リージョン実装 + `#0`（inactive）順序修正~~ 完了（2026-07-14、イベントループ再構成として一括対応）
4. ~~indexed part-select (`+:`/`-:`) 対応~~ 完了（2026-07-23、picorv32.v スモークチェックの
   ブロッカー解消のため）
5. ~~A10: 二項演算子の結合順序バグ~~ 完了（2026-07-29）
6. ~~A11: モジュール中盤の `localparam` 名前解決~~ 完了（2026-07-29に`cpu_state_*`パターンの範囲、
   2026-07-30に三項演算子・`||`・`*`・括弧を含む複雑な定数式（`WITH_PCPI`等）の残課題も解消）
7. ~~picorv32.v フルシミュレーションのハング原因調査~~ 完了（2026-08-03、A12参照）
8. ~~D: 連続代入の sensitivity 駆動化（scheduler.rs/systask.rs の死コード整理含む）~~
   完了（2026-08-12、D節参照）
9. B: サイレントスキップの診断化（`_ => {}` を `UnsupportedConstruct` エラーに置換）
10. E: fmt/clippy ジョブの CI 追加、負パステストの拡充
11. picorv32.v にテストベンチ・クロック生成を追加した上でのフル命令実行シミュレーション確認
    （A12解消によりelaborationからsim終了までは到達するようになったが、実際に命令を実行させる
    検証はまだ未実施）

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

## $signed/$unsigned 対応と部分選択/連結バグ修正（2026-07-12開始、2026-07-16 Task2c/2d/3/4完了）

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
| 3 | LHS 部分選択 `q[31:20] <= x` が全ビット代入になる | ✅ 修正済み（Task 2c、2026-07-16） |
| 4 | LHS 連結 `{a,b} <= x` が先頭要素のみ（F 節既知） | ✅ 修正済み（Task 2d、2026-07-16） |

### Task 2c・2d・3・4（2026-07-16 完了）

- **Task 2c**: `frontend/src/lower.rs` の `lower_var_lvalue` に part-select 分岐を追加
  （`lower_lvalue_part_select` 新設、`lower_part_select`（式版）と同型）。elab の
  `lower_lvalue`（`HirLValue::PartSelect` アーム）は元々対応済みだったため変更不要
- **Task 2d**: `hir::LValue`/`mir::LValue` に `Concat(Vec<LValue>)` を追加。
  `lower_var_lvalue` の `VL::Lvalue` アーム（従来は先頭要素のみ返す既知バグ）を修正し
  `LValue::Concat` を返すよう変更。elab の `lower_lvalue` に再帰変換アームを追加。
  sim側は新設 `lvalue_width`（lvalue の合計ビット幅を再帰計算するヘルパー）を軸に、
  `write_lvalue`（MSBから幅で切り出して各部分へ再帰書き込み）・`get_lval_val`
  （各部分の値をMSB順に concat）・`trigger_sensitivity`（合計幅で old/new を揃えてから
  各部分へ再帰分割）にそれぞれ `Concat` アームを追加
- **Task 3**: `write_lvalue` に `signed: bool` 引数を追加。`LValue::Net` アームで
  signed なら `extend_sign`、そうでなければ従来通り `resize`（切り詰め/ゼロ拡張）。
  呼び出し元4箇所（`BlockingAssign`/`NbaAssign`×2＋`ContAssign`）で
  `ElaboratedDesign::expr_signed` から signed を取得して渡す。`nba_queue` の要素も
  `(LValue, LogicVal, bool)` に変更しNBA適用時までsignedを保持
- **Task 4**: `apply_binop` で `both_signed` かつ幅が異なる場合、演算前に両辺を
  共通の最大幅へ `extend_sign` で揃えるよう修正。シフト演算（右辺は self-determined
  unsigned）と `===`/`!==`（暗黙のコンテキスト拡張をしない厳密比較）は対象外
- 新規テスト `tests/integration/cases/lvalue_select/`（LHS部分選択・LHS連結、
  blocking/nonblocking 双方）を追加、iverilog 実出力から期待値作成
- 既存の `signed_cast`/`compare_signed_cast`（Task 3・4 の TDD アンカー）が
  新規追加なしで FAIL→PASS に変化することを確認

### Task 5（2026-07-23 実施）

picorv32.v が入手できたため実施。1回目の実行で `picorv32_pcpi_fast_mul` モジュール内の
indexed part-select（`next_rd[j +: CARRY_CHAIN]`等）が `Unsupported construct` で
パースを止めていたことが判明し、`+:`/`-:` indexed part-select 対応を実装（下記
「indexed part-select 対応」節参照）。実装後の再実行結果は以下の通り:

- パースは8モジュールすべて成功（indexed part-selectのブロッカーは解消）
- elaboration も `picorv32` トップまで到達するが、モジュール中盤で宣言された
  `localparam`（`cpu_state_fetch`等）が名前解決できず大量の `unresolved net/param` 警告
  → **新規発見 A11**（PLAN.md 実装課題A節参照）
- シミュレーション開始後、`--max-time 10` を指定してもハングし停止しない
  （state machine の case 文がすべてX比較になるため、A11が原因と推定、未確定）
- 調査の過程で **A10（二項演算子の結合順序が壊れている、重大）** を発見。
  `a-b+c`や`a*b+c`のような3項以上の演算子チェーンが優先順位・結合則を無視して
  常に右結合で評価される、`sv-parser`クレート由来の不具合。picorv32.vのような
  実RTLでは高確率で誤動作すると見られる

Task 5自体（parse/elabスモークチェック）は「エラー内容を明らかにする」という目的は
達成したが、A10・A11という新たな重大課題が見つかったため、picorv32.vのフル
シミュレーションはまだ未達成。次の一手はユーザーと相談（A10優先が妥当と推測: 影響範囲が
広く、算術式を含む既存の全機能の正しさに関わるため）。

### indexed part-select (`+:` / `-:`) 対応（2026-07-23 実装）

Task 5のブロッカー解消のため実装。`DynBitSelect`（既存の動的1bitビット選択）と同型の
パターンで、「base は実行時式、width は定数」の part-select を追加:

- HIR: `Expr::IndexedPartSel`/`LValue::IndexedPartSelect`（net, base式, width式, plus_dir）
- MIR: `Expr::DynPartSel`/`LValue::DynPartSelect`（NetId, base ExprId, 定数width, plus_dir）
- frontend: `lower_part_select`/`lower_lvalue_part_select` の `IndexedRange` 明示エラー
  分岐を実装に置換
- elab: `lower_expr`/`lower_lvalue` に新アーム追加、baseは定数畳み込みせずExprIdのまま
  伝搬、widthのみ`eval_const_hir`で確定。`compute_expr_signed`にself-determined
  unsignedとして追加
- sim: `eval_expr`/`write_lvalue`/`lvalue_width`/`get_lval_val`/`trigger_sensitivity`に
  新アーム追加。範囲外アクセス（負のbase、ネット幅超過）は既存の`PartSelect`と同水準の
  簡略化（読み出しは全体X、書き込みは無視）
- `LogicVal::x_of_width`ヘルパーを新設（範囲外アクセス時の正しい幅のX値生成用）
- 新規テスト `tests/integration/cases/indexed_part_select/`（RHS `+:`/`-:`、LHS
  `+:`/`-:`のblocking/nonblocking、実行時変数をbaseに使用）。iverilog実出力から
  期待値作成
- テスト作成中にA10（二項演算子結合順序バグ）を発見したため、`i*8+7`のような
  「乗算の後に加算」の順序を避け`7+i*8`（数学的に等価、バグの影響を受けない順序）で
  記述した

### 検証状態（2026-07-23 時点）

- `cargo build --workspace` / `cargo test --workspace` 全通過（新規
  `test_indexed_part_select`/`compare_indexed_part_select` 含む、既存テストへの回帰なし）
- `signed_cast`/`compare_signed_cast` は引き続きPASS
- `samples/counter4`・`samples/fifo_sync`（M1受入れサンプル）を実行し VCD 生成・
  正常終了を確認（回帰なし）
- picorv32.v: パース成功（indexed part-selectブロッカー解消）、ただしA10・A11により
  フルシミュレーションは未達成（上記Task 5節参照）
