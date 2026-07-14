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

---

## 2026-06-19

### Task

`function`/`task` 対応の実装。

### 設計

- **HIR** (`crates/hir/src/design.rs`): `FunctionDecl`/`TaskDecl`/`TfArg` を追加。`HirModule.functions`/`tasks` フィールド追加。`Expr::Call(name, args)`（関数呼び出し式）、`Stmt::TaskCall(name, args)`（タスク呼び出し文）を追加。
- **Frontend** (`crates/frontend/src/lower.rs`): ANSI形式 (`function [W] f(input a, ...);`) と旧式 (`function [W] f; input a; ...; endfunction`) の両方をパース。`Primary::FunctionSubroutineCall` → `Expr::Call`、`SubroutineCall::TfCall` → `Stmt::TaskCall`。
- **Elab** (`crates/elab/src/elaborate.rs`): 関数/タスクごとに専用スコープ (`$func_*`/`$task_*`、親は宣言元モジュールスコープ) を確保し、引数・ローカル変数を reg として確保（呼び出し毎に再エントラントではない、非再帰モデル）。`Expr::Call` は呼び出し元スコープで引数を評価して引数regへ代入する `Stmt::BlockingAssign` 列 + 本体 `StmtId` を `Expr::CallResult(setup_stmts, ret_net)` にまとめる。`Stmt::TaskCall` は入力代入→本体→出力書き戻し（`output`/`inout` 引数は単純な net 名のみ対応）をまとめた `Stmt::Block` に展開。
- **MIR** (`crates/mir/src/ir.rs`): `Expr::CallResult(Vec<StmtId>, NetId)` を追加。
- **Interpreter** (`crates/sim/src/interp.rs`): `eval_expr`/`get_lval_val`/`format_args`/`format_string` を `&mut self` 化（式評価中に呼び出しのセットアップ文を実行できるようにするため）。`exec_sync_stmt` を新設し、`Expr::CallResult` 評価時にセットアップ文（引数代入＋関数本体）をスケジューラを介さず即時実行してから戻り値 net を読む（Verilog の関数はゼロタイムで実行される仕様に対応）。タスク呼び出しは通常の `Stmt::Block` として既存のプロセススケジューラ経由で実行されるため、本体内の `#delay`/`@event` も動作する。

### 制約

- 関数本体内の `#delay`/`@event` は無視して即時実行（仕様上関数では使用不可のため許容範囲）。
- 関数/タスクは非再帰（呼び出し毎に専用regを再利用、再入不可）。
- `output`/`inout` 引数は単純な net 名のみ書き戻し対応（部分選択や式は不可）。
- 連続代入 (`assign`) 内での関数呼び出しは未対応。

### Result

```
$ cargo test --workspace
test test_counter4 ... ok
test test_func_task ... ok   (新規)
test test_fifo_sync ... ok
全テストパス ✅
```

手動確認 (`/tmp/test_func_task.v`):
```
add8(3,4) = 7
show_sum: 3 + 4 = 7
after task r=7
$finish at time 0
```

feat/m1-milestone にプッシュ済み (e3d1b48)。

### Next

- iverilog との出力比較 CI 導入
- `generate`/`genvar` 対応
- gate primitive (`and`/`or`/`not`/`buf` 等)

---

## 2026-06-19 (2)

### Task

gate primitive (`and`/`or`/`nand`/`nor`/`xor`/`xnor`/`buf`/`not`) 対応の実装。

### 設計

- **Frontend** (`crates/frontend/src/lower.rs`): `ModuleOrGenerateItem::Gate` を新規ハンドリング。`lower_gate_inst` で `GateInstantiation::NInput`（and/nand/or/nor/xor/xnor、可変入力数）と `NOutput`（buf/not、複数出力対応）をそれぞれ `ContinuousAssign` に展開。ゲートは組合せ論理そのものなので、専用の HIR/MIR ノードを増やさずに既存の `assign` 機構へ直接変換するだけで済む。
- 否定系ゲート（nand/nor/xnor/not）は `Expr::Un(UnOp::BitNot, ...)` でラップ。
- switch/cmos/pass/pullup/pulldown 系プリミティブは本サブセットでは未対応（黙ってスキップ）。

### バグ修正: net 代入時の幅切り詰め漏れ

ゲート実装の検証中に、`nand`/`nor`/`xnor`/`not` の出力が 1bit のはずなのに 32bit 幅で表示される不具合を発見。

原因: `crates/sim/src/interp.rs` の `write_lvalue` の `LValue::Net` 分岐が、代入値を **net の宣言幅に切り詰めずにそのまま格納**していた。たとえば `reg a; ... a = 1;` の `1` は無符号化なし32bit定数のため、`a`（1bit net）に 32bit 幅の値がそのまま保存され、以後 `~a` などの演算で上位ビットのゴミがそのまま伝播していた（and/or/xor は結果が偶然0で見た目上問題が出ていなかった）。

修正: `LogicVal` に `resize(width)` メソッドを追加（`from_chunks` 経由でチャンク単位に切り詰め/ゼロ拡張）し、`write_lvalue` の `LValue::Net` 分岐で代入前に net の宣言幅へ `resize` するよう変更。`crates/mir/src/logicval.rs` / `crates/sim/src/interp.rs` を修正。

### Result

```
$ cargo test --workspace
test test_counter4 ... ok
test test_func_task ... ok
test test_gates ... ok   (新規)
test test_fifo_sync ... ok
全テストパス ✅
```

手動確認:
```
a=0 b=0 and=0 or=0 nand=1 nor=1 xor=0 xnor=1 buf=0 not=1
a=1 b=0 and=0 or=1 nand=1 nor=0 xor=1 xnor=0 buf=1 not=0
a=1 b=1 and=1 or=1 nand=0 nor=0 xor=0 xnor=1 buf=1 not=0
$finish at time 3
```

### Next

- iverilog との出力比較 CI 導入
- `generate`/`genvar` 対応
- `disable`/`fork`-`join` 対応

---

## 2026-06-20

### Task

`generate`/`genvar` 対応の実装。

### 設計

- **HIR** (`crates/hir/src/design.rs`): `GenerateItems`（nets/regs/mems/locals/assigns/initials/alwayses/instances/nested の集合）、`GenerateConstruct`（If/Case/For）、`GenerateIf`/`GenerateCase`/`GenerateFor` を追加。`HirModule.generates: GenerateItems` フィールドを追加（既存の `nets`/`assigns` 等とは別に、generate 由来の項目だけを保持）。
- **Frontend** (`crates/frontend/src/lower.rs`):
  - `NonPortModuleItem::GenerateRegion`（`generate`/`endgenerate` ブロック）と `ModuleCommonItem::LoopGenerateConstruct`/`ConditionalGenerateConstruct`（ブロックなしで直接書かれた for/if/case）の両方をハンドリング。
  - `lower_generate_items`/`process_generate_mogi`/`lower_generate_block` で再帰的に `GenerateItems` を構築（ネストした generate も `nested` 経由で再帰）。
  - genvar の境界式・ステップ式（`i < WIDTH`、`i = i + 1` 等）はテキストの場当たり的パースではなく、`ConstantExpression` 構文木を正しく辿る `lower_constant_expr`/`lower_constant_primary` を新設して二項/単項/三項演算・genvar 識別子・parameter 識別子を扱えるようにした。
  - 副作用として見つけたバグ修正: `#(parameter WIDTH = 8, parameter DEPTH = 16)` 形式（`ParameterPortList::Declaration`）のデフォルト値が `Expr::Const(0)` に固定されていた（デフォルト値の式を読まずに捨てていた）。インスタンス化時に必ず override する既存サンプルでは問題が露見していなかった。`Assignment` 形式と同様にデフォルト式をパースするよう修正。
