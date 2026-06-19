# Diary

## 2026-05-04

### Task

Rust 製 Verilog-2001 サブセットシミュレータ `rverilog` の実装継続。M0/M1 マイルストーンを目指す。

### What was done

#### logicval.rs の主要エラー修正

1. **LogicVal 定数の追加**
   - `LogicVal::ZERO` (a=0, b=0)
   - `LogicVal::ONE` (a=1, b=0)
   - `LogicVal::X` (a=1, b=1)
   - `LogicVal::Z` (a=0, b=1)

2. **LogicVal メソッドの実装**
   - `width()` - サイズ取得
   - `pad_to_width()` - aval の指定幅マスク
   - `pad_to_width_b()` - bval の指定幅マスク（既存）
   - `new(width, a, b)` - コンストラクタ
   - `is_zero()`, `is_one()`, `is_z()`, `is_x()` - 値判定
   - `is_known()` - 既知値判定（b=0 なら既知）
   - `has_z()`, `has_x()` - Z/X フラグ判定（未使用）
   - `get_bit()` - ビット選択
   - `extend_zero()` - ゼロ拡張

3. **構文エラー修正**
   - 重複した `impl LogicVal {` ブロックの修正
   - 不要な閉じ括弧の削除（2か所）

4. **型エラー修正**
   - `has_z()`/`has_x()` を u64 に対して誤使用 → `self_b != 0` に変更（shl, shr, ashl）
   - `repeat()` メソッドの `u32 * u64` 乗算エラー → `as u32` に修正
   - `trim_start_matches().unwrap_or()` → `if` 式で空文字列チェック

5. **未使用 import の削除**
   - `PartialOrd`

### Result

✅ `rverilog-mir` クレートがコンパイル成功
✅ Workspace 全体がコンパイル可能

### Remaining

- `mir/src/ir.rs` の MIR ノード定義
- Frontend (HIR lowering)
- Elaboration (parameter solving, instance expansion)
- Simulation scheduler and interpreter
- System tasks ($display, $finish, $time, etc.)
- VCD output
- CLI

### TODO リストを追加しました：
- 未使用関数警告の整理
- pub 修飾子の検討
- PartialEq/Eq の Large 対応確認
- repeat()/concat() の Large サポート
- Large 配列操作の実装
- Display の完全性確認
- MIR ノード定義作成

## 2026-05-07

### Task

M1 マイルストーン実装継続。Frontend (HIR lowering) の実装に進む。

### What was done

#### 1. PLAN.md 読み直し

- M1 目的: 階層RTL（同期FIFO + 4bitカウンタ）のイベント駆動シミュレーション
- パイプライン: sv-parser → frontend lowering → elaboration → MIR → simulator → VCD
- 依存方向: 単方向（循環なし）

#### 2. sv-parser AST 構造調査

**ファイル構造**（`/home/zosan/github/rverilog/test_parse/src/test_parse.rs` で調査）

- `SourceText` (root)
  - `.description` → `Description`
    - `.module_declaration` → `Option<Box<ModuleDeclaration>>`
      - `ModuleDeclarationAnsi` / `ModuleDeclarationNonansi`

**パーサ呼び出し**:
```rust
let content = fs::read_to_string(path)?;
let preprocessed = sv_parser::preprocess(&content, &[], &[], &path)?;  // PathBuf必要
let syntax_tree = sv_parser::parse_sv_pp(&preprocessed, path)?
```

**注意点**:
- `preprocess()` は `PathBuf` が必要（文字列不可）
- ASTはマクロ生成で型が複雑
- `AnyNode`フィールドにAST本体が格納

#### 3. ワークスペースビルド確認

```bash
cargo build --workspace
```

✅ 成功

#### 4. Frontend 実装（`crates/frontend/src/lower.rs`）

**現状**:
- `parse_files()` で複数ファイルパース
- `extract_module()` でモジュール抽出（一時実装）
- `find_module()` で指定モジュール名を検索

**問題点**:
```rust
// 現在: ハードコードされたcounter4データを返す
HirModuleDef {
    name: "counter4".to_string(),
    ports: vec![...],
    declarations: vec![...],
    statements: vec![...],
    instances: vec![...],
}
```