- **Elab** (`crates/elab/src/elaborate.rs`): `elab_generate_items` を新設し、`elab_module` の最後で `hir.generates` を展開。
  - if/case はその場で条件を `eval_const_hir` で評価し、選ばれた分岐の `GenerateItems` を**同じスコープ**に登録。
  - for は genvar の値ごとに**専用の子スコープ**（`Scope { parent: Some(scope), name: "$gen_<var>_<i>", .. }`）を割り当て、genvar を `register_param` で登録。これにより本体内の幅式・instance パラメータ・assign/always の実行時式が genvar 値を `ctx.resolve_param` 経由で透過的に定数として解決できる（HIR 木を書き換える置換パスは不要）。無限ループ防止に `MAX_GENERATE_ITERS = 4096` の上限を設けた。
- **MIR/Sim**: generate-for でビット配列を per-bit instance に展開する一般的な書き方（`gate #(...) u(.a(a[i]), .o(out[i]))`）を実際に動かす過程で、出力ポート接続が `HirExpr::Net` 以外（`out[i]` のような動的ビット選択）を一切サポートしていなかったことが判明。
  - `LValue::DynBitSelect(NetId, ExprId)` を MIR に追加し、`sim/src/interp.rs` の `get_lval_val`/`write_lvalue`/`trigger_sensitivity` に実行時ビット位置での読み書きを実装。
  - `elab/src/elaborate.rs::lower_lvalue` の `HirLValue::IndexSel` を、従来の「常にビット0にフォールバック」する誤った実装から `LValue::DynBitSelect` を使う正しい実装に修正。
  - インスタンスの出力ポート接続を汎用化する `expr_as_lvalue` ヘルパーを追加し、`.o(out)` だけでなく `.o(out[i])` / `.o(out[hi:lo])` も lvalue として正しく解決できるようにした（従来は `HirExpr::Net` のみ対応で、ビット選択は黒く無視されていた）。

### 制約

- generate-for の本体は genvar ごとに新しい子スコープへフラットに展開する。Verilog 標準のような `genblk[i].foo` 階層パスは作らない（VCD 階層やデバッグ表示には影響するが、信号の参照解決自体には影響しない）。
- `lower_constant_expr` は ConstantExpression の主要なバリアント（リテラル・genvar/parameter 識別子・単項/二項/三項演算・括弧）のみ対応。`inside` 式や function call 等は未対応（エラーまたはテキストパースへのフォールバック）。
- generate ブロック内での `function`/`task` 宣言は非対応（無視）。

### Result

```
$ cargo test --workspace
test test_counter4 ... ok
test test_func_task ... ok
test test_gates ... ok
test test_generate ... ok   (新規)
test test_fifo_sync ... ok
全テストパス ✅
```

手動確認 (`tests/integration/cases/generate/dut.v`):
```
WIDTH>2 branch taken
a=1010 b=110 and=10 or=1110
$finish at time 1
```
genvar による for ループで4bit分の and/or ゲートインスタンスを生成し、各ビットが正しく a&b / a|b を計算していることを確認。generate-if / generate-case の分岐選択も正しく動作。

### Next

- iverilog との出力比較 CI 導入
- `disable`/`fork`-`join` 対応
- `$readmemh`/`$random` 対応

---

## 2026-06-20 (2)

### Task

`disable`/`fork`-`join` 対応の実装。

### 設計

- **HIR** (`crates/hir/src/design.rs`): `Stmt::NamedBlock(SmolStr, Vec<Stmt>)`（`begin : label ... end`）、`Stmt::Disable(SmolStr)`（`disable label;`）、`Stmt::Fork(Vec<Stmt>)`（`fork ... join`、各要素が並行実行する分岐）を追加。
- **Frontend** (`crates/frontend/src/lower.rs`): `lower_seq_block` がブロックのラベル（`SeqBlock.nodes.1`）の有無で `Stmt::Block`/`Stmt::NamedBlock` を切り分けて生成。`StatementItem::DisableStatement`（`DisableStatement::Block`/`Task` の両方を同じ仕組みで処理。`disable fork;` は未対応としてエラー）と `StatementItem::ParBlock`（`fork`/`join`/`join_any`/`join_none` の区別なく全分岐を `Stmt::Fork` に詰める）を新規ハンドリング。
- **Elab** (`crates/elab/src/elaborate.rs`): `ElabCtx` に `scope_blocks: IndexMap<u32, IndexMap<SmolStr, u32>>` と `next_block_id` を追加し、`register_block`/`resolve_block` で名前付きブロックに一意な `u32` ID を割り当て（他の `resolve_*` 系と同じ親スコープ遡上方式）。`lower_stmt` で `HirStmt::NamedBlock` を処理する際は **先にブロックIDを登録してから子文を lowering** することで、ブロック自身を対象とする内側の `disable label;` が解決できるようにした。`HirStmt::Disable` はこの ID を解決して `Stmt::Disable(id)` に変換、未解決ならエラー（他プロセスの名前解決は対象外）。`HirStmt::Fork` は分岐を再帰的に lowering するだけ（並行実行ロジックはsim側）。
- **Sim** (`crates/sim/src/interp.rs`): プロセス実行のフレームスタック `Frame` に `Option<u32>`（このフレームが対応する `NamedBlock` のID）を追加。
  - `Stmt::NamedBlock(id, stmts)`: フレームをラベルID付きでpush。
  - `Stmt::Disable(target)`: 現在のプロセスのフレームスタックを**末尾（最内）から**探索し、対象IDを持つフレームが見つかった位置で `truncate`。これにより、そのブロックとそれより内側のフレームが全て破棄され、外側のフレーム（ループの増分文や次の文）から実行が継続する——IEEE仕様通り「そのブロックの残り部分をスキップする」動作（forループの**1イテレーションだけ**をスキップする「continue」的挙動になる。forループ全体を止めたい場合はfor文自体ではなくbodyではない外側のブロックをラベル付けする必要がある）。対象ブロックが現在のプロセス内に見つからない場合は no-op（他プロセスのタスク/ブロックの中断は未対応）。
  - `Stmt::Fork(branches)`: 各分岐を新規 `ProcState`（`fork_ctx: Some(fork_id)`）として `self.active` に積み、現在のプロセスは `StepResult::ForkJoin(fork_id)` を返して `fork_waiters` に保存され停止。各分岐プロセスは通常のプロセスと同じスケジューラ（`#delay`/`@event` 含む）で独立に進行し、完了時 (`exec_proc` の `StepResult::Done` 処理) に `fork_ctx` を見て `fork_branch_done` を呼び、残り分岐数をデクリメント。0になったら `fork_waiters` から親を取り出し `active` に戻して join を完了させる。
  - 関数呼び出し本体（ゼロタイム実行の `exec_sync_stmt`）では `NamedBlock` はラベル無視で逐次実行、`Disable` は no-op、`Fork` は逐次実行にフォールバック（関数内でのdisable/forkは仕様上ほぼ使われないため、サブセットとして許容）。

### 制約

- `disable` は同一プロセス内の名前付きブロックのみ対象。他プロセスで実行中のタスク呼び出しやブロックを中断する一般形（`disable` の本来の主用途の一つ）は未対応。
- `disable fork;`（forkした分岐をまとめて中断）は未対応。
- `fork`/`join_any`/`join_none` の区別をしておらず、全て `join`（全分岐完了待ち）として扱う。
- 関数本体（ゼロタイム実行）内の `disable`/`fork` は意味的に簡略化（no-op/逐次実行）。

### Result

```
$ cargo test --workspace
test test_counter4 ... ok
test test_func_task ... ok
test test_gates ... ok
test test_generate ... ok
test test_disable_fork ... ok   (新規)
test test_fifo_sync ... ok
全テストパス ✅
```

手動確認 (`tests/integration/cases/disable_fork/dut.v`):
```
i=0
i=1
i=2
i=3
i=5
i=6
i=7
i=8
i=9
after loop i=10
branch b done at 2
branch a done at 5
joined at 5 a=1 b=2
$finish at time 6
```
`disable loop_body;` がfor文の1イテレーション（i=4）だけをスキップし（forループ自体は継続）、`fork...join` がdelay 2とdelay 5の2分岐を並行実行して両方完了するt=5までjoinが待つことを確認。

### Next

- iverilog との出力比較 CI 導入
- `$readmemh`/`$random` 対応
- 部分 X 伝搬の精度向上

---

## 2026-06-21

### Task

`$readmemh`/`$readmemb`/`$random` 対応の実装。

### 設計

- **HIR/MIR**: `SysTask` に `ReadMemH`/`ReadMemB` を追加。汎用の `Stmt::SysCall(SysTask, Vec<ExprId>)` だと第2引数の「対象メモリ」を `ExprId` として表現できない（メモリは式ではなく `MemId` で参照する必要がある）ため、MIRに専用バリアント `Stmt::ReadMem(SysTask, ExprId, MemId)` を新設した。`$random` は式コンテキストのシステム関数のため、既存の `SysFuncKind`（従来は `Clog2` のみ・const式専用）に `Random` を追加し、実行時評価される `Expr::Random(Option<ExprId>)` をMIRに新設した。
- **Frontend** (`crates/frontend/src/lower.rs`): `lower_system_task` の名前抽出部を `system_tf_name` ヘルパーに切り出し、文コンテキスト（`lower_system_task`）と式コンテキスト（`lower_primary` の `FunctionSubroutineCall` 内、新規追加した `SystemTfCall` 分岐）の両方から共有。`$readmemh`/`$readmemb` は文として、`$random` は式としてパースする。対象メモリ識別子はビット選択なしの単純参照なので、既存の `lower_primary`（`Hierarchical` 分岐）がそのまま `Expr::Net(mem_name)` を生成する。
- **Elab** (`crates/elab/src/elaborate.rs`): `HirStmt::SysCall` の処理を `ReadMemH`/`ReadMemB` 専用分岐と既存の汎用分岐に分割。専用分岐では第2引数の `HirExpr::Net(name)` を `ctx.resolve_mem` でメモリ本体に解決し `Stmt::ReadMem` を生成（メモリ以外の式が渡された場合は `UnsupportedConstruct` エラー）。`lower_expr` の `HirExpr::SysFunc` 分岐を `Clog2`/`Random` で分け、`Random` はseed引数（あれば）を実行時式として lowering して `Expr::Random(Option<ExprId>)` を生成。
- **Sim** (`crates/sim/src/interp.rs`):
  - `Stmt::ReadMem` 実行時に対象ファイルを読み込み、`strip_readmem_comments`（`//`/`/* */` 除去）→ 空白区切りトークン化 → `@<hexaddr>` でアドレスジャンプ → 各値トークンを `parse_readmem_token`（16進/2進、`x`/`X`/`z`/`Z`/`_` 対応）でパースし `mem_values` に書き込む。`x`/`z` は桁（nibble/bit）全体をX/Zにする必要があり、最初の実装では1ビットだけX/Zにする bug があったため、digit→bit展開を「数値桁はビット分解、x/z桁は全ビット同値」に修正。
  - `Expr::Random(seed)`: `Interpreter` に固定シード（`0x2545F4914F6CDD1D`）の `rng_state: u64` を追加し、xorshift64* で32bit値を生成。seed引数があれば一度だけ `rng_state` を上書き（IEEE仕様の「seedを参照で更新する」動作は実装せず、読み取り専用として扱う）。

### 制約

- `$readmemh`/`$readmemb` のファイルパスは文字列リテラルのみ対応（変数・式は不可）。対象メモリ引数はビット選択なしの単純識別子のみ。
- `$random(seed)` の seed は読み取り専用（IEEE仕様の参照更新は未実装）。
- PRNGは固定シード・簡易アルゴリズム（xorshift64*）であり、iverilog等の `$random` 系列とは数値が一致しない。決定的なため統合テストでの再現性は保たれる。
- 既存の `$display`/`$write`/`$monitor` の `%h`/`%b`/`%o`/`%d` フォーマッタは X/Z ビット（bval）を見ずに aval をそのまま出力する既存の制約があり、`$readmemh` で読み込んだ X/Z 値は `%h` 等で正しく "x" と表示されない（"部分X伝搬の精度向上" は別タスクの範囲）。この制約に当たるため、統合テストでは X/Z パース自体の正しさを `crates/sim/src/interp.rs` 内の単体テスト（`readmem_tests` モジュール）で直接 `LogicVal` の bit 単位検証により確認した。

### Result

```
$ cargo test --workspace
test test_counter4 ... ok
test test_func_task ... ok
test test_gates ... ok
test test_generate ... ok
test test_disable_fork ... ok
test test_readmem_random ... ok   (新規)
test test_fifo_sync ... ok
全テストパス ✅
```

新規追加: `crates/sim/src/interp.rs` の `readmem_tests` モジュール（5テスト、hex/bin/x/z digit decomposition とコメント除去を検証）。

統合テスト (`tests/integration/cases/readmem_random/`):
```
mem[0]=0
mem[1]=11
mem[2]=22
mem[3]=0
mem[4]=0
mem[5]=aa
rand0=325595736
rand1=258385808
rand_seeded=144411322
$finish at time 0
```
`$readmemh` がコメント（`//`/`/* */`）と `@addr` アドレスジャンプを正しく処理してメモリを初期化し、`$random`/`$random(seed)` が決定的なPRNG値を返すことを確認。

### Next

- iverilog との出力比較 CI 導入
- 部分 X 伝搬の精度向上（`%h`/`%b`/`%o`/`%d` フォーマッタのX/Z対応含む）

---

## 2026-06-21 (2)

### Task

「iverilog との出力比較 CI 導入」。

### 経緯・発見した既存バグ