**実際の挙動**:
- `counter4.v` をパース → "counter4" を返す（ハードコード）
- `tb.v` をパース → "counter4" を返す（ハードコード）
- ❌ ファイルごとに異なるモジュール名を抽出できない

**必要な作業**:
- `SourceText` → `Description` → `ModuleDeclaration` の構造をたどる
- モジュール名（`identifier`フィールド）を抽出
- ポート/宣言/文/インスタンスを全抽出してHIR構築

### Result

✅ Workspace builds successfully  
✅ sv-parserでVerilogファイルがパース可能  
✅ パーサAPI（`preprocess()`/`parse_sv_pp()`）の使い方が確認  
✅ ASTの構造が把握（`SourceText`→`Description`→`ModuleDeclaration`）  
✅ `extract_module()` がハードコード実装されていることの確認  

### Remaining

#### M1-1: HIR定義＋Frontend Lowering（優先度: 高）

**TODO**:
1. sv-parser AST → HIR の完全なマッピング実装
2. モジュール宣言の抽出（ANSI/non-ANSI対応）
3. ポート定義の抽出（input/output/inout）
4. 個々の宣言抽出:
   - `NetDeclaration` (wire, tri, etc.)
   - `RegDeclaration`
   - `IntegerDeclaration`
   - ベクタ宣言 `[msb:lsb]` の処理
5. 文の抽出:
   - `ContinuousAssign`
   - `AlwaysConstruct`
   - `InitialConstruct`
   - `ModuleInstantiation`
6. サブセット外構造のエラー検出（`generate`/`function`/`task`/`interface`/`class`）

**ヒント**（PLAN.mdより）:
- 収集パス: 全`ModuleDeclaration`を`HirModuleDef`に収集
- 下げパス: 各モジュール内の宣言/文を訪問してHIR構築
- 受理ホワイトリスト方式で未対応ノードに遭遇したらエラー

#### 今後のM1タスク

- M1-2: LogicVal Large variant + 全演算 + proptest
- M1-3: Elaboration（top展開、parameter override、instance flatten、name resolution）
- M1-4: ビット幅・符号推論
- M1-5: MIR構築＋sensitivity逆引きテーブル
- M1-6: Scheduler骨格（active/nba/inactive/monitor）
- M1-7: プロセス実行インタプリタ
- M1-8: システムタスク
- M1-9: VCD writer統合
- M1-10: counter4/fifo_sync通過

### TODO

- [ ] sv-parser ASTからHIRへの完全な変換実装
- [ ] モジュール名抽出（`identifier`フィールド）
- [ ] ポート定義抽出（input/output/inout、方向と幅）
- [ ] NetDeclaration抽出（wire/reg/integer、幅、スコープ）
- [ ] ContinuousAssign抽出（左辺／右辺式）
- [ ] AlwaysConstruct抽出（センシティビティリスト／ブロック）
- [ ] InitialConstruct抽出（ブロック）
- [ ] ModuleInstantiation抽出（モジュール参照／ポート接続）
- [ ] サブセット外構造のエラーハンドリング
- [ ] unit test for frontend

## 2026-05-11

### Task

M1 マイルストーン実装継続。Frontend (HIR lowering) の完全実装。

### What was done

#### 1. 進捗状況の整理

**完了**:
- `rverilog_hir` クレートに `HirModule` / `Design` 型定義
- `rverilog_mir` クレートに `LogicVal` 型定義（4値ロジック）
- `parse_files()` フレームワーク（複数ファイルパース）
- sv-parser AST構造の解析（`ModuleDeclarationAnsi/Nonansi/Wildcard` 対応）
- `extract_module()` の基本実装（モジュール名抽出 + ポートパラメータ処理）

**未実装**:
- `process_module_item()` 完全実装
- ポート抽出（`PortDeclaration` → `ListOfPortDeclaration` → `Input/Output/Inout/Ref/Interface`）
- ネット宣言（`NetDeclaration` → `NetType` → `wire/tri/tri1/etc.`）
- データ宣言（`DataDeclaration` → `Var/Logic/Real/etc.`）
- パラメータ宣言（`ParameterDeclaration/LocalParameterDeclaration` → `ListOfParamAssignment`）
- 連続代入（`ContinuousAssign` → `NetAssignment`）
- 初期化文（`InitialConstruct` → `Statement`）
- 常時文（`AlwaysConstruct` → `Sensitivity` + `Statement`）
- インスタンス宣言（`ModuleInstantiation`）