CI導入の前段として、既存の統合テストケース（`disable_fork`/`func_task`/`generate`/
`fifo_sync` 等）を実際に iverilog (v13.0) で実行して比較したところ、
`crates/sim/src/interp.rs` の `format_string`（`$display`/`$write`/`$monitor` の
書式処理）が `%d`/`%h`/`%o`/`%b` の**幅修飾子なし**の既定フィールド幅パディングを
一切実装していないバグが判明した。IEEE 1364では幅修飾子なしの場合、フィールド幅は
オペランドのビット幅から導出される（`%d`なら最大値の10進桁数で空白パディング、
`%h`/`%o`/`%b`ならビット幅から導出される桁数でゼロパディング）。例:
`generate` テストで4bit値 `0110` を `%b` 出力すると iverilog は `0110` だが
rverilog（修正前）は先頭ゼロが落ちて `110` になっていた。

これはCI比較を導入する上で既存テストが即座に不一致になる実物の正確性バグであり、
CI導入の前提として今回修正した（部分X伝搬/bval表示の精度問題とは独立した、
known値の桁数パディング欠落というだけの問題）。

### 修正

- `format_string` に「幅修飾子があるかどうか」のフラグを追加し、幅修飾子が
  **ない**場合のみビット幅由来の既定フィールド幅でパディングする処理を追加：
  `%d`→最大値の10進桁数で空白右詰め、`%h`→`ceil(width/4)`桁でゼロ左詰め、
  `%o`→`ceil(width/3)`桁でゼロ左詰め、`%b`→`width`桁でゼロ左詰め。
  幅修飾子がある場合（`%08d`等、既存テストでは未使用）は従来どおりパディングなし。
  64bit超（Large値）とX/Z表示は対象外（既存の制約のまま、別タスク）。
- 修正の影響で `func_task`/`generate`/`readmem_random` の `expected.stdout` を
  実際の（iverilog互換の）出力に更新した。

### CI構成

- `crates/cli/tests/common/mod.rs` に `workspace_root`/`run_sim`/`expected_stdout`
  を切り出し、既存の `integration.rs` と新規の `iverilog_compare.rs` から共有。
- `crates/cli/tests/iverilog_compare.rs` を新規追加。`iverilog`コマンドが
  環境に無い場合は `eprintln!` してスキップ（ローカル開発者の `cargo test` を
  壊さない方針）。あれば `iverilog -g2001` でコンパイル→`vvp`実行し、rverilogの
  出力と比較する。`$finish`の通知メッセージはrverilog独自文言
  （`$finish at time N`）とiverilog独自文言（`... $finish called at N (1s)`）で
  意図的に異なるため、両者とも末尾の`$finish`行を1行除去してから比較する。
  比較対象: `counter4`/`func_task`/`gates`/`generate`/`disable_fork`/`fifo_sync`。
  `readmem_random` は `$random` のPRNGアルゴリズムが異なり数値が一致しないため除外。
- `.github/workflows/ci.yml` を新規追加。`apt-get install iverilog` の後に
  `cargo build --workspace`→`cargo test --workspace` を実行する単純な構成。
  これによりCI環境では `iverilog_compare` テストが必ず実行される。

### Result

```
$ cargo test --workspace
running 7 tests (integration.rs)  -- 全件pass（func_task/generate/readmem_random は
                                       新フォーマッタ出力に合わせ expected.stdout 更新済み）
running 6 tests (iverilog_compare.rs)  -- 全件pass
running 5 tests (readmem_tests, interp.rs内)  -- 全件pass
全テストパス ✅
```

### Next

- 部分 X 伝搬の精度向上（`%h`/`%b`/`%o`/`%d` フォーマッタのX/Z対応、幅修飾子付き指定子の完全実装含む）

---

## 2026-06-21 (3)

### Task

「部分X伝搬の精度向上」。`$display`系フォーマッタ（`%d`/`%h`/`%o`/`%b`）のX/Z表示と、
幅修飾子付き指定子（`%5d`/`%08h`等）の完全実装。

### 発見した追加バグ（実装中）

検証用に `8'bxxxx_xxxx`/`8'bzzzz_zzzz`/`8'b1010_xxxx` 等のリテラルで動作確認したところ、
フォーマッタとは別の、より根本的なバグが判明した：`crates/frontend/src/lower.rs` の
`parse_number_text`（基数付きリテラルのパース）が、digit部分に`x`/`X`/`z`/`Z`が
**1文字でも**含まれていると、桁ごとの情報を一切見ずに**全bit X**（`lv_x`）に
丸めていた。つまり `8'bzzzz_zzzz` や `8'b1010_xxxx` のようなリテラルは、ソースコード
経由ではこれまで全く正しく表現できていなかった（`$readmemh`はファイル読み込み時に
別の専用パーサ`parse_readmem_token`を使っていたため、こちらは元々正しくX/Zを保持できていた）。
今回のタスクの本質（X/Z表示精度の検証）を進める前提として、このリテラルパースも修正した。

### 修正

1. **リテラルのbit単位X/Z保持** (`crates/frontend/src/lower.rs`):
   `parse_number_text`に`parse_based_digits`関数を新設（`$readmemh`の
   `parse_readmem_token`と同様、digitごとにX/Zをbit展開するclosureパターンを採用）。
   2進/8進/16進は桁ごとに`x`/`z`を正しく展開し、10進は値全体が`x`/`z`の場合のみ対応
   （IEEE仕様上、10進は桁単位のX/Z混在が存在しないため）。
2. **フォーマッタのX/Z対応＋幅修飾子完全実装** (`crates/sim/src/interp.rs`の`format_string`):
   iverilog (v13.0) で`8bit`値の既知/全X/全Z/部分X/部分Z/明示幅修飾子の組み合わせを
   実機検証し、以下の規則を確認・実装した:
   - 各「桁」（`%b`=1bit、`%h`=4bit、`%o`=3bit、`%d`=値全体を1グループ）について、
     グループ内が全known→数字、全unknownかつ単一種別（全X or 全Z）→小文字`x`/`z`、
     部分known+unknownまたは種別混在→大文字`X`/`Z`（iverilog実測と一致）。
   - 幅修飾子なし: ビット幅由来の桁数（`%d`は最大値の10進桁数、`%h`/`%o`/`%b`は
     ビット幅から導出）で算出した「自然表示」をそのまま使う。
   - 幅修飾子`0`のみ（`%0d`等）: 自然表示の先頭`'0'`（および`%d`の場合は先頭空白）を
     取り除いた最小桁数表示。
   - 幅修飾子が数値N（`%5d`/`%08h`等）: 自然表示をN文字に達するまで、先頭が`0`なら
     ゼロ詰め、そうでなければ空白詰め。
   - 新設のヘルパー`group_char`/`natural_repr`/`apply_width_modifier`で実装。
     `%h`/`%o`の境界（8bit幅で`%o`の最上位桁が2bitしかない等）はグループごとに
     実際に残っているbit数でマスクを動的に決定し、誤って既知0bitを混入させない
     ようにした（このマスク計算を誤ると、本来"全unknown→小文字"になるべき桁が
     "部分known→大文字"に誤判定される）。

### 制約

- 64bit超（Large値）は既存通り対象外（生のaval表示のまま）。
- グループ内でX型とZ型のunknownビットが混在する稀なケースは大文字`X`にフォールバック
  （iverilogでの実機確認なし、保守的な仕様判断）。

### Result

```
$ cargo test --workspace
8 tests (integration.rs)        全件pass（新規 test_format_xz 追加）
7 tests (iverilog_compare.rs)   全件pass（新規 compare_format_xz 追加。
                                 $readmemhでX/Z値を読み込んだケースもiverilogと完全一致）
11 tests (interp.rs内ユニットテスト) 全件pass（新規 format_tests モジュール6件、
                                 iverilog実測値をそのまま期待値として使用）
全テストパス ✅
```

`$readmemh`で`xx`/`zz`/`1z`等を読み込んだメモリを`%h`/`%b`/`%d`で表示するテスト
（`tests/integration/cases/format_xz/`）を追加し、iverilogとの出力比較でも一致を確認。
これにより以前明記していた「`$readmemh`で読み込んだX/Z値が`%h`等で正しく表示されない」
という制約は解消された。

### Next

- 64bit超（Large値）のフォーマッタ・リテラルパースのX/Z対応
- `fork`/`join_any`/`join_none`の区別
- `$random(seed)`のseed参照更新（IEEE仕様準拠）

## 2026-07-02

### 実装方針レビュー（コード変更なし）

PLAN.md の設計方針と現行実装の乖離・IEEE 1364 セマンティクス上の問題を調査。
主な発見:

1. scheduler.rs は死んだコード（run_step が TODO スタブ、実ループは interp.rs に別実装）
2. $monitor が $display と同一動作（monitor リージョン未実装、値変化時の再表示なし）
3. エッジ検出が IEEE 非準拠: X→1 の posedge / 1→X の negedge を検出できない
   （trigger_sensitivity が aval のみで判定）
4. #0 遅延が future ヒープ経由のため NBA 適用の「後」に再開される
   （IEEE の inactive→NBA 順序と逆）
5. 連続代入が毎 δ サイクル全件再評価の固定点ループ（200 回打ち切り）。
   sensitivity 逆引きテーブル方式（PLAN 記載）と乖離、規模で性能劣化
6. write_lvalue のビット/部分選択パスが u64 演算のみで 64bit 超ネットに未対応
7. signed 演算が全面未実装（NetInfo に is_signed なし、>>>・signed 比較・%d が unsigned 扱い）
8. width.rs は 14 行の実質スタブ（context-determined width 未実装）
9. $dumpvars の深さ・スコープ引数未対応

課題リストは会話ログ参照。M2 の優先順位候補: signed 対応 → monitor リージョン →
エッジ検出修正 → 連続代入の sensitivity 駆動化。

### PLAN.md へ反映

- 「M2 以降」を更新: 完了済み項目（function/task, generate, gate primitive,
  disable/fork, $readmem/$random, iverilog 比較 CI）と残タスク優先順を明記
- 新セクション「実装レビューと課題（2026-07-02）」を追加:
  重大 5 件 / 乖離 3 件 / 軽微 5 件の課題表と対応方針（テスト先行で修正）

## 2026-07-09

### Task

master ベースで実装課題の棚卸しを行い、PLAN.md に「実装課題」セクションとして追記。

### What was done

- コードベース全体（frontend/elab/mir/sim/vcd_out/cli）を調査し、未対応構文・
  コードレベルの負債（TODO/スタブ）・既知の近似・システムタスク不足・幅/符号推論の
  ギャップを洗い出した
- 未マージブランチ（`feat/func-task-gates`、`feat/generate-disable-fork`、
  `docs/implementation-review`）と突合し、ブランチ側で解決済みの項目
  （function/task、generate/genvar、disable/fork-join、gate primitive、
  $readmemh/$readmemb/$random、iverilog 比較 CI）には「※未マージ」と注記
- PLAN.md 末尾に「実装課題（2026-07-09 時点、master ベース）」を追記。
  正確性 9 件（A1-A9）・未対応言語機能・システムタスク不足・設計乖離/死コード 4 件・
  テスト/CI・軽微 5 件の 6 分類と推奨着手順を記録
- `docs/implementation-review` ブランチの PLAN.md に既存の
  「実装レビューと課題（2026-07-02）」があるため、マージ時に統合する旨を明記

### Notes

- 最優先は signed 対応（A1、width.rs 再設計とセット）。データ構造に触るため
  後回しにするほど手戻りが大きい
- 修正前に iverilog 比較 CI へテストケースを追加するテスト先行方針

---

## 2026-07-09 (続き)

### Task

未マージ 3 ブランチ（`feat/func-task-gates`、`feat/generate-disable-fork`、
`docs/implementation-review`）を master に統合。実装課題の推奨着手順 0 番。

### What was done

- ブランチ間の祖先関係を確認: `docs/implementation-review` が他 2 ブランチを
  完全に含む単一の直列統合ブランチであることが判明（マージは 1 回で済んだ）
- `git merge --no-ff docs/implementation-review` を実行。PLAN.md/DIARY.md が
  コンフリクト（両ブランチが独立に追記していたため）
  - DIARY.md: ブランチ側のエントリ（2026-06-18〜07-02）を自分の 07-09 エントリの
    前に時系列で並べ替えて統合
  - PLAN.md: 「実装課題（2026-07-09）」と「実装レビューと課題（2026-07-02）」の
    重複 2 セクションを 1 つに統合。「※未マージ」注記を解消し、branch 側のみに
    あった軽微課題（disable の同一プロセス制限、$random の iverilog 非互換）を
    軽微セクションへ統合。テスト/CI セクションを現状（iverilog 比較 CI 導入済み、
    統合テスト 8 ケース）に更新
- `cargo build --workspace` / `cargo test --workspace` で検証

### Result

✅ マージ成功、コンフリクト解消
✅ `cargo build --workspace` 成功（dead-code warning 3 件のみ）
✅ `cargo test --workspace` 全 39 テスト通過（unit 22 + integration 8 + iverilog_compare 7 +
   frontend 1 + doc-tests 0、iverilog 実行環境あり）

### Next

- 実装課題 A1: signed 演算対応（`elab/src/width.rs` の幅・符号推論再設計とセット）

---

## 2026-07-09 (続き2)

### Task

実装課題リストの推奨着手順1番、signed 演算対応（A1）を実装。

### What was done

#### データ構造の拡張
- `hir::design.rs`: `NetDecl`/`RegDecl`/`PortDecl`/`TfArg`/`FunctionDecl` に `signed: bool` を追加
- `mir::ir.rs`: `NetInfo` に `is_signed: bool`、`ElaboratedDesign` に `expr_signed: Vec<bool>`
  （ExprId ごとの signed 文脈フラグ）を追加

#### frontend: signed キーワードのパース
- `sv_parser::Signing::Signed` ノードを検出する `has_signed()` ヘルパを追加し、
  net/reg/port/function戻り値/tf引数の各宣言箇所に適用
- `integer` 宣言は IEEE 1364 通り常に signed=true
- **重要な追加修正**: 符号無し10進即値（`-7`, `2` 等、基数指定なし）と `'s` 基数指定
  （`4'sd5` 等）が IEEE 4.8 上 signed 文脈になることが未実装だったため、
  `HirExpr::SignedConst` バリアントを新設して対応。また `'s` マーカーの
  パースが未実装で `4'sd5` が常に0になっていたバグも修正

#### elaboration: signedness 伝搬
- `ElabCtx.expr_signed: Vec<bool>` を追加し、`alloc_expr()` が子 ExprId の
  signedness から自動計算（IEEE 1364-2001 4.5.1 簡易版: 算術/ビット演算は両辺
  signed のとき signed、比較/論理演算は常に1bit unsigned、シフトは左辺の
  signedness を伝播）