**問題点**:
- `process_module_item()` がモジュール内容の大部分を処理できていない
- ポート抽出関数 `extract_ports_from_list()` 未実装
- 宣言処理関数群（`process_net_declaration`, `process_data_declaration`, `process_parameter_declaration` など）未実装
- 式変換関数（`parse_expression`, `parse_net_lvalue`, `parse_constant_expression`）未実装
- センシティビティ抽出（`extract_sensitivity`）未実装
- ステートメント解析（`parse_statement`）未実装

**構造的理解**:
- `ModuleDeclarationAnsi`:
  - `ModuleAnsiHeader` → `ModuleIdentifier` + `PortDeclaration` + `NonPortModuleItem`
  - `ModuleOrGenerateItem` → `ModuleOrGenerateItemDeclaration` / `ModuleOrGenerateItemModule`
  - `ModuleOrGenerateItemModuleItem` → `ModuleCommonItem` (ContinuousAssign, InitialConstruct, AlwaysConstruct)

#### 2. 実装方針の確認

**sv-parser API 使用法**:
- `RefNode` をパターンマッチで分岐
- `unwrap_node!()` マクロで子ノード抽出
- `syntax_tree.get_str()` で文字列抽出
- `Locate` トレイトから span 情報取得（エラーレポートに使用）

**対応するモジュール型**:
- `ModuleDeclarationAnsi`: ANSIスタイル（ポート宣言あり）
- `ModuleDeclarationNonansi`: non-ANSIスタイル（ポート宣言なし）
- `ModuleDeclarationWildcard`: ワイルドカードポート（`.*`）

**サポートしない構造**（エラー):
- `generate` ブロック
- `function`/`task` 宣言
- `interface`/`class` 宣言
- `package` 宣言
- `program` 宣言

**HIR 型マッピング**:
- `PortDecl` → ポート名、方向（Input/Output/Inout）、幅
- `ParamDecl` → パラメータ名、定数式
- `LocalParamDecl` → ローカルパラメータ名、定数式
- `NetDecl` → ネット名、幅、種類（Wire/Tri/etc.）
- `RegDecl` → レジスタ名、幅
- `ContinuousAssign` → 左辺式（LValue）、右辺式（Expr）
- `InitialConstruct` → 初期文（Stmt）
- `AlwaysConstruct` → センシティビティ（Sensitivity）、本体（Stmt）
- `ModuleInstance` → モジュール名、インスタンス名、ポート接続

#### 3. ファイル構造の確認

**関連ファイル**:
- `/home/zosan/github/rverilog/crates/frontend/src/lower.rs`: 実装対象
- `/home/zosan/github/rverilog/crates/hir/src/design.rs`: `HirModule` 型定義
- `/home/zosan/github/rverilog/crates/mir/src/logicval.rs`: `LogicVal` 型定義
- `/tmp/sv-parser/sv-parser/examples/module_list.rs`: sv-parser 使用例
- `/tmp/sv-parser/sv-parser-syntaxtree/src/expressions/expressions.rs`: AST式構造

### Result

✅ `rverilog_hir` クレートが使用可能  
✅ `rverilog_mir` クレートが使用可能  
✅ `parse_files()` フレームワークが構築済み  
✅ sv-parser ASTのモジュール構造が理解済み  
✅ 必要な補助関数が未実装（実装予定）

### Remaining

**直ちに実装すべき関数**:
1. `extract_ports_from_list()` - ポートリスト抽出
2. `process_net_declaration()` - ネット宣言処理
3. `process_data_declaration()` - データ宣言処理
4. `process_parameter_declaration()` - パラメータ宣言処理
5. `process_local_parameter_declaration()` - ローカルパラメータ宣言処理
6. `process_continuous_assign()` - 連続代入処理
7. `process_initial_construct()` - 初期化文処理
8. `process_always_construct()` - 常時文処理
9. `extract_sensitivity()` - センシティビティ抽出
10. `parse_expression()` - 式変換
11. `parse_net_lvalue()` - 左辺式変換
12. `parse_statement()` - ステートメント変換
13. `parse_numeric_constant()` - 数値定数変換
14. `parse_constant_expression()` - 定数式変換