- `SignedConst` は自動計算をバイパスする `alloc_expr_signed()` で明示登録

#### mir::logicval.rs: signed 演算プリミティブ
- `extend_sign()`（符号拡張）、`sign_bit()`、`as_i64()`（64bit以内のsigned変換）
- `lt_signed`/`gt_signed`/`le_signed`/`ge_signed`（符号拡張後の二の補数比較）
- `div_signed`/`mod_signed`（0への切り捨て、Rustの`%`と同じ符号規則）
- 単体テスト6件追加（符号拡張・signed比較・signed除算/剰余・ashr vs shr）

#### sim::interp.rs: 演算子選択と%dフォーマット
- `apply_binop` が `l_signed`/`r_signed` を受け取り、Div/Mod/Lt/Gt/Le/Ge は
  両辺signedのときのみsigned版を、`>>>`は左辺のsignednessのみを見て
  `ashr`/`shr`を選択（IEEE: シフト量の符号は結果に影響しない）
- `%d`表示: `natural_repr`が`signed`引数を受け取りMSB=1なら二の補数を負の
  10進数として表示。`apply_width_modifier`のデフォルト幅計算もsigned時は
  「最大正値の桁数+1（符号分）」に修正（iverilog実測: 32bit signedで11桁、
  以前のunsigned式(10桁)から+1個の差分をiverilog比較テストで検出して修正）

#### テスト
- `tests/integration/cases/signed/`を新設。signed比較・signed除算/剰余・
  `>>>`のsigned/unsigned切替・signed `%d`表示を検証する`dut.v`を作成し、
  iverilogの実行結果をそのまま`expected.stdout`として採用
- `test_signed`（integration.rs）・`compare_signed`（iverilog_compare.rs）を追加

### Result

✅ `cargo build --workspace` 成功
✅ `cargo clippy --workspace --all-targets` エラーなし
✅ `cargo test --workspace` 全41テスト通過（新規signed単体テスト8件・
   integration 1件・iverilog比較1件を含む）
✅ `compare_signed`（iverilog実行環境との bit-exact 比較）通過

### Next

- 実装課題 A2: エッジ検出の IEEE 準拠化（X→1 posedge 検出）
- `elab/src/width.rs` の context-determined 幅推論再設計は今回見送り（別課題として残存）

## 2026-07-14

### Task

実装課題リストの推奨着手順2番、エッジ検出の IEEE 準拠化（A2）を実装。

### What was done

#### 原因調査
- `crates/sim/src/interp.rs` の `trigger_sensitivity`（719-756行）が
  posedge/negedge 判定に `LogicVal` の a-plane（aval）ビットのみを見ており、
  b-plane（bval）を無視していることを確認。X（a=1,b=1）は 1（a=1,b=0）と、
  Z（a=0,b=1）は 0（a=0,b=0）と区別できず、IEEE 1364 が定める
  `0→X`/`X→1`（posedge）や `1→X`/`X→0`（negedge）を検出できなかった。
- 4つの呼び出し元（interp.rs:142, 301, 599, 770）はいずれも完全な
  `LogicVal` old/new を渡しており、バグは `trigger_sensitivity` 内部の
  分類ロジックに限局していることを確認。

#### テスト先行
- `tests/integration/cases/edge_x/dut.v` を新設。`reg sig`（初期値X）を
  `0→X→1→X→0→1→0` と遷移させ、`always @(posedge sig)`/`always @(negedge sig)`
  でそれぞれカウンタをインクリメントし `$display` する自己完結モジュール。
- 実機 iverilog を実行し `pos_count=3 neg_count=4` を確認、これを
  `expected.stdout` に採用（手書きの期待値ではなく実機オラクル）
- 修正前の rverilog は `pos_count=2 neg_count=3`（バグを再現）だったことを
  `cargo test` で確認してから修正に着手
- `test_edge_x`（integration.rs）・`compare_edge_x`（iverilog_compare.rs）を追加

#### 修正
- `trigger_sensitivity` を、old/new それぞれの a/b ビットから `{0,1,X,Z}` を
  分類し、IEEE 表（posedge: `0→1`/`0→X`/`X→1`、negedge: `1→0`/`1→X`/`X→0`）で
  判定するよう書き換え。Z は edge 判定上 X 相当として扱う（iverilog実機の
  挙動と一致することをテストオラクルで確認）
- `Sensitivity::All`（`@*`）が使う `any_change`（LogicVal全体の等価比較）は
  元々正しいため変更なし

### Result

✅ `cargo build --workspace` 成功
✅ `cargo clippy --workspace --all-targets` 新規警告なし（既存の
   frontend/elab警告は本修正と無関係で対象外）
✅ `cargo test --workspace` 全50テスト通過（新規 `test_edge_x`・
   `compare_edge_x` を含む。`signed`/`fifo_sync`/`counter4` 等
   posedge依存の既存テストに回帰なし）

### Next

- 実装課題 A3・A4: `$monitor` の専用リージョン実装 + `#0`（inactive）
  順序修正（イベントループ再構成として一括対応、PLAN.md 対応方針の次項）

## 2026-07-10

### Task

`$dumpvars(0);` だけ呼び出して `$dumpfile(...)` を呼ばない testbench で
VCD 波形が全く出力されない不具合を調査・修正。

### What was done

#### 原因調査
- `samples/fifo_sync/tb/tb.v` の initial ブロック冒頭に `$dumpvars(0);` を
  追加して再現。VCD ファイルが一切生成されないことを確認。
- `crates/sim/src/interp.rs` の `SysTask::DumpVars` ハンドラを確認したところ、
  `self.vcd_path`（`$dumpfile` 呼び出しでのみ設定される）が `None` の場合は
  何もせず無言でスキップする実装になっていた。
- IEEE 1364-2005 §17.2 では `$dumpfile` 未呼び出し時、既定で
  カレントディレクトリの `dump.vcd` に出力する仕様。実機 iverilog で
  `$dumpfile` なしの `$dumpvars` を実行し、`dump.vcd` が生成され
  かつ起動時に `VCD info: dumpfile dump.vcd opened for output.` が
  stdout に出力されることを確認、既定動作を裏付けた。

#### 修正
- `SysTask::DumpVars`: `vcd_path` が未設定の場合 `PathBuf::from("dump.vcd")`
  にフォールバックするよう修正。二重初期化防止のため `self.vcd.is_none()`
  ガードも追加。
- `init_vcd_from_path` に、iverilog と同じ `VCD info: dumpfile <path>
  opened for output.` を stdout / `output_buf` へ出力する処理を追加
  （既存の iverilog 差分比較テストとの bit-exact 一致を維持するため）。
- `tests/integration/cases/fifo_sync/expected.stdout` に上記メッセージ行を
  追加し、`samples/fifo_sync/tb/tb.v` に追加された `$dumpvars(0);`
  （`$dumpfile` なし）に対応させた。

### Result

✅ `cargo build --workspace` 成功
✅ `cargo clippy --workspace --all-targets` エラーなし
✅ `cargo test --workspace` 全テスト通過（`compare_fifo_sync` の
   iverilog差分比較含む）
✅ 実際に `dump.vcd` が生成され、`$dumpvars` 以降の信号変化が
   記録されていることを目視確認

### Next