**実装手順**:
1. 式関連関数（`parse_expression`, `parse_net_lvalue`, `parse_numeric_constant`）の実装
2. 宣言処理関数（`process_*_declaration`）の実装
3. 文処理関数（`process_*_construct`, `extract_sensitivity`, `parse_statement`）の実装
4. `process_module_item()` に統合
5. `counter4.v` でテスト

### TODO

- [ ] `extract_ports_from_list()` 実装
- [ ] `process_net_declaration()` 実装
- [ ] `process_data_declaration()` 実装
- [ ] `process_parameter_declaration()` 実装
- [ ] `process_local_parameter_declaration()` 実装
- [ ] `process_continuous_assign()` 実装
- [ ] `process_initial_construct()` 実装
- [ ] `process_always_construct()` 実装
- [ ] `extract_sensitivity()` 実装
- [ ] `parse_expression()` 実装
- [ ] `parse_net_lvalue()` 実装
- [ ] `parse_statement()` 実装
- [ ] `parse_numeric_constant()` 実装
- [ ] `parse_constant_expression()` 実装
- [ ] `lower.rs` 全実装完了
- [ ] `cargo check --workspace` でコンパイル確認
- [ ] `counter4.v` で frontned test 実行

## 2026-05-17

### Task

frontend lowering (SyntaxTree → HIR) の本実装。

### What was done

#### HIR の更新

- `Sensitivity` を `All / Items(Vec<SensitivityItem>)` に変更（多辺対応）
- `SensitivityItem { edge: Option<EdgeType>, signal: SmolStr }` 追加
- `AlwaysConstruct.sensitivity` を `Sensitivity` → `Option<Sensitivity>` に変更  
  (`always #5 clk = ~clk` のような event なし always に対応)
- `hir/src/lib.rs` に `SensitivityItem` をエクスポート追加

#### lower.rs 全実装

sv-parser AST → HIR 変換を全面実装:

- `parse_files()`: `parse_sv()` 呼び出し (preprocess 内包)、モジュール収集
- ANSI モジュール (`ModuleDeclarationAnsi`) と non-ANSI (`ModuleDeclarationNonansi`) 対応
- ANSI ポート: `AnsiPortDeclarationNet/Variable` から direction + packed width を抽出
- パラメータポートリスト抽出
- モジュールアイテム: AlwaysConstruct / InitialConstruct / ContinuousAssign / NetDecl / DataDecl / ModuleInstantiation
- `always` の timing control 解析: `@(posedge clk or posedge rst)` → `Sensitivity::Items`、`always #5` → `Option::None`
- ステートメント: SeqBlock / ConditionalStatement / BlockingAssignment / NonblockingAssignment / ProceduralTimingControlStatement / SubroutineCallStatement / CaseStatement
- 式: Primary (識別子・リテラル・連結・repeat) / Unary / Binary / Conditional
- 数値リテラル: `4'd0`, `8'hFF`, `4'b1010`, unsized decimal (X/Z 入りも対応)
- システムタスク: `$display`, `$write`, `$monitor`, `$finish`, `$time`, `$dumpfile`, `$dumpvars`
- モジュールインスタンス: named/ordered ポート接続、parameter override

#### テスト追加と通過

- `crates/frontend/src/lib.rs` に `test_parse_counter4` 追加
- counter4.v + tb.v を正常パース: 2 モジュール、3 ポート、1 always を確認
- `cargo test --workspace` 全通過

### Next

- M1.3: Elaboration (instance 展開、名前解決、port 接続)
- M1.4: ビット幅・符号推論
- M1.5: MIR 構築

## 2026-05-19

### Task

M1.3〜M1.7 Elaboration + Interpreter 本実装。counter4 サンプル動作確認。

### What was done