- 実装課題 A2: エッジ検出の IEEE 準拠化（X→1 posedge 検出）
- `elab/src/width.rs` の context-determined 幅推論再設計は今回見送り（別課題として残存）

## 2026-07-10 (2)

### Task

「このシミュレータの SystemVerilog 仕様の準拠程度」について調査依頼を受け、
コードベース実地調査を実施。結果を PLAN.md に反映。

### What was done

#### 調査
- `crates/frontend/src/lower.rs` の SV 構文分岐（`always_ff` 等の未対応確認）、
  `crates/mir/src/ir.rs` の `SysTask`/`BinOp`/`Stmt` 列挙、
  `crates/sim/src/interp.rs::exec_syscall` の実装済みシステムタスクを直接調査
- `lower_loop_stmt` が `LS::For` のみ対応し、`while`/`repeat`/`forever` は
  明示エラーになることをコードで確認
- `lower.rs` 内の複数の `_ => {}` catch-all（`defparam`/`specify`/UDP 等が
  無言スキップされる箇所）を再確認。既存の実装課題 B 節と同一問題であることを確認
- PLAN.md の Context 節に「Verilog-2001 サブセット」が当初からの目標として
  明記されていることを再確認し、SV 準拠評価はスコープ外である旨を結論に含めた

#### 反映
- PLAN.md に「SystemVerilog 準拠度調査（2026-07-10）」節を追加。
  SV 固有機能の対応状況表、Verilog-2001 サブセットとしての評価、
  既存の実装課題節（特に B: 無言スキップ診断化）との関連付けを記載

### Result

✅ 実コード（frontend/mir/sim の3クレート）を根拠に調査完了、PLAN.md 反映済み
✅ 既存の実装課題節との重複を避け、関連箇所への参照リンクとして記述

### Next

- 実装課題節の推奨着手順（A2 → A3/A4 → D → B）は変更なし。優先度は据え置き

## 2026-07-12

### Task

`$signed`/`$unsigned` システム関数対応（picorv32.v シミュレーションが目標）。
設計書 `docs/superpowers/specs/2026-07-12-signed-support-design.md` を実装。
作業ブランチ: `feat/signed-impl`

### What was done

#### $signed/$unsigned のパイプライン貫通（Task 2）

- `SysFuncKind` に `Signed`/`Unsigned` を追加（`hir/src/design.rs`）
- frontend の `SystemTfCall` に `$signed`/`$unsigned` の腕を追加。引数1個を検証、
  不正なら `FrontendError::ParseError`（`frontend/src/lower.rs`）
- elab は inner のトップ `Expr` を `alloc_expr_signed` で複製登録する
  「アプローチB」（MIR に cast ノードを追加しない）。定数式パスにも素通し腕を追加
  （`elab/src/elaborate.rs`）

#### 作業中に発見した既存バグ2件を修正（Task 2a/2b）

いずれも `$signed` と独立で、**エラーにならず黙って誤値になる**タイプ:

1. 連結 `{imm[11], x}` の lowering が subtree 全走査
   （`c.as_ref()` の全 `RefNode::Expression`）だったため、ビット選択の
   インデックス式 `11` などが parts に混入 → `Brace<List>` の直接の子のみ走査に修正
2. 式コンテキストの部分選択 `imm[10:5]` が **全ビット値** に化けていた
   （`lower_primary` の `P::Hierarchical` がビット選択しか処理せず、
   part-select は無言で `Net` にフォールバック）→ `PartSelectRange::ConstantRange`
   を `HirExpr::PartSel` へ lowering。`+:`/`-:`（IndexedRange）は明示エラーに。
   elab/sim 側は元々 `PartSel` 対応済みで frontend のみの修正

#### テスト（Task 1、TDD）

- `tests/integration/cases/signed_cast/` を追加（picorv32 型の NBA/継続代入/
  比較/除算/`$unsigned`/幅違い signed 加算を網羅）。期待値は iverilog 実出力から作成
- `integration.rs` に `test_signed_cast`、`iverilog_compare.rs` に
  `compare_signed_cast` を追加

### Result

✅ 既存テスト全パス（iverilog 比較 8/8）、リグレッションなし
✅ 連結・部分選択の値が正しくなった（`{imm[11], imm[10:5]}` = `1000000` 等）
⏳ `signed_cast` テストは意図的に FAIL 中（TDD アンカー）。値は全行正しく、
   残るは符号拡張のみ（`cat=00000040` ← 期待 `ffffffc0`、`add=17` ← 期待 `1`）

### Remaining

- Task 2c: LHS 部分選択 `q[31:20] <= x`（現状**全ビット代入に化ける**バグ。
  frontend `lower_var_lvalue` のみの修正で済む見込み）
- Task 2d: LHS 連結 `{a,b} <= x`（現状先頭要素のみ。HIR/MIR に `LValue::Concat`
  追加と sim の分割書き込みが必要）
- Task 3: 代入時の符号拡張（`write_lvalue` に `rhs_signed`、NBA キューにフラグ）
- Task 4: 演算オペランドの符号拡張（`apply_binop` で max 幅へ `extend_sign`）
- Task 5: picorv32.v parse/elab スモークチェック

詳細は PLAN.md「$signed/$unsigned 対応と部分選択/連結バグ修正（2026-07-12、進行中）」
節および `docs/superpowers/plans/2026-07-12-signed-support.md` を参照。

## 2026-07-14 (2)

### Task

実装課題リストの推奨着手順3番、`$monitor` 専用リージョン実装 + `#0`
（inactive）順序修正（A3・A4）をイベントループ再構成として一括実装。

### What was done

#### 原因調査
- `crates/sim/src/interp.rs::run()`（87-173行）が active drain と NBA 適用を
  1つのループで交互に行い、両方が空になって初めて `future` ヒープを見に行く
  構造であることを確認。`#0` は他の `#N` と同じ `future` に
  `wake = now + 0` として積まれるが、このループ構造だとNBA適用より後にしか
  拾われないため、IEEEの `active → inactive → NBA` 順が逆転していた（A4）。
- `exec_syscall` の `SysTask::Monitor`（406-411行）が `$display` と
  バイト単位で同一実装で、印字時に値変化の判定を一切行っていないことを確認（A3）。

#### テスト先行
- `tests/integration/cases/monitor/dut.v`: `$monitor("t=%0t cnt=%d", $time, cnt)`
  を一度登録し `cnt` を複数回変化（一部は無変化）させるケースを作成。
  iverilog実測値（`t=0 cnt=0`, `t=1 cnt=1`, `t=3 cnt=2` の3回のみ印字、
  無変化のt=2はスキップ）を `expected.stdout` に採用。
- `tests/integration/cases/delay0/dut.v`: `initial a=0;` / `initial a<=1;`
  （NBA登録）/ `initial begin #0; $display(...) end`（inactiveで再開）を
  並べたケースを作成。iverilog実測値（`#0`再開時点でNBA適用前の
  `a=0`が見える）を `expected.stdout` に採用。
- 修正前のrverilogでは monitor が最初の1回（`t=0`）しか印字せず、
  delay0 では `#0` 再開時に既にNBA適用後の `a=1` が見える（バグを再現）
  ことを `cargo test` で確認してから修正に着手。
- `test_monitor`/`test_delay0`（integration.rs）・`compare_monitor`/
  `compare_delay0`（iverilog_compare.rs）を追加。