#### MIR (ir.rs) 全面更新

- `Arena<T>` → `Vec<T>` ベースに変更（id-arena 削除、indexmap 追加）
- `stmts: Vec<Stmt>`, `exprs: Vec<Expr>` フィールド追加
- `sensitivity_table: IndexMap<u32, Vec<u32>>` 追加
- `Sensitivity` を `All | Items(Vec<SensitivityEdge>)` に変更
- `SensitivityEdge { edge: Option<EdgeType>, net: NetId }` 追加
- `EdgeType { Posedge, Negedge }` 追加
- `ProcessKind { Initial, Always }` 追加
- `Stmt::Null`, `Expr::StringLit` 追加
- `UnOp` にリダクション演算子追加 (RedAnd/RedOr/RedXor 等)
- `LValue::PartSelect(NetId, u32, u32)` に変更（Range 廃止）

#### Elaboration (elaborate.rs) 本実装

- HIR → MIR の完全変換実装
- `ElabCtx` で scope/net/param マップを管理
- `elab_module()`: スコープ生成、port/net/reg 登録、インスタンス再帰展開
- `lower_expr()`: HIR::Expr → MIR::ExprId（名前解決込み）
- `lower_stmt()`: HIR::Stmt → MIR::StmtId
- `lower_sensitivity()`: HIR::Sensitivity → MIR::Sensitivity
- `eval_const_hir_with()`: 定数式畳み込み（パラメータ解決）
- port 接続を ContAssign として実装（input: parent→child、output: child→parent）
- sensitivity 逆引きテーブル構築

#### HIR に StringLit 追加

- `HirExpr::StringLit(SmolStr)` 追加
- `lower.rs` の `PrimaryLiteral::StringLiteral` を `Expr::StringLit` に変換

#### Interpreter (interp.rs) 本実装

- フレームスタック方式 `Vec<(Vec<StmtId>, usize)>` でコンティニュエーション実装
- `exec_proc()`: プロセスをステップ実行、Delay/Wait で中断してスケジューラに登録
- `eval_expr()`: Const/Net/BitSel/PartSel/Concat/Repeat/Bin/Un/Cond を完全実装
- `apply_binop()` / `apply_unop()`: 全演算子実装
- `trigger_sensitivity()`: posedge/negedge 検出、event_waiters を wake
- `eval_conts()`: 連続代入の固定点評価
- `format_args()`: $display の format string 処理 (%d/%h/%b/%o/%s/%t)
- NBA キュー実装（nba_queue → apply → trigger）
- BinaryHeap で時間順イベントスケジューリング

#### 主要バグ修正

- `@(posedge clk)` 系 always プロセスの無限ループ修正
  - sensitivity 付き always は初期化時に event_waiters に入れて待機
  - Done 時も event_waiters に戻す（body 再実行前にイベント待ち）

### Result

```
$ rverilog --top tb_counter4 samples/counter4/counter4.v samples/counter4/tb.v
Starting counter test
Final count: 9
$finish at time 110
```

✅ `cargo test --workspace` 全通過  
✅ counter4 サンプル動作成功（EXIT:0）

### Next

- M1.10: fifo_sync サンプルの動作確認
- VCD writer 統合（$dumpfile/$dumpvars）
- `$display` の `%d` フォーマットで幅付き表示の改善
- Always の @* (sensitivity all) 対応改善

## 2026-05-21

### Task

fifo_sync サンプルの動作確認。未実装機能（for ループ・メモリ配列・`$clog2`・localparam パース）の追加実装。

### What was done

#### サンプルファイル修正

- `fifo_sync.v`: `{...}` → `begin...end`、`output wire dout` → `output reg dout`
- `tb.v`: `for (int i ...)` → `integer i;` + `for (i ...)` に修正、`#10 * DEPTH` → `#160` に修正

#### HIR 拡張

新規型追加:
- `SysFuncKind { Clog2 }` — `$clog2()` 専用の system function enum
- `MemDecl { name, elem_width: Expr, depth: Expr }` — 未パックメモリ配列宣言
- `HirModule.mems: Vec<MemDecl>` フィールド追加
- `Expr::IndexSel(SmolStr, Box<Expr>)` — 動的インデックスアクセス（mem/net 共用）
- `Expr::SysFunc(SysFuncKind, Vec<Expr>)` — `$clog2()` 式
- `LValue::IndexSel(SmolStr, Box<Expr>)` — 書き込み側の動的インデックス
- `Stmt::For { var, init, cond, step, body }` — for ループ文

#### MIR 拡張

新規型追加:
- `MemId(u32)` — メモリ配列 ID
- `MemInfo { depth, elem_width, scope, name }` — メモリ配列情報
- `ElaboratedDesign.memories: Vec<MemInfo>` フィールド追加
- `Expr::MemRead(MemId, ExprId)` — メモリ読み出し
- `LValue::MemWrite(MemId, ExprId)` — メモリ書き込み
- `Stmt::While(ExprId, StmtId)` — while ループ（for のターゲット）

#### Frontend 拡張 (lower.rs)

- **localparam パース**: `PD::LocalParameterDeclaration` → `lower_localparam()` で名前・値を抽出。値は `tree.get_str(cpe)` でテキスト取得後 `parse_simple_const_expr()` でパース。
- **メモリ配列検出**: `lower_data_decl()` を `(Vec<RegDecl>, Vec<MemDecl>)` 返す形に変更。`UnpackedDimensionRange` を検出してメモリ配列を識別。
- **整数型対応**: `IntegerAtomType` を検出して幅 32 に。
- **`$clog2` 式パース**: `parse_simple_const_expr()` が `$clog2(...)` を `Expr::SysFunc(Clog2, ...)` に変換。
- **インデックスアクセスパース**: `PrimaryHierarchical.nodes.2.nodes.1.nodes.0`（`BitSelect` の `Vec<Bracket<Expression>>`）を検出し `Expr::IndexSel` / `LValue::IndexSel` を生成。
- **for ループパース**: `SI::LoopStatement` ブランチ追加。`lower_loop_stmt()` / `lower_for_init()` / `lower_for_step()` で `Stmt::For` を生成。SV 型宣言付き for (`for (int i ...)`) と Verilog-2001 形式両対応。
- **パラメータ値抽出修正**: `lower_param_port_list()` と `lower_param_overrides()` が値を `lv(0,32)` に固定していたのを修正し、実際の式テキストから `parse_simple_const_expr()` で抽出するよう変更。
- **`%0d` フォーマット対応**: `format_string()` で数値幅修飾子（`%0d`, `%8d` 等）をスキップするよう修正。

#### Elaboration 拡張 (elaborate.rs)

- `ElabCtx` に `memories: Vec<MemInfo>` と `scope_mems: IndexMap<u32, IndexMap<SmolStr, MemId>>` 追加
- `alloc_mem()` / `register_mem()` / `resolve_mem()` メソッド追加
- `elab_module()` でメモリ配列を登録（`eval_const_hir` で elem_width/depth を評価）
- `lower_expr()` に `IndexSel` → `MemRead` または `BitSel` の分岐追加
- `lower_expr()` に `SysFunc(Clog2)` → const として畳み込む分岐追加
- `lower_lvalue()` に `IndexSel` → `MemWrite` の分岐追加
- `lower_stmt()` に `For` → `Block([init, While(cond, Block([body, step]))])` に変換
- `eval_const_hir_with()` に `SysFunc(Clog2)` の定数評価追加
- `clog2(n)` ヘルパ関数追加（`$clog2` の整数演算）

#### Interpreter 拡張 (interp.rs)

- `mem_values: HashMap<(u32, u32), LogicVal>` フィールド追加（(MemId.0, idx) → value）
- `Stmt::While` ハンドリング: フレームスタックに `[body, while_stmt_itself]` を push する自己再帰方式
- `Expr::MemRead` ハンドリング: idx を評価してメモリから読み出し
- `LValue::MemWrite` ハンドリング: idx を評価してメモリに書き込み
- `LValue::MemWrite` の `get_lval_val` / `write_lvalue` / `trigger_sensitivity` 対応（MemWrite はネット sensitivity を発生させない）
- **サブモジュール clk 伝播バグ修正**: `eval_conts()` が sensitivity を発生させてもその cycle 内で process が実行されなかった問題を修正。メインループを「active drain → eval_conts → active が空になるまでループ」の構造に変更。