#### 修正
- `Interpreter` に `monitor_args: Option<Vec<ExprId>>`・
  `monitor_last: Option<Vec<LogicVal>>` を追加。`$monitor` はIEEE 1364通り
  シミュレーション全体で1つだけアクティブ（新規呼び出しが前の登録を置き換え、
  `monitor_last` もリセットして次回flush時に必ず再印字させる）。
- `SysTask::Monitor` ハンドラは登録のみ行うよう変更（即印字しない）。
- 新設 `flush_monitor()`: 登録済み引数（`args[1..]`、`args[0]`はフォーマット
  文字列）を評価した `Vec<LogicVal>` を前回スナップショットと比較し、
  異なる場合（初回含む）のみ `format_args` で整形して印字。`%t`/`%T` は
  `self.now` を直接参照し `ExprId` を消費しないため、このスナップショットは
  時刻ノイズを含まない。
- `run()` のメインループを再構成: 従来の「active drain + NBA適用」の
  内側ループから、active region（`eval_conts`含む完全収束）→
  inactiveリージョン（同時刻`#0`待ちプロセスをNBA適用前に`active`へ復帰、
  復帰があれば`continue`でactive regionへ戻る）→ NBAリージョン（適用後
  `continue`）→ monitorリージョン（`flush_monitor`、3リージョンとも
  完全収束した時のみ到達）→ 次時刻への前進、の順に分離。

### Result

✅ `cargo build --workspace` 成功
✅ `cargo clippy --workspace --all-targets` 新規警告なし（既存の
   frontend/elab/sim内の他行の警告は本修正と無関係で対象外）
✅ `cargo test --workspace` 全通過（新規 `test_monitor`・`test_delay0`・
   `compare_monitor`・`compare_delay0` を含む。`fifo_sync`/`counter4`/
   `signed` 等既存テストに回帰なし）
✅ `compare_monitor`・`compare_delay0`（iverilogとのbit-exact比較）通過

### Next

- 実装課題 D: 連続代入の sensitivity 駆動化（`eval_conts` の総当たり
  固定点ループを `NetId→ContId` 逆引きテーブル方式に）。
  scheduler.rs/systask.rs の死コード整理はどのタイミングでも安価
- 実装課題 B: サイレントスキップ（`defparam`/`specify`/UDP等）の診断化
- 実装課題 E: fmt/clippy ジョブの CI 追加、負パステストの拡充

## 2026-07-15

### Task

A2（エッジ検出のIEEE準拠化）を実装するよう依頼を受けたが、作業中に
「同じ課題に対して複数のPRが並行して存在し、マージ時にコンフリクトが
起きる」という状態が発覚。まず自前でA2を実装したところ、実は
**別セッションが2026-07-13〜14に同じA2、および後続のA3・A4を
先に実装しPR化していた**（PR #13 `fix/edge-detection-ieee`、
PR #14 `fix/monitor-region-inactive-order`）ことが判明。両PRは
signed対応（PR #7〜#12）がマージされる前の古い `feat/m1-milestone`
から分岐しており、ベースが進んだことでマージ時にコンフリクトする
状態になっていた。

### What was done

#### 状況把握
- 自分で実装したA2修正をPR #15として一旦作成した後、`gh pr list`で
  同一課題を扱う未マージPRが他に2件（#13, #14）存在することを発見
- 各PRの `mergeable`/`mergeStateStatus` を確認し、#13・#14が
  `CONFLICTING`（ベースの `feat/m1-milestone` が signed 対応の
  マージ（#7〜#12）で先に進んでいたため）である一方、自分の#15は
  最新ベースから作成したため単体では `MERGEABLE` だが内容が
  #13と重複している状態であることを確認
- ユーザーに「#13・#14を活かす／#15を活かす／中身を精査してから判断」の
  3択で方針を確認 → 「#13・#14を活かす」を選択

#### PR #13・#14 のリベースによるコンフリクト解消
- `fix/edge-detection-ieee`（PR #13、A2）を最新の `origin/feat/m1-milestone`
  （commit `0ea32bf`、signed対応マージ済み）に `git rebase` し、
  DIARY.md/PLAN.md/`integration.rs`/`iverilog_compare.rs` の
  コンフリクト（いずれも独立した追記同士がテキスト上隣接しただけで
  実質的な衝突ではない）を解消。`crates/sim/src/interp.rs` は
  自動マージで解決（A2の変更が `trigger_sensitivity` 内、A3・A4は
  別関数のため衝突なし）
- `fix/monitor-region-inactive-order`（PR #14、A3・A4）は、
  `git rebase --onto fix/edge-detection-ieee 7a9571c
  fix/monitor-region-inactive-order` で、リベース後の#13ブランチの
  上に積み直す形で適用（A3・A4はA2の上に構築される前提のため、
  スタックドPRとして扱った）。同様にドキュメント/テストファイルの
  コンフリクトを解消、`interp.rs` は自動マージ
- 各リベース後に `cargo build --workspace` / `cargo test --workspace`
  で全体を再検証。新規テスト（`test_edge_x`/`compare_edge_x`、
  `test_monitor`/`compare_monitor`/`test_delay0`/`compare_delay0`）
  含め通過を確認
- `test_signed_cast` のみ失敗するが、これはDIARY 2026-07-12節に
  記載の**既知の意図的なTDDアンカー失敗**（符号拡張未実装、Task 3/4）
  であり、`origin/feat/m1-milestone` 単体でも同一の失敗が再現する
  ことを確認済み。本セッションの変更とは無関係
- `git push --force-with-lease` で両PRのリモートブランチを更新。
  `gh pr view` で両方とも `mergeable: MERGEABLE` に変化したことを確認
  （CI `test` ジョブは上記の既知failureにより赤のままだが、
  マージ可能性とは別問題）

#### 重複PRの後始末
- 自分が作成したPR #15はPR #13と内容が重複するため、経緯を
  コメントに残した上で `gh pr close` でクローズ
- 重複していたローカル/リモートの `fix/edge-detection` ブランチを削除
- ローカルの `feat/m1-milestone` ブランチが実験中の余分なコミットで
  origin から乖離していたため、`git reset --hard origin/feat/m1-milestone`
  でクリーンな状態に戻した（余分だったコミットの内容は #13 に
  リベース済みで保全されているため損失なし）

### Result

✅ PR #13・#14 とも `mergeable: MERGEABLE` に変化（コンフリクト解消）
✅ 各リベース後に `cargo build --workspace`・`cargo test --workspace` で
   回帰なしを確認（`test_signed_cast` の既知failureを除く）
✅ PR #15 クローズ、重複ブランチ削除、ローカル `feat/m1-milestone` を
   origin と同期
⏳ CI `test` ジョブは `test_signed_cast`（既知のTDDアンカー、本セッション
   無関係）により両PRとも赤のまま。マージ自体は可能

### Next

- PR #13 → PR #14 の順でマージを推奨（#14が#13に依存するスタック構成のため）
- `test_signed_cast` の残タスク（Task 3: 代入時の符号拡張、Task 4: 演算
  オペランドの符号拡張）に着手すればCIも green になる見込み
- マージ後の実装課題節の残タスク（D: 連続代入のsensitivity駆動化、
  B: サイレントスキップの診断化、E: fmt/clippyジョブのCI追加）は
  従来通り