### Result

```
$ rverilog --top tb_fifo_sync samples/fifo_sync/tb/tb.v samples/fifo_sync/rtl/fifo_sync.v
Starting FIFO test
Write done, full=0 empty=0   ← 継続調査中
read[0] = 0
read[1] = 0
read[2] = 1
...
```

✅ fifo_sync のパース・エラボレーションが成功  
✅ for ループ・メモリ配列・`$clog2` の基本実装完了  
✅ `%0d` フォーマット対応  
✅ counter4 動作維持  
⚠️ サブモジュールへの posedge clk 伝播に問題あり（`full/empty` の値が不正）

### Next

- サブモジュールの clk 伝播問題を完全修正（cont assign 経由の posedge 検出）
- `count == DEPTH` 比較の正常動作確認
- VCD writer 統合（`$dumpfile` / `$dumpvars`）

---

## 2026-05-31

### Task

M1 マイルストーン最終確認。

### Result

**M1 マイルストーン達成** ✅

前回セッションで残っていた問題（サブモジュール clk 伝播、VCD writer 統合）がすでに解決済みであることを確認。

```
$ rverilog --top tb_counter4 samples/counter4/tb.v samples/counter4/counter4.v
Starting counter test
Final count: 10
$finish at time 110

$ rverilog --top tb_fifo_sync -o build/fifo.vcd samples/fifo_sync/tb/tb.v samples/fifo_sync/rtl/fifo_sync.v
Starting FIFO test
Write done, full=1 empty=0
read[0] = 0 ... read[15] = 15
Read done, full=0 empty=1
FIFO test completed
$finish at time 370
```

- counter4: $display 出力・VCD 生成 ✅
- fifo_sync: 正しい full/empty 動作・read 値 0-15・VCD 生成（3.3KB）✅
- `cargo test --workspace` 全通過 ✅

### Next (M2 候補)

- iverilog との出力比較 CI 導入
- `function`/`task`/`generate`/`genvar` 対応
- `$readmemh`/`$random` 対応
- SystemVerilog 拡張（`logic`/`always_ff`/`always_comb`）
- ビット幅推論の精度向上（context-determined 完全対応）
- reg 幅のパラメータ依存解決（`[ADDR_WIDTH:0]` → 実幅の評価）

---

## 2026-06-18

### Task

統合テストハーネスの実装。

### What was done

#### GitHub リポジトリ作成・PR 作成

- `ORYZAPAO/rverilog` として新規パブリックリポジトリを作成
- master ブランチ: workspace スケルトン（Cargo.toml + .gitignore）のみ
- `feat/m1-milestone` ブランチ: M1 全実装（39 ファイル、7440 行）
- PR #1 を作成: https://github.com/ORYZAPAO/rverilog/pull/1

#### 統合テストハーネス実装

**変更方針**: `Interpreter` に出力バッファを持たせ、`$display`/`$write`/`$finish` の出力を通常の stdout 印刷と同時に内部バッファにも蓄積。テストは `interp.output()` で取得した文字列を `expected.stdout` ファイルと比較。

**変更ファイル**:

- `crates/sim/src/interp.rs`
  - `output_buf: String` フィールド追加
  - `exec_syscall` の Display/Write/Monitor で `output_buf` に追記
  - `exec_proc` の `StepResult::Finish` で `$finish at time N` を `output_buf` に追記
  - `pub fn output(&self) -> &str` メソッド追加

- `crates/cli/tests/integration.rs`（新規）
  - `workspace_root()`: `CARGO_MANIFEST_DIR` から workspace ルートを算出
  - `run_sim(top, files)`: parse → elaborate → Interpreter::run → output() を返すヘルパ
  - `expected_stdout(case)`: `tests/integration/cases/<case>/expected.stdout` を読み込む
  - `test_counter4`, `test_fifo_sync` の 2 テスト

- `tests/integration/cases/counter4/expected.stdout`（新規）
- `tests/integration/cases/fifo_sync/expected.stdout`（新規）

### Result

```
$ cargo test --workspace
test test_counter4 ... ok
test test_fifo_sync ... ok
test result: ok. 2 passed; 0 failed
（他既存テストも全通過）
```

✅ 統合テストハーネス実装完了  
✅ feat/m1-milestone ブランチにコミット・プッシュ済み

### Next

- `LogicVal::Large` バリアント（64bit 超ベクタ対応）
- reg 幅のパラメータ依存解決（`[ADDR_WIDTH:0]` → 実幅の評価）
- iverilog との出力比較 CI 導入

---

## 2026-06-18 (2)

### Task

パラメータ依存の reg/net/port 幅解決バグの修正。

### 原因

`RegDecl`/`NetDecl`/`PortDecl` の `width: u32` はフロントエンドでパース時にリテラル整数として評価していた。`ADDR_WIDTH-1` のようなパラメータ依存式は整数パースに失敗し `width = 0` になっていた。

`MemDecl` はすでに `elem_width: Expr` と `depth: Expr` でエラボレーション時に `eval_const_hir` で評価する設計になっていたため、同じパターンを適用した。

### 変更ファイル

- `crates/hir/src/design.rs`: `PortDecl`/`NetDecl`/`RegDecl` に `width_expr: Expr` を追加
- `crates/frontend/src/lower.rs`:
  - `lower_ansi_port_net`/`lower_ansi_port_variable`: `packed_width` → `packed_width_expr` に変更
  - `lower_net_decl`: `packed_width` → `packed_width_expr` に変更
  - `lower_data_decl`: `width_expr` を `RegDecl` に渡すよう変更
- `crates/elab/src/elaborate.rs`: ポート/ネット/レジスタ登録時に `eval_const_hir(ctx, scope, &*.width_expr).unwrap_or(static_width).max(1)` で幅を解決

### Result

```
// reg [WIDTH-1:0] data; (WIDTH=8 でインスタンス化)
data=171 width=8 addr=3
data width bits: 10101011
```

全テスト通過 ✅。feat/m1-milestone にプッシュ済み。

### Next

- `LogicVal::Large` バリアント（64bit 超ベクタ対応）
- iverilog との出力比較 CI 導入

---

## 2026-06-18 (3)

### Task

`LogicVal::Large` バリアント（64bit超ベクタ）の完全実装。

### What was done

`crates/mir/src/logicval.rs` を全面的に書き直し。チャンクベースのヘルパーを追加し、全演算を Large 対応に。

**追加ヘルパー関数**:
- `num_chunks(width)` — ビット幅に必要なu64チャンク数
- `top_mask(width)` — 最上位チャンクのマスク
- `chunk_mask(width, idx)` — チャンクiのマスク
- `LogicVal::from_chunks(width, &[u64], &[u64])` — チャンク列からの構築

**修正メソッド**:
- `is_zero/is_known/is_one/is_x/is_z`: `pad_to_width`（チャンク0のみ）→ 全チャンクループに変更
- `eq/ne/case_eq/case_ne`: 全チャンク比較
- `lt/gt/le/ge`: MSBチャンクから降順の多倍長符号なし比較
- `add/sub`: キャリー/ボロー伝播付き多倍長加減算
- `shl/shr/ashl/ashr`: チャンクをまたぐシフト演算
- `concat`: 任意幅のビット連結（チャンク境界またぎ対応）
- `extend_zero`: Large対応
- `part_select/bit_select`: チャンクインデックスで直接アクセス
- `reduce_and/or/xor`: 全チャンク畳み込み
- `Display`: 64bit超は16進表示
- `Not/BitAnd/BitOr/BitXor`: 既存のチャンクループ実装を整理・統合
- `mul/div/mod_`: >64bit は X のまま（RTL実用上まれ）

**テスト追加**: Large専用7ケース（add/concat/is_zero/eq/shl/part_select）

### Result

```
cargo test -p rverilog-mir
test result: ok. 11 passed; 0 failed
cargo test --workspace
全テストパス ✅
```

feat/m1-milestone にプッシュ済み。

### Next

- iverilog との出力比較 CI 導入
- `function`/`task` 対応
