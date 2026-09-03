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

## 2026-07-16

### Task

PR #13・#14 マージ後、PR #16（`docs/pr-conflict-resolution-2026-07-15`、
本DIARY記録PRそのもの）がベースの `feat/m1-milestone` に対して
コンフリクトしている、との報告を受け調査。

### What was done

- `gh pr view 16` で `mergeable: CONFLICTING` を確認。原因は
  DIARY.md末尾への追記が両側（PR #16のブランチと、#13・#14マージ後の
  `feat/m1-milestone`）で隣接していたための機械的な衝突で、実質的な
  内容衝突ではないことを diff で確認。
- `git rebase origin/feat/m1-milestone` を実行し、DIARY.md の
  コンフリクトを「2026-07-14 (2)」→「2026-07-15」の時系列順に
  並べる形で解消。他ファイルの変更なし（DIARY.mdへの82行追加のみ、
  という元PRの差分内容も rebase 後に一致することを確認）。
- `git push --force-with-lease` でPR #16のリモートブランチを更新。

### Result

✅ PR #16: `mergeable: CONFLICTING` → `MERGEABLE` に変化
⏳ `mergeStateStatus: UNSTABLE` はCI `test` ジョブが pending
   のためで、コンフリクトとは無関係

### Next

- PR #16のCI完了後、マージ可能

## 2026-07-16 (2)

### Task

PLAN.md「$signed/$unsigned 対応と部分選択/連結バグ修正（2026-07-12、進行中）」節の
残タスク（Task 2c・2d・3・4）を実装。`test_signed_cast` が意図的なTDDアンカーとして
FAIL状態のままだったのを解消するのが主目的。

### What was done

#### Task 3: 代入時の符号拡張
- `crates/sim/src/interp.rs` の `write_lvalue` に `signed: bool` 引数を追加。
  `LValue::Net` アームで signed なら `extend_sign`、そうでなければ従来通り `resize`
- 呼び出し元4箇所（`step()`のBlockingAssign/NbaAssign、`exec_sync_stmt()`の統合アーム、
  `eval_conts()`）で `ElaboratedDesign::expr_signed[expr_id]` から signed を取得して伝搬。
  `nba_queue: Vec<(LValue, LogicVal)>` は `Vec<(LValue, LogicVal, bool)>` に変更し、
  NBA適用（`run()`）まで signed を保持

#### Task 4: 演算オペランドの符号拡張
- `apply_binop` で `both_signed` かつ両辺の幅が異なる場合、演算前に共通の最大幅へ
  `extend_sign` で揃えるよう修正。シフト演算（`Shl`/`Shr`/`Ashl`/`Ashr`。右辺の
  シフト量は IEEE 上 self-determined unsigned のため対象外にする必要がある）と
  `===`/`!==`（暗黙のコンテキスト拡張をしないビット厳密比較）は除外
- `sa(4bit signed, -2) + sb(8bit signed, 3)` が zero-extend のまま加算されて
  `17` になっていたバグ（期待値 `1`）を修正

#### Task 2c: LHS部分選択（frontend）
- `crates/frontend/src/lower.rs` の `lower_var_lvalue` に、既存の式側
  `lower_primary`/`lower_part_select` と同型のパターンで part-select 分岐を追加
  （`lower_lvalue_part_select` 新設）。`elab::lower_lvalue` の
  `HirLValue::PartSelect` アームは元から実装済みだったため変更不要だった
  （frontend が生成していなかっただけ）

#### Task 2d: LHS連結
- `hir::LValue`/`mir::LValue` に `Concat(Vec<LValue>)` を追加
- `lower_var_lvalue` の `VL::Lvalue` アーム（`{a,b} <= x` で従来は先頭要素のみ返し
  残りを無言で捨てていた既知バグ、PLAN.md F節）を修正し `LValue::Concat` を返すよう変更
- `elab::lower_lvalue` に `HirLValue::Concat` → 再帰変換アームを追加
- sim側: 新設 `lvalue_width`（lvalueの合計ビット幅を再帰計算）を軸に、
  `write_lvalue`（合計幅へ resize/extend_sign → MSBから幅で切り出して各部分へ再帰書込）・
  `get_lval_val`（各部分の値をMSB順にconcat、欠損はX埋め）・`trigger_sensitivity`
  （合計幅でold/newを揃えてから各部分へ再帰分割）に `Concat` アームを追加

#### テスト
- 新規 `tests/integration/cases/lvalue_select/`（LHS部分選択・LHS連結を
  blocking/nonblocking 双方でカバー）を追加。期待値は iverilog 実出力から作成
- 既存 `test_signed_cast`/`compare_signed_cast`（Task 3・4 のTDDアンカー、
  2026-07-12 に用意済み）が新規追加なしで FAIL→PASS に変化することを確認

### Result

✅ `cargo build --workspace` 成功
✅ `cargo test --workspace` 全通過（新規 `test_lvalue_select`/`compare_lvalue_select` 含む、
   既存テストへの回帰なし。`test_signed_cast`/`compare_signed_cast` が FAIL→PASS）
✅ `samples/counter4`・`samples/fifo_sync`（M1受入れサンプル）を実行し、VCD生成・
   正常終了を確認（回帰なし）
⏳ Task 5（picorv32.v の parse/elab スモークチェック）は実ファイル未入手のため
   ユーザー判断でスキップ。PLAN.md に残タスクとして明記

### Next

- Task 5: picorv32.v を入手した場合にスモークチェックを実施
- 実装課題節の残タスク（D: 連続代入のsensitivity駆動化、B: サイレントスキップの診断化、
  E: fmt/clippyジョブのCI追加）は従来通り

## 2026-07-23

### Task

`feat/signed-lvalue-assign-ext` ブランチ（Task 2c/2d/3/4 実装コミット `5cdae32` 済み、
`feat/m1-milestone` からの差分はこの1コミットのみ）で、作業ディレクトリに `picorv32.v`
が入手できたため、PLAN.md に残タスクとして記録されていた Task 5
（picorv32.v の parse/elab スモークチェック）を実施。

### What was done

- `cargo build --workspace` / `cargo build -p rverilog-cli --release` が成功することを確認
- `rverilog --top picorv32 -o /tmp/picorv32_smoke.vcd --max-time 100 picorv32.v` を実行
- 結果: パースエラーで停止。`Unsupported construct: indexed part-select (+:/-:) in lvalue`
- 原因箇所を特定: `picorv32_pcpi_fast_mul` モジュール（2318行目〜、高速乗算器、
  `ENABLE_FAST_MUL` パラメータで選択的に使われる非デフォルト実装）内、2264〜2265行目
  ```verilog
  {next_rdt[j+CARRY_CHAIN-1], next_rd[j +: CARRY_CHAIN]} =
          next_rd[j +: CARRY_CHAIN] + next_rdx[j +: CARRY_CHAIN] + this_rs2[j +: CARRY_CHAIN];
  ```
  が LHS（連結内の part-select）・RHS 双方で indexed part-select (`+:`) を使用
- frontend の collection パスは**全モジュール定義を無条件に lowering する**設計のため、
  `picorv32` トップが `picorv32_pcpi_fast_mul` を実際にインスタンス化するか
  （`ENABLE_FAST_MUL` の既定値）とは無関係にこのエラーで停止する
- picorv32.v 全体を `while`/`repeat`/`forever`/`**`/`defparam`/`specify`/`real` について
  grep 走査した限り、他の明示的サブセット外構文は検出されず。indexed part-select が
  唯一のブロッカーと推定（ただしこの箇所を越えた先で新たな未対応構文に遭遇する可能性はある）

### Result

⏳ Task 5: picorv32.v は現状パース不可。ブロッカーは indexed part-select (`+:`/`-:`) 未対応
   （PLAN.md 実装課題 B 節「明示エラーになるもの」に記載済みの既知の未対応構文）
✅ 未対応構文の検出自体は「受理ホワイトリスト方式」の設計通り正しく機能している
   （サイレントスキップではなく明示エラー）

### Next

- indexed part-select (`+:`/`-:`) の実装要否をユーザーと相談。実装する場合は
  HIR/MIR に「動的base式＋定数width＋方向」を持つ新バリアントが必要になる見込みで、
  現行の `PartSel(NetId, Range)`（range が定数 msb/lsb 前提）とは別設計が要る
  （調査中）

## 2026-07-23 (2)

### Task

ユーザー承認により indexed part-select (`+:`/`-:`) 対応を実装（Planモードで設計後、
承認を得て着手）。picorv32.v スモークチェックのブロッカー解消が目的。

### What was done

- 既存アーキテクチャ調査（サブエージェント使用）: 定数レンジ part-select は
  frontend/HIR/MIR/elab/simの5層すべてで「コンパイル時定数(hi,lo)」前提。一方、動的な
  単一ビット選択は既に `Expr::BitSel`/`LValue::DynBitSelect` という並行パターンが
  存在（`crates/mir/src/ir.rs:129`）。indexed part-selectはこれを「1bit→定数幅Nビットの
  ウィンドウ」に拡張したものとして実装する方針を確定
- MIR (`crates/mir/src/ir.rs`): `Expr::DynPartSel(NetId, ExprId, u32, bool)`、
  `LValue::DynPartSelect(NetId, ExprId, u32, bool)` を追加（net, base式, 定数width, plus_dir）
- HIR (`crates/hir/src/design.rs`): `Expr::IndexedPartSel`/`LValue::IndexedPartSelect`
  を追加。widthは`Box<Expr>`のまま保持し、elabの`eval_const_hir`で確定させる二段階方式
  （既存の`Range.left/right`と同型）
- frontend (`crates/frontend/src/lower.rs`): `lower_part_select`/
  `lower_lvalue_part_select`の`IndexedRange`明示エラー分岐を実装に置換。
  `tree.get_str(&ir.nodes.1)`で`+:`/`-:`の方向を判定（既存の`lower_binary_op`等と同じ
  文字列比較パターン）
- elab (`crates/elab/src/elaborate.rs`): `lower_expr`/`lower_lvalue`に新アーム追加。
  baseは定数畳み込みせずExprIdとして伝搬、widthのみ`eval_const_hir`で確定。
  `compute_expr_signed`に`Expr::DynPartSel(..) => false`を追加（BitSel/PartSelと同じ
  self-determined unsigned）
- `LogicVal::x_of_width`ヘルパーを新設（`crates/mir/src/logicval.rs`、範囲外アクセス時に
  正しい幅の全X値を生成。既存のprivate `num_chunks`/`from_chunks`を再利用）
- sim (`crates/sim/src/interp.rs`): 共通ヘルパー`indexed_part_select_bounds`（base/width/
  方向から(hi,lo)を計算、不正範囲はNone）を新設し、`eval_expr`・`lvalue_width`・
  `get_lval_val`・`write_lvalue`・`trigger_sensitivity`の5箇所に`DynPartSel`/
  `DynPartSelect`アームを追加。範囲外アクセスは既存の`PartSelect`と同水準の簡略化
  （読み出しは全体X、書き込みは無視）
- 新規テスト`tests/integration/cases/indexed_part_select/`を追加（RHS `+:`/`-:`、LHS
  `+:`/`-:`のblocking/nonblocking、実行時for変数をbaseに使用）。iverilog実出力から
  期待値作成

#### 実装中に発見した重大な既存バグ（A10・A11、indexed part-selectとは独立）

テストケース作成中、`src[i*8+7 -: 8]`（乗算の後に加算）というよくあるパターンの結果が
iverilogと食い違うことに気づき調査した結果:

- **A10（重大）**: `crates/frontend/src/lower.rs`の`lower_expression`の`E::Binary`は
  `sv_parser`が返す生の`Expression::Binary`ノードをそのまま辿っているだけだが、
  その生ノード自体が演算子の優先順位・結合則を無視した木になっていることを
  `tree.get_str`で直接確認（`a-b+c`が`Binary(lhs="a", op="-", rhs="b+c")`という
  誤った木として返る）。常に右結合で評価されるため、`a*b+c`のような「高優先順位演算子の
  後に低優先順位演算子が続く」パターンで誤った結果になる（`7+i*8`のように逆順なら
  たまたま正しい木と一致するため見過ごされやすい）。picorv32.vのような複雑な算術式を
  含む実RTL全般に影響する可能性が高い、根本的な正しさの問題。PLAN.md 実装課題A節に
  A10として記録、次の最優先候補として明記
- **A11**: picorv32.vのモジュール中盤で宣言された`localparam`（状態機械の状態名等）が
  elabで名前解決できず`unresolved net/param`警告→`LogicVal::X`にフォールバックする
  ことを発見。case文の状態比較がすべてXになり、シミュレーションが`--max-time`指定でも
  停止せずハングすることを確認。原因箇所は未特定（未調査、A11として記録のみ）
- 両バグとも今回のスコープ外と判断し深追いせず、テスト自体は`7+i*8`のようにA10の
  影響を受けない順序で記述して実装を完了させた

### Result

✅ `cargo build --workspace`成功
✅ `cargo test --workspace`全通過（新規`test_indexed_part_select`/
   `compare_indexed_part_select`含む、既存テストへの回帰なし、計29テスト）
✅ `samples/counter4`・`samples/fifo_sync`（M1受入れサンプル）を実行しVCD生成・
   正常終了を確認（回帰なし）
✅ picorv32.vスモークチェック再実行: indexed part-selectのパースエラーは解消、
   8モジュール全てパース成功、elaborationも`picorv32`トップまで到達
⏳ picorv32.vのフルシミュレーションはA10・A11により未達成（シミュレーションが
   ハングして停止しない）
🔴 A10（二項演算子結合順序バグ）・A11（localparam名前解決）を新規発見、PLAN.mdに記録

### Next

- A10（二項演算子結合順序、重大）の修正をユーザーと相談。影響範囲が広いため優先度高いと
  判断
- A11（モジュール中盤のlocalparam名前解決）の原因調査・修正
- 上記2件の修正後、picorv32.vスモークチェックを再実行してフルシミュレーション到達を確認
- 従来からの残タスク（D: 連続代入のsensitivity駆動化、B: サイレントスキップの診断化、
  E: fmt/clippyジョブのCI追加）

## 2026-07-29 (2)

### Task

A11（モジュール中盤のlocalparam名前解決）の原因調査・修正。A10（別ブランチ
`fix/binop-precedence-a10`で対応済み、未マージ）とは独立のため、`feat/m1-milestone`
から新規ブランチ`fix/localparam-midmodule-a11`を切って着手。

### What was done

- 最小репro（`localparam cpu_state_trap = 8'b10000000;`等をalwaysブロックの後で宣言し、
  case文で比較するpicorv32.v同型パターン）を作成し、`unresolved net/param`警告を再現
- `eval_const_hir_with`に一時デバッグ出力を仕込んで原因を特定: 宣言位置は無関係で、
  実際の原因は2つの独立バグの複合だった
  1. **主因**: `lower_localparam`（`frontend/src/lower.rs`）が localparam の値を
     `tree.get_str`で取り出した生テキストを自前の簡易パーサ`parse_simple_const_expr`で
     解釈する設計になっており、このパーサは整数リテラル・`+`/`-`分割・`$clog2(...)`しか
     認識せず、`8'b01000000`のようなサイズ付き基数リテラルを識別子として`Expr::Net("8'b...")`
     に誤変換していた。elab側の`eval_const_hir_with`がこれを名前解決しようとして
     `UnresolvedName`を返し、`Err(_) => {}`で黙って登録スキップされていた
     （通常の10進値のlocalparamは`s.parse::<i64>()`で素通りするため気づかれなかった）
  2. **副次バグ**: 1を修正しても、`lower_expr`の`HirExpr::Net`アームがparam/localparam
     参照を`Expr::Const(LogicVal::new(32, val, 0))`で常に32bit固定生成していたため、
     8bit `state`レジスタとのcase比較（`LogicVal::case_eq`は幅不一致だと無条件でZERO）が
     常に不一致となり、default分岐に落ちていた（`unresolved`警告は消えたが値が一致しない
     という新しい症状として顕在化）
- 修正1: `parse_simple_const_expr`の末尾フォールバック（識別子扱い）の直前に、`'`を含む
  文字列は既存の`parse_number_text`（`lower_number`等で使われている正式なサイズ付き
  基数リテラルパーサ）に委譲する分岐を追加。`+`/`-`分割は既存のまま温存し、分割後の
  各部分式が再帰的に本分岐へ到達する形なので複合式（`8'd3 + 8'd2`等）も壊さない
- 修正2: `ElabCtx.scope_params`の値型を`u64`から`(u64, u32)`（値, 幅）に変更。
  新設`hir_const_width`ヘルパー（宣言側HIR式が`Const`/`SignedConst`直書きならその幅、
  それ以外は既定32bit）で幅を推定し、`register_param`呼び出し4箇所
  （通常param/localparam、generate内local、genvar）を更新。`lower_expr`の
  `HirExpr::Net`アームは推定幅で`LogicVal::new`するよう変更
- 回帰テスト`tests/integration/cases/localparam_midmodule/`を新規追加。picorv32.vと
  同じ「alwaysブロックの後でone-hot状態localparamを宣言→case文で比較」パターンを
  2状態分（state_fetch/state_exec）用意。iverilog実出力から`expected.stdout`を作成し、
  `test_localparam_midmodule`・`compare_localparam_midmodule`の両方に登録

### Result

✅ `cargo build --workspace`成功
✅ `cargo test --workspace`全通過（新規`test_localparam_midmodule`/
   `compare_localparam_midmodule`含む、既存テストへの回帰なし、計30テスト）
✅ 最小repro・新規テストケース共にrverilogとiverilogの出力がbit-exact一致
   （`fetch`→`exec`の順でstate遷移し、正しいcase分岐を通過）
✅ picorv32.vスモークチェック: `cpu_state_*`（8箇所）を含む`unresolved net/param`警告が
   解消したことを確認（11件→`WITH_PCPI`/`regfile_size`/`regindex_bits`/`irqregs_offset`のみに減少）
⏳ picorv32.vのフルシミュレーションは未達成（`--max-time 1000`でも停止せずハング）。
   新たに判明した残課題: モジュール冒頭の`localparam integer irqregs_offset =
   ENABLE_REGS_16_31 ? 32 : 16;`等、三項演算子・`||`・`*`・parenを含む複雑な定数式は
   `parse_simple_const_expr`の対応範囲外で依然`unresolved`のまま。A10 は同日に
   `fix/binop-precedence-a10` から取り込み済みであり、残るフルシミュレーションの
   ブロッカーはこの複雑な定数式の対応である
- PLAN.md 実装課題A節のA11を対応済み（範囲限定）に更新、未解決の残課題を明記

### Next

- localparamの複雑な定数式（三項演算子・論理演算子・乗算・括弧）対応。
  `parse_simple_const_expr`をこれ以上拡張するか、正式な`ConstantExpression`
  AST（sv-parserの`ConstantExpressionBinary`/`ConstantExpressionTernary`等、
  `Expression`とは別の並行文法）を使う`lower_constant_expression`を新設するか、
  方針をユーザーと相談
- 上記対応後、picorv32.vスモークチェックを再実行してフルシミュレーション到達を確認
- 従来からの残タスク（D: 連続代入のsensitivity駆動化、B: サイレントスキップの診断化、
  E: fmt/clippyジョブのCI追加）

## 2026-07-30

### Task

A11残課題（localparamの複雑な定数式：三項演算子・`||`・`*`・括弧）対応。
`feat/m1-milestone`から新規ブランチ`fix/localparam-const-expr-a11`を切って着手。
ユーザー指示により実装はCodex（`codex:rescue`サブエージェント経由）に委任した。

### 事前調査（サブエージェント使用）

実装前にサブエージェントで根本原因を調査した結果、「専用パーサが無い」ことが原因ではなく
**配線漏れ**と判明:

- localparam/parameter宣言値は文法上すでにsv-parserの`ConstantParamExpression`
  （正式なConstantExpression系AST）としてパースされている
- 三項・二項・単項演算子を正しくHIRの`Expr::Cond`/`Bin`/`Un`へ変換する
  `lower_constant_expr`/`lower_constant_primary`（`lower.rs:541-599`）は
  genvar/generate文脈向けに**既に実装済み**で動いていた
- しかしlocalparam宣言・parameterポートのデフォルト値・インスタンスのparam
  override（計5箇所）はこの専用ラダーを使わず、生テキストを自前の簡易パーサ
  `parse_simple_const_expr`に丸投げしており、整数リテラル・`+`/`-`分割・
  `$clog2(...)`しか認識できず三項演算子等を識別子と誤解釈していた
- 加えて`lower_constant_expr`のCE::Binaryアーム自体もA10と同型の演算子優先順位
  バグ（sv-parserの`constant_expression_binary`が`expression_binary`と同じ
  非優先順位パーサ構造）を内包していた
- elab側`eval_const_hir_with`の`HirBinOp`畳み込みも`LogAnd`/`LogOr`/`CaseEq`/
  `CaseNe`/`BitNand`/`BitNor`/`BitXnor`が未対応だった

この調査結果から、新規ConstantExpression専用パーサは作らず「既存の
`lower_constant_expr`系を5つの未配線箇所に繋ぐ」方針をユーザーと確認し、
Planモードで詳細な実装計画（変更箇所・行番号・コード）を作成、承認を得た。

### What was done（Codexに委任・実装完了を確認）

- `crates/frontend/src/lower.rs`:
  - `lower_constant_expr`のCE::Binaryアームを、既存の`flatten_binary_chain`/
    `build_binop_tree`/`binop_precedence`（A10で導入した汎用実装、変更不要）を
    再利用する`flatten_constant_binary_chain`（新設）ベースの実装に置換
    （優先順位バグ対策）
  - ブリッジ関数を新設: `lower_constant_mintypmax`（`lower_constant_primary`の
    重複コードを関数化）、`lower_constant_param_expr`（`ConstantParamExpression`
    →`Expr`）、`lower_mintypmax`・`lower_param_expr`（`MintypmaxExpression`/
    `ParamExpression`→`Expr`、既存の優先順位対応済み`lower_expression`に委譲）
  - `lower_localparam`・`lower_param_port_list`（2箇所）・`lower_param_overrides`
    （2箇所）、計5箇所の`parse_simple_const_expr(text.trim())`を型付き
    ブリッジ関数呼び出し＋`?`伝搬に置換（`parse_simple_const_expr`自体は
    配列次元レンジの文字列分割で引き続き使用するため削除せず）
- `crates/elab/src/elaborate.rs`: `eval_const_hir_with`の`HirBinOp`畳み込みに
  `LogAnd`/`LogOr`/`CaseEq`/`CaseNe`/`BitNand`/`BitNor`/`BitXnor`を追加
- 新規回帰テスト`tests/integration/cases/localparam_const_expr/`
  （三項演算子・`||`・`*`・括弧を含むlocalparam定数式、picorv32.vの
  `irqregs_offset`/`regfile_size`/`WITH_PCPI`と同型パターン）を追加、
  iverilog実出力から`expected.stdout`作成、`test_localparam_const_expr`/
  `compare_localparam_const_expr`として登録

Codex側の実装完了後、こちらで独立に検証:

- `git diff`で全変更内容をレビュー、計画通りの実装であることを確認
- `cargo build --workspace`成功（既存の未使用関数警告のみ、新規warning無し）
- `cargo test --workspace`全通過（68テスト、新規2件含む、既存回帰なし）
- `iverilog`で新規テストケースの実出力を独立に再生成し、`expected.stdout`と
  一致することを確認
- picorv32.vスモークチェック（`timeout 15 rverilog --top picorv32 picorv32.v
  --max-time 10`）: パース8モジュール成功、elaboration完了、`unresolved
  net/param`警告が0件（従来の`WITH_PCPI`/`regfile_size`/`regindex_bits`/
  `irqregs_offset`の4件が解消）を確認。「Running simulation」まで到達後
  timeoutでkill（フルシミュレーションのハングは本タスクと独立の既知課題、
  下記Next参照）

### Result

✅ A11残課題（localparamの複雑な定数式）解消。`cargo build`/`cargo test`
   （68テスト）全通過、既存回帰なし
✅ picorv32.vの`unresolved net/param`警告が完全に0件になったことを確認
   （2026-07-29時点の残4件がすべて解消）
⏳ picorv32.vのフルシミュレーションは`--max-time`指定でも停止せずハングする
   （elaboration完了後、"Running simulation"の段階で発生。A10・A11とは別の
   未特定の原因、次の課題）

### Next

- picorv32.vのフルシミュレーションハングの原因調査（A10・A11とは独立、
  elaboration完了後の実行時の問題と推定）
- 従来からの残タスク（D: 連続代入のsensitivity駆動化、B: サイレントスキップの
  診断化、E: fmt/clippyジョブのCI追加）

## 2026-08-03

### Task

PLAN.md推奨着手順7番: picorv32.vフルシミュレーションのハング原因調査・修正。
`feat/m1-milestone`から新規ブランチ`fix/picorv32-sim-hang`を切って着手。ユーザー指示に
より実装はCodex（`codex:rescue`サブエージェント経由）に委任した。

### 事前調査（自分で実施、デバッグ計装によるボトムアップ調査）

Codexに委任する前に、こちらでデバッグ用のeprintln計装を`interp.rs`に一時的に追加して
原因を特定:

- `exec_proc`のACTIVE region drainループ・単一プロセスのstepループそれぞれにイテレーション
  数カウンタを仕込んだが、いずれも閾値（50万〜500万回）に到達せず、素朴な「単純な
  ビジーループ」ではないことが判明
- `eval_expr`にグローバルなatomicコールカウンタを追加したところ、20秒で2億8千万回超の
  呼び出しが発生しているにもかかわらず収束しないことを確認（再帰深度は1〜5程度で
  暴走はしていない）→ 大量だが正当な処理が回り続けている状態
- プロセスごとのstepガード（500万ステップ）を仕込んで発火させ、フレーム情報とscope名を
  出力させたところ、ハングしているのは全てpicorv32トップモジュール直下の
  `kind=Always, sensitivity=All`（＝`always @*`）プロセスであることを特定
- **確定原因1（重大・確実）**: `crates/sim/src/interp.rs`の`exec_proc`、
  `StepResult::Done`ハンドリング（alwaysプロセス完了時に次のイベント待ちへ戻す処理）が
  `Sensitivity::Items`かつ非空の場合のみ`event_waiters`に登録してreturnする一方、
  `Sensitivity::All`（`always @*`）の場合はどの分岐にもマッチせず素通りし、コメント
  「always without sensitivity: loop immediately」の通り本体を無条件に即再実行して
  しまう。つまり`always @*`ブロックは一度実行完了すると二度とイベント待ちに入らず、
  トリガー条件を一切見ずに無限に自分の本体を再実行し続ける完全なビジーループだった
- この最小修正（`Sensitivity::All`も`event_waiters`へ登録）だけでは別の問題が露呈する
  ことも計装で確認: `trigger_sensitivity`の`Sensitivity::All => any_change`（設計内の
  どのネットが変化しても無条件に起床）と、非インクリメンタルな`eval_conts`
  （全連続代入を毎回総当たりで最大200回再評価するbrute-force fixed-pointループ、
  D節で既知の課題）が組み合わさり、picorv32規模（数百ネット・多数の`always @*`）の
  設計では実質収束せずハングし続ける
- 調査終了後、デバッグ計装は全て除去し、pristineな状態に戻してからCodexに実装を委任した
  （`git diff`が空であることを確認済み）

### What was done（Codexに委任・実装完了を確認）

上記調査結果を基に、3点の実装をCodexへ依頼:

1. **`always @*`の自動センシティビティリスト化（本命の修正）**:
   `crates/elab/src/elaborate.rs`に`collect_sensitivity_stmt`/`collect_sensitivity_expr`/
   `collect_sensitivity_lvalue`を新設。`always @*`の本体（Stmt/Expr木）を再帰的に走査し、
   右辺値として読み出されるネットID集合を収集（If/Case/代入/演算/連結/動的添字・部分選択/
   関数呼び出しのlowering済み本体まで対応、LHS自体は含めないが動的添字式は含める）。
   収集した集合から`Sensitivity::Items(edge: None のエントリ集合)`を生成し、
   `Sensitivity::All`の代わりにプロセスへ格納。ネットが1つも見つからない場合は安全側に
   倒して従来の`Sensitivity::All`にフォールバック
2. **`exec_proc`の`Sensitivity::All`対応（最小修正、必須）**: フォールバック時にも
   正しく動くよう、`Sensitivity::All`の場合も完了時に必ず`event_waiters`へ登録して
   returnするよう修正（元のバグそのものの修正）
3. **`eval_conts`の変化判定改善**: 連続代入の変化判定をRHS評価値の幅ではなく、
   `write_lvalue`後にLHSから読み戻した実値で比較するよう修正（幅差による見せかけの
   固定点非収束を防止）
4. **`--max-time`の配線（独立した既存バグ、安全弁として修正）**: `crates/cli/src/cli.rs`の
   `max_time`フィールドがどこにも使われていなかった問題を修正。`Interpreter`に
   `max_time: Option<u64>`を持たせ`set_max_time`で設定、`run()`が未来イベントの時刻が
   `max_time`を超える箇所で`"sim: reached --max-time {N}, stopping"`を出して正常終了する
   よう`crates/cli/src/driver.rs`から配線
5. 新規回帰テスト`tests/integration/cases/always_star/`（`always @*`が入力変化のたびに
   正しく再トリガーされることをiverilog実出力とbit-exact比較で確認）・
   `tests/integration/cases/max_time/`（`always`が永久に停止しない設計で`--max-time`が
   実際にプロセスを止めることを確認、iverilog比較は無し）を追加、
   `crates/cli/tests/integration.rs`/`iverilog_compare.rs`に登録

Codex側の実装完了後、こちらで独立に検証:

- `git diff --stat`で変更ファイル・行数を確認（`crates/elab/src/elaborate.rs` +77/-0、
  `crates/sim/src/interp.rs` +33/-12、`crates/cli/src/driver.rs` +1、テスト関連含め
  計127行追加・12行削除＋新規テスト4ファイル）
- `cargo build --workspace`成功（既存の`dead_code`警告3件のみ、新規警告なし）
- `cargo test --workspace`全通過（68テスト: 統合テスト20件・iverilog比較18件を含む、
  既存テストへの回帰なし）
- `timeout 30 cargo run -p rverilog-cli --release -- --top picorv32 picorv32.v
  --max-time 200`を実行し、**0.55秒で正常終了（exit=0）**することを確認
  （クロック・テストベンチが無いRTL単体のため、`--max-time`到達前に未来イベントが
  尽きて自然終了。ハング・timeoutなし）
- `samples/counter4`・`samples/fifo_sync`（M1受入れサンプル）を再実行し、VCD生成・
  `$finish`による正常終了を確認（回帰なし）

### Result

✅ picorv32.vフルシミュレーションのハング（PLAN.md推奨着手順7番）を解消。原因は
   `always @*`が完了後に二度とイベント待ちへ戻らず無限ビジーループする重大バグで、
   自動センシティビティリスト化により根本修正
✅ 独立した既存バグだった`--max-time`未配線も安全弁として修正
✅ `cargo build`/`cargo test`（68テスト）全通過、既存回帰なし
✅ picorv32.vが0.55秒で正常終了することを確認（従来はtimeoutで強制終了するまで
   ハングしていた）
✅ M1受入れサンプル（counter4・fifo_sync）の回帰なしを確認

### Next

- PLAN.md 実装課題A節にA12として今回のバグを記録
- 従来からの残タスク（D: 連続代入のsensitivity駆動化の残り、B: サイレントスキップの
  診断化、E: fmt/clippyジョブのCI追加、picorv32.vにテストベンチ・クロック生成を
  追加した上でのフル命令実行シミュレーション確認）

## 2026-08-12

### Task

PLAN.md推奨着手順8番: 連続代入(`assign`)のsensitivity駆動化 + sim死コード整理。
`feat/m1-milestone`から新規ブランチ`feat/cont-assign-sensitivity`を切って着手。
ユーザー指示により実装はCodex（`codex:rescue`サブエージェント経由）に委任した。

### 事前調査（自分で実施）

Codexに委任する前にこちらで現状コードを調査:

- `crates/sim/src/interp.rs`の`eval_conts()`は、ACTIVE regionが空になるたびに
  design内の全`assign`文を無条件に最大200回、値が変化しなくなるまで再評価する
  brute-force固定点ループだった（`design.conts.clone()`を毎回丸ごと舐める）。
  発振する組み合わせループがあっても打ち切り時に警告が一切出ない
- `always @*`についてはA12（2026-08-03解消）で既に「本体を静的解析して読み出し
  ネット集合を収集し、そのネットが変化したときだけ起床する」sensitivity駆動の仕組み
  （`elab/src/elaborate.rs`の`collect_sensitivity_expr`/`collect_sensitivity_stmt`）が
  実装済みで、同じインフラを連続代入にも再利用できると判断
- `ElaboratedDesign`には既に`sensitivity_table: IndexMap<u32, Vec<u32>>`
  （net.0→\[process.0\]）というプロセス用逆引きテーブルがあるが、interp.rs側は
  これを一切読まず`event_waiters`の線形スキャンで独自判定していることを確認
  （今回のスコープ外の別件と判断し、触らず）。連続代入専用に同型の新規テーブル
  `cont_sensitivity`（net.0→\[cont_id\]）を追加する方針とした
- `collect_sensitivity_expr`/`collect_sensitivity_stmt`は`Expr`/`Stmt`の全バリアントを
  網羅したexhaustive matchで、実装上は常に`true`を返す（「false=fallback」経路は
  現状到達不能）ことを確認。連続代入用に特別なfallback処理は不要と判断
- `crates/sim/src/scheduler.rs`・`crates/sim/src/systask.rs`（TODOスタブ、
  `interp.rs`と実装重複）はワークスペース全体をgrepしてもこの2ファイル以外から
  一切参照されていないことを確認、削除対象とした
- 上記調査結果を基にPlanモードで詳細な実装計画（変更箇所・関数シグネチャ・
  疑似コード）を作成、承認を得た

### What was done（Codexに委任・実装完了を確認）

- `crates/mir/src/ir.rs`: `ElaboratedDesign`に`cont_sensitivity: IndexMap<u32, Vec<u32>>`
  フィールドを追加
- `crates/elab/src/elaborate.rs`: `elaborate()`内、`elab_module`完了後・
  `ElaboratedDesign`構築前に、`ctx.conts`を走査して各`ContAssign`のRHS式に
  `collect_sensitivity_expr`を適用し`cont_sensitivity`を構築するコードを追加
- `crates/sim/src/interp.rs`:
  - `Interpreter`に`cont_dirty: VecDeque<u32>`（dirtyワークリスト）・
    `cont_queued: Vec<bool>`（重複enqueue防止ビットセット）を追加
  - `run()`冒頭でプロセスseedの直後に全cont_idを`mark_cont_dirty`し初期dirty化
    （起動時は全conts未評価のため従来の全件評価動作を維持）
  - `trigger_sensitivity()`の`any_change`判定直後に、変化があった場合のみ
    `design.cont_sensitivity`から該当cont_idを引いて`mark_cont_dirty`する処理を追加
  - `eval_conts()`を全面置換。dirtyワークリストをpopしながら評価し、値が実際に
    変化した場合のみ`trigger_sensitivity`を呼ぶ（連鎖伝搬は`mark_cont_dirty`が担う）。
    pop回数上限（`conts.len() * 64`、最低1000）を設け、超過時は
    `eprintln!("sim: continuous assignment did not converge (possible combinational
    loop) ...")`で警告を出してから打ち切るよう変更（従来は無警告だった）
- `crates/sim/src/scheduler.rs`・`crates/sim/src/systask.rs`を削除、
  `crates/sim/src/lib.rs`から該当`pub mod`/`pub use`を除去
- 新規回帰テスト`tests/integration/cases/cont_loop/`（`assign a = ~a;`という
  組み合わせループが、warningを出しつつハングせず`$finish`まで到達することを確認。
  真の組み合わせ発振はiverilog側にも有効な比較対象がないため、A12時の`max_time`
  テストと同じ前例に倣い`crates/cli/tests/integration.rs`にのみ登録、
  `iverilog_compare.rs`には登録せず）を追加

Codex側の実装完了後、こちらで独立に検証:

- `git diff --stat`で計画通りの変更であることを確認
  （7ファイル、+72/-205行、`scheduler.rs`/`systask.rs`削除込み）
- `cargo build --workspace`成功（既存の`lower.rs`未使用関数警告3件のみ、新規警告なし）
- `cargo test --workspace`全通過（39テスト: 統合テスト21件・iverilog比較18件を含む、
  新規`test_cont_loop`含め既存回帰なし）
- `samples/counter4`・`samples/fifo_sync`（M1受入れサンプル）を実行しVCD生成・
  `$finish`による正常終了を確認（回帰なし）
- `picorv32.v`スモークチェック（`--max-time 200`）を再実行し、**0.57秒で正常終了**
  （A12解消時の0.55秒からほぼ同等、性能劣化・デッドロックなし）を確認
- `cont_loop`テストのdutを直接実行し、
  `sim: continuous assignment did not converge (possible combinational loop) at
  time 0, stopping after 1001 evaluations`という警告が実際に出力され、その後
  `$finish at time 1`まで到達することを目視確認

### Result

✅ 連続代入がsensitivity駆動（dirtyワークリスト方式）になり、brute-force総当たり
   固定点ループを解消。組み合わせループも警告付きで安全に打ち切られるようになった
✅ `sim/src/scheduler.rs`・`sim/src/systask.rs`の死コードを削除
✅ `cargo build`/`cargo test`（39テスト）全通過、既存回帰なし
✅ M1受入れサンプル（counter4・fifo_sync）・picorv32.vスモークチェックいずれも
   回帰なし（picorv32.vは0.57秒で正常終了、性能劣化なし）
✅ PLAN.md 実装課題D節・推奨着手順8番を完了マークに更新

### Next

- 従来からの残タスク（B: サイレントスキップの診断化、E: fmt/clippyジョブのCI追加、
  `elab/src/width.rs`のcontext-determined幅推論再設計、picorv32.vにテストベンチ・
  クロック生成を追加した上でのフル命令実行シミュレーション確認）
- ブランチ`feat/cont-assign-sensitivity`はローカルで検証完了、push・PR作成は
  ユーザーの明示確認待ち（2026-08-12時点でPR #22としてオープン・CIグリーン、
  マージはユーザー判断待ち）

## 2026-08-12 (2)

### Task

PLAN.md推奨着手順9番: B（サイレントスキップの診断化）。`defparam`・`specify`ブロック・UDP
（user-defined primitive）が`lower.rs`の`_ => {}` catch-allで診断なしに無言スキップされる
問題を`FrontendError::UnsupportedConstruct`エラーに置換する。`feat/m1-milestone`から新規
ブランチ`fix/silent-skip-diagnostics-b`を切って着手。ユーザー指示により実装はCodex
（`codex:codex-rescue`サブエージェント経由）に委任した。

前提として、推奨着手順8番（D: 連続代入のsensitivity駆動化）は前回セッションで既に実装済み・
PR #22としてオープン済み（CIグリーン）であることを確認したが、ユーザー判断でマージは保留し
B番のみ着手する方針とした。

### 事前調査（自分で実施）

Codexに委任する前に、`sv-parser-syntaxtree-0.13.5`のソース（`ModuleCommonItem`/
`NonPortModuleItem`/`ModuleOrGenerateItem`/`ModuleOrGenerateItemDeclaration`各enum定義）を
直接確認し、対象箇所を特定:

- `defparam`は文法上`ModuleOrGenerateItem::Parameter`（`ParameterOverride`が
  `ListOfDefparamAssignments`をラップ）としてパースされ、`process_mogi`関数
  （`crates/frontend/src/lower.rs`）の末尾`_ => {}`に落ちて無言スキップされていた
- UDPインスタンス化は`ModuleOrGenerateItem::Udp`として同じく`process_mogi`の`_ => {}`に
  落ちていた
- `specify`ブロックは`NonPortModuleItem::SpecifyBlock`として、モジュール直下のアイテムを
  走査する`lower_nonport_items`・`lower_module_items`（ANSI/non-ANSIポート形式それぞれに
  対応する2つの並行実装）の`_ => {}`に落ちていた
- スコープ外と判断した箇所も明確化: `process_generate_mogi`（generate block内、`()`を返す
  設計で子呼び出しのエラーも既存で無視される別問題）、`lower_decl`内の`_ => {}`
  （DPI/class/covergroup等、このプロジェクトの対象外のSystemVerilog専用機能）、ゲート
  プリミティブの`_ => {}`（switch/cmos/pass/pullup/pulldown、既にコメントで明記済みの
  別課題）、トップレベルの`primitive`宣言自体の無言スキップ（UDPインスタンス化のエラー化で
  実質カバーされるため見送り）

### What was done（Codexに委任・実装完了を確認）

- `crates/frontend/src/lower.rs`: 4箇所に`unsupported()`ヘルパー（既存）を使った
  明示的エラー分岐を追加
  - `process_mogi`: `MOGI::Parameter(_) => return Err(unsupported("defparam"))`・
    `MOGI::Udp(_) => return Err(unsupported("UDP instantiation"))`
  - `lower_nonport_items`・`lower_module_items`（各1箇所、計2箇所）:
    `NonPortModuleItem::SpecifyBlock(_) => return Err(unsupported("specify block"))`
- 新規回帰テスト`crates/cli/tests/unsupported_construct.rs`（プロジェクト初のサブセット外
  構文の負パステスト）を追加。`rverilog_frontend::parse_files`を一時ファイル経由で直接呼び、
  defparam・specifyブロック・UDPインスタンス化（`primitive`宣言込み）の3パターンそれぞれで
  `Err(FrontendError::UnsupportedConstruct(_))`が返り、エラーメッセージに該当語
  （"defparam"/"specify"/"UDP"）を含むことを確認

Codex側の実装完了後、こちらで独立に検証:

- `git diff`で変更内容が計画通り4箇所ちょうどに絞られていること（スコープ外と指定した箇所への
  変更がないこと）を確認
- `cargo build --workspace`成功（既存の`lower.rs`未使用関数warning3件のみ、新規warningなし）
- `cargo test --workspace`全通過（71テスト: 既存68テスト＋新規3テスト、回帰なし）
- `samples/counter4`（`--top tb_counter4`）・`samples/fifo_sync`（`--top tb_fifo_sync`）を
  それぞれ実行し、VCD生成・`$finish`による正常終了を確認（回帰なし）

### Result

✅ B（サイレントスキップの診断化）完了。`defparam`・`specify`ブロック・UDPインスタンス化の
   計4箇所を`FrontendError::UnsupportedConstruct`エラーに置換
✅ プロジェクト初のサブセット外構文の負パステストを追加（`unsupported_construct.rs`、3件）
✅ `cargo build`/`cargo test`（71テスト）全通過、既存回帰なし
✅ M1受入れサンプル（counter4・fifo_sync）の回帰なしを確認
✅ PLAN.md 実装課題B節・推奨着手順9番を対応済みに更新

### Next

- PR #22（D: 連続代入のsensitivity駆動化）はこのセッション内でマージ済み
  （2026-08-12、`feat/m1-milestone`へ取り込み済み）
- E: fmt/clippyジョブのCI追加（負パステストは今回着手済みだが、fmt/clippy自体は未着手）
- picorv32.vにテストベンチ・クロック生成を追加した上でのフル命令実行シミュレーション確認
  （推奨着手順11番、未着手）

## 2026-08-13

### Task

新規セッション開始。PLAN.md・DIARY.mdを読み込み、前回セッションの続きとして実装を進める。
まずローカルリポジトリの状態確認から着手。

### 事前調査（自分で実施）

- ローカル`feat/m1-milestone`が`origin/feat/m1-milestone`より2コミット遅れていることを
  `git fetch`で発見（PR #22マージコミットが未反映）。`git pull --ff-only`で追従
- `gh pr list`でPR #23（B: サイレントスキップ診断化、2026-08-12オープン、CIグリーン）が
  未マージのまま残っていることを確認。`gh pr view 23`で`mergeStateStatus: DIRTY`・
  `mergeable: CONFLICTING`を検出
- 原因調査: `git merge-base origin/fix/silent-skip-diagnostics-b
  origin/feat/m1-milestone`が8f8f2cb（PR #21直後、PR #22マージ前）であることを確認。
  PR #23のブランチはPR #22（D）がマージされる前に切られたため、そのままマージすると
  D（連続代入sensitivity駆動化・`scheduler.rs`/`systask.rs`削除・`cont_loop`テスト）が
  丸ごとリバートされてしまう状態だった

### What was done（自分で実施、コンフリクト解消のためCodex委任は不要と判断）

- ローカルの`fix/silent-skip-diagnostics-b`ブランチを`origin`の最新へ`reset --hard`
- `feat/m1-milestone`をマージ。コードファイル（`interp.rs`/`elaborate.rs`/`ir.rs`/`lib.rs`/
  `scheduler.rs`/`systask.rs`/テストファイル）はDとBが別々の箇所を変更していたため
  自動マージで無競合。競合したのは`DIARY.md`・`PLAN.md`の2ファイルのみ（両ブランチが
  同じ「2026-08-12」節に別々のタスク記録を追記していたため）
- `PLAN.md`: 推奨着手順8〜10番の記述をD・B両方「完了」と一貫する形に統合
- `DIARY.md`: 2026-08-12のD（連続代入sensitivity駆動化）作業記録とB（サイレントスキップ
  診断化）作業記録を、実際の時系列順（D先行→B後続、B側の記録内に明記あり）で
  「## 2026-08-12」と「## 2026-08-12 (2)」の2節に整理し直し、両者の「Next」節の
  記述矛盾（B側が「D=マージ判断保留中」と書いていた）を「Dはマージ済み」に修正
- `cargo build --workspace`（新規warningなし）・`cargo test --workspace`
  （72テスト全通過、既存回帰なし）で検証後、マージコミットを作成しpush
- PR #23のCIが再度グリーンになったことを確認（ユーザーに確認の上）マージ・
  ブランチ削除（`gh pr merge 23 --merge --delete-branch`）

### Result

✅ PR #22（D）・PR #23（B）双方が`feat/m1-milestone`へ正しく統合された状態を確認
   （Dの変更がBのマージで消えるという事故を未然に防止）
✅ `cargo build`/`cargo test`（72テスト）全通過、既存回帰なし
✅ PLAN.md推奨着手順8・9番（D・B）を完了として確定

### Next

- E: fmt/clippyジョブのCI追加、負パステストの拡充（推奨着手順10番、着手予定）
- picorv32.vにテストベンチ・クロック生成を追加した上でのフル命令実行シミュレーション確認
  （推奨着手順11番）

## 2026-08-13 (2)

### Task

PLAN.md 推奨着手順10番、E（fmt/clippy ジョブの CI 追加と負パステストの拡充）を実施。

### What was done

- `.github/workflows/ci.yml` に `cargo fmt --check` を実行する `fmt` ジョブと、
  `cargo clippy --workspace --all-targets -- -D warnings` を実行する `clippy` ジョブを追加
- ワークスペース全体へ `cargo fmt` を適用し、既存の整形差分を解消
- clippy の指摘を解消。`div_ceil`・`is_none_or`・`checked_div` 等の標準APIへの置換、
  不要なキャスト・未使用ヘルパーの削除、イテレータ記法への機械的な整理を実施
- `unsupported_construct.rs` に while/repeat/forever ループと `**` 演算子が
  `UnsupportedConstruct` を返す負パステストを追加
- 式中の関数呼び出しは実装を確認した結果 `Expr::Call` として受理されるため、受理を確認する
  回帰テストを追加
- PLAN.md の実装課題E節と推奨着手順10番を完了として更新

### Result

✅ fmt・clippyをCIで強制できる状態になった
✅ サブセット外のループ文・冪乗演算子の診断を回帰テストで保護し、関数呼び出し式の現行仕様も確認

### Next

- picorv32.v にテストベンチ・クロック生成を追加した上でのフル命令実行シミュレーション確認
  （推奨着手順11番）

## 2026-08-13 (3)

### Task

PR #25（E: fmt/clippy CI追加）のdiffレビュー中、`cargo fmt`が既存の英語コメント6箇所
（`crates/cli/tests/common/mod.rs`・`crates/frontend/src/lower.rs`・
`crates/hir/src/design.rs`・`crates/sim/src/interp.rs`・`crates/vcd_out/src/dump.rs`）を
再整形して表面化させたのをユーザーが発見し「コメントは日本語で」と指示。プロジェクト規約
（[[feedback_comment_language]]）に沿って該当コメントを日本語へ翻訳した
（`vcd_out/dump.rs`はついでに同ファイル内の他の未翻訳コメントも含めて修正）。

### What was done

- 該当5ファイルの英語コメントをすべて日本語へ翻訳（ロジック変更なし）
- `chore/fmt-clippy-ci`ブランチに追加コミットしてpushしたが、**PR #25はその追加pushより前に
  既にマージ済み**だったため、この修正はfeat/m1-milestoneへ反映されていなかった
  （リポジトリの`allow_auto_merge`はfalse・rulesetも無しのため自動マージではなく、
  ユーザーがCIグリーンを見て直接マージしたタイミングの問題と推定）
- 取りこぼしに気付き、`feat/m1-milestone`から新規ブランチ`fix/ja-comments-followup`を切って
  該当コミットをcherry-pick、独立に`cargo build`/`fmt --check`/`clippy -D warnings`/
  `cargo test`（80テスト）を再検証してからPR #26として切り出し、マージした

### Result

✅ 該当コメント6箇所（+αで`dump.rs`内の残り5箇所）すべて日本語化完了、`feat/m1-milestone`へ反映
✅ `cargo build`/`cargo fmt --check`/`cargo clippy -D warnings`/`cargo test`（80テスト）
   全通過を確認

### Next

- 教訓: PRに追加pushする場合、マージ前に必ず最新のCI結果・マージ状態を再確認してから
  次のアクション（追加push→確認待ち、等）に進むこと。今回のように「push直後にはまだ
  マージされていない」という前提でユーザー確認を挟んでも、確認のやり取りの間に
  ユーザー側で先にマージが完了しているケースがあり得る
- picorv32.v にテストベンチ・クロック生成を追加した上でのフル命令実行シミュレーション確認
  （推奨着手順11番、次のタスク）

## 2026-08-13 (4)

### Task

PLAN.md 推奨着手順11番として、PicoRV32を実際にクロック駆動し、手アセンブルしたRV32I命令列を
実行するエンドツーエンドのスモークテストを追加する。

### 原因調査

- テストベンチは`addi x1, x0, 5`、`addi x2, x0, 37`、`add x3, x1, x2`、`lui x4, 0x10000`、
  `sw x3, 0(x4)`を実行し、Icarus Verilogでは時刻205で`RESULT=42`とPASSを出力した
- rverilogでは`#2000`のtimeoutまで進み、初期reset中にもかかわらず`mem_valid`がXのままで最初の
  命令fetchを発行しなかった。PicoRV32固有ではなく、X/Zを含む論理値の真偽判定に共通する不具合だった
- `1 || X`はIEEE 1364では1だが、`LogicVal::log_or`が既知値でないことを先に検査してXを返していた。
  さらにif/while/三項演算子の実行器も「全ビット既知かつ非ゼロ」でのみ真と扱い、既知の1ビットを含む
  X混在値をどちらの分岐にも進めない、または三項演算子でXへ落とす状態だった

### What was done

- `LogicVal::is_true`を追加し、a=1かつb=0の既知1ビットが一つでもあれば真とするIEEE短絡規則を実装
- `&&`は全ゼロ判定を、`||`は既知1判定をX/Z判定より先に評価するよう修正
- 通常スケジューラと関数/タスク同期実行のif/while、および三項演算子を`is_true`に統一
- `logical_x_shortcircuit`回帰テストを追加し、`1 || X`、`0 && X`、X混在の真条件、reset形状をiverilogと
  bit-exact比較で保護
- ISCライセンスのPicoRV32本体をテストケースへコピーし、ゼロウェイトメモリを持つ`picorv32_smoke`を追加。
  実命令実行のPASS表示を機能テストとiverilog比較テストの双方に登録

### Result

✅ A13（論理X真偽判定のIEEE短絡規則違反）を修正し、再発防止テスト（`logical_x_shortcircuit`、
   機能テスト・iverilog比較テスト双方）が通過することを確認。既存回帰なし
⚠️ PicoRV32スモークテストはIcarusでは`RESULT=42`を確認したが、rverilogでは不正命令トラップ後に
   timeoutする。実命令実行の確認は継続課題。`test_picorv32_smoke`/`compare_picorv32_smoke`は
   `cargo test --workspace`を赤くしないよう`#[ignore]`付きで登録（理由をコメントに明記）、
   テストベンチ・PicoRV32本体・登録コードは次のセッションでそのまま使える状態で残した

### Next

- PicoRV32が最初の命令fetch前に不正命令トラップへ入る実行器側の原因を調査し、
  `#[ignore]`を外してスモークテストを通す（推奨着手順11番、継続）

## 2026-08-15

### Task

新規セッション開始。PLAN.md・DIARY.mdを読み込み、推奨着手順11番（picorv32.vフル命令実行
シミュレーション確認）の続きを実施。実装はCodexへ委任する運用方針を確認した。

### 原因調査（自分で実施）

- `feat/m1-milestone`を最新化（PR #28マージ分を`git pull --ff-only`で反映）。
- `cargo build -p rverilog-cli --release`でビルドし、`tb_picorv32`を`--max-time 2000`で
  実行、従来通りTIMEOUTすることを確認
- 生成VCDをPythonスクリプトで解析（当初、識別子を1文字と決め打ちして誤った信号対応表を
  作ってしまい「大量に発振している」ように見えたが、`$var`定義を正しくスコープ付きで
  パースし直したところ誤りと判明。234信号中140個が2文字識別子だったため単純な
  `line[1]`パースでは衝突していた）。正しく解析した結果、`dut.clk`は正常にトグルする一方、
  `dut.resetn`/`dut.mem_valid`/`dut.mem_instr`/`dut.mem_addr`はtime=0のX値のまま二度と
  更新されないことを発見
- 最小再現ケース（2階層、`input clk, rst,`という1つの`input`にカンマ区切りで2ポートを
  まとめる書き方）で同一症状（`rst`が子モジュールへ伝搬せず`cnt`がXのまま）を確認。
  ポート宣言を`input clk; input rst;`と分離すると正常動作することも確認し、
  「カンマ区切りANSIポート宣言」が原因と特定
- `crates/frontend/src/lower.rs`に一時的な`eprintln!`デバッグを仕込み、sv-parserの
  実際のAST構造を確認。ヘッダを持つ1番目のポート（`clk`）は`AnsiPortDeclaration::Net`、
  ヘッダを省略した2番目のポート（`rst`）は`AnsiPortDeclaration::Variable`として
  parseされることが判明。`lower_ansi_port_variable`はヘッダが`None`の場合に無条件で
  `PortDirection::Output`へフォールバックしており、IEEE 1364-2001 12.3.3の
  「ヘッダ省略時は直前のポート宣言の方向を継承する」規則に反していた
  （`lower_ansi_port_net`側もヘッダ省略時`PortDirection::Input`固定で、先頭ポート以外では
  本来誤りだが、今回のケースではたまたま影響なし）。picorv32.vの実際のポート宣言
  （`input clk, resetn,`）がまさにこのパターンで、`resetn`が誤ってOutput
  （`NetKind::Reg`）として登録され、elabのインスタンス接続ロジック
  （`elaborate.rs`のポート接続コード）が接続方向を逆に解釈し、親の`resetn`が
  子へ一切伝搬しない不具合になっていた
- 自分でこの原因分析に基づき一度直接修正・動作確認まで行ったが、ユーザーから
  「実装はCodexにやり直させる」との指示を受け、修正をrevertして`fix/ansi-port-direction-inherit`
  ブランチを作成し、原因分析結果をそのままCodexへの依頼文に含めて委任した

### What was done（Codexに委任・実装完了を確認）

- `crates/frontend/src/lower.rs`: `lower_ansi_ports`に`prev_dir`（直前確定方向、
  先頭はIEEE既定のInput）を追加し、`lower_ansi_port`/`lower_ansi_port_net`/
  `lower_ansi_port_variable`（`Paren`分岐も含む）のヘッダ省略時フォールバックを
  ハードコードされたInput/Outputから継承方向`prev_dir`に変更
- 回帰テスト`tests/integration/cases/ansi_port_comma_direction/`
  （`input clk, rst,`パターンの最小再現ケース）を新規追加し、
  `crates/cli/tests/integration.rs`・`crates/cli/tests/iverilog_compare.rs`双方に登録

Codex側の実装完了後、こちらで独立に検証:

- `git diff`で変更内容が依頼した通り（`lower.rs`の該当4関数＋回帰テスト1件）に
  絞られていることを確認
- `cargo build --workspace`成功、`cargo fmt --check`・
  `cargo clippy --workspace --all-targets -- -D warnings`ともに警告なし
- `cargo test --workspace`で81テスト全通過（既存回帰なし、ignore 2件は
  picorv32_smoke関連で意図通り）
- `samples/counter4`・`samples/fifo_sync`（M1受入れサンプル）を実行しVCD生成・
  正常終了を確認（回帰なし）
- picorv32.vスモークテストを手動実行し、VCD上で`dut.resetn`が`#0: X`→`#10: 1`と
  正しく遷移するようになったことを確認。ただし`mem_do_prefetch`が立たない別原因により
  依然TIMEOUTのままで、`picorv32_smoke`の`#[ignore]`は継続

### Result

✅ A14（ANSIポートのカンマ区切り宣言における方向継承バグ、IEEE 1364-2001 12.3.3違反）を
   修正。`input clk, resetn,`のようなpicorv32.v実際の記法で発生していた重大なポート
   伝搬バグを解消
✅ 回帰テスト`ansi_port_comma_direction`を追加し、`cargo test --workspace`
   （81テスト）全通過を確認
✅ picorv32.vで`dut.resetn`が正しく伝搬するようになったことをVCDで確認（部分的前進）
⚠️ picorv32_smokeは`mem_do_prefetch`が立たない別の未特定原因により依然PASS未到達
✅ PLAN.md実装課題A節にA14を追加、推奨着手順11番の進捗を更新

### Next

- `mem_do_prefetch`がpicorv32内部で立たない原因を調査する（cpu_stateは
  `01000000`→`10000000`と遷移しておりFSM自体は動いているように見えるため、
  decoder_trigger周りかmem_do_prefetchの生成条件自体を疑うべき）。原因調査は
  自分で行い、実装はCodexへ委任する運用を継続する（推奨着手順11番、継続）

## 2026-08-15 (2)

### Task

前回セッションに引き続き、推奨着手順11番（picorv32.vフル命令実行シミュレーション確認）を継続。
`mem_do_prefetch`が立たない原因を追う中で、A15・A16という2つの重大バグを発見・対応した。

### 原因調査（自分で実施、A15）

- `feat/m1-milestone`を最新化（PR #29=A14マージ分を`git pull --ff-only`で反映）
- `picorv32.v`に一時的な`always @(posedge clk) $display(...)`デバッグ計装を追加し、
  `resetn`/`cpu_state`/`mem_state`/`mem_do_rinst`/`mem_do_prefetch`/`mem_done`/
  `decoder_trigger`/`mem_valid`/`mem_ready`/`mem_xfer`を毎クロック出力させて調査
- `mem_done`（`wire mem_done = resetn && ((...) || (...)) && (...);`という宣言時代入で
  定義されたワイヤ）が全シミュレーション時間を通じて`z`のまま一切変化しないことを発見
- 最小再現（`wire c = a & b;`のような宣言時代入）で同一症状（`c`が`z`のまま張り付く）を
  再現し、`assign`文に分けて書くと正常動作することを確認
- `crates/frontend/src/lower.rs`の`lower_net_decl`（1077-1099行目付近）を確認したところ、
  `unwrap_all_net_identifiers`でネット名だけを抜き出しており、`NetDeclAssignment`が持つ
  初期化式（sv-parser型定義: `NetDeclAssignment.nodes.2: Option<(Symbol, Expression)>`）を
  一切見ていないことが判明。`wire foo = expr;`はpicorv32.vで数十箇所使われている一般的な
  イディオムであり、これが根本原因の一つと特定した

### What was done（Codexに委任・実装完了を確認、A15）

- ブランチ`fix/net-decl-assignment-dropped`を作成し、原因分析結果をそのままCodexへ委任
- `lower_net_decl`の戻り値を`(Vec<NetDecl>, Vec<ContinuousAssign>)`に変更し、初期化式が
  あれば`ContinuousAssign`（`LValue::Net(name)` ← `lower_expression(expr)`）を生成
- 呼び出し元3箇所（モジュール直下`process_mogi`、generate内`lower_generate_decl`、
  関数/タスクローカル宣言`lower_decl`）全てで`assigns`への合流を配線
- 回帰テスト`tests/integration/cases/net_decl_assignment/`（単一代入・同一宣言内複数代入
  混在）を追加

Codex側の実装完了後、こちらで独立に検証:

- `git diff`で変更が依頼範囲（`lower_net_decl`本体＋3呼び出し元＋回帰テスト）に
  絞られていることを確認
- `cargo build --workspace`成功、`cargo fmt --check`・`cargo clippy -D warnings`とも警告なし
- `cargo test --workspace`で83テスト全通過（既存回帰なし）
- `samples/counter4`・`samples/fifo_sync`回帰なし確認
- デバッグ計装版picorv32.vを再実行し、`mem_done`が`z`から抜け出し`0`/`1`を正しく
  出力するようになったことを確認。ただし`cpu_state`が`cpu_state_fetch`(`01000000`)から
  `cpu_state_trap`(`10000000`)へ一度遷移した後、そこで完全に停止することを新たに発見

### 原因調査（自分で実施、A16）

- `cpu_state_trap`への遷移は想定外（1命令目`addi`は正常な命令のはずでtrapに落ちるのは
  おかしい）。`mem_done`の算出式が`&mem_state`（reduction AND）・`\|mem_state`
  （reduction OR）という2bit信号への単項リダクション演算子を使っていることに着目し、
  デバッグ計装をさらに追加してこれらの部分式を個別出力
- `mem_state=00`（2bit both 0）にもかかわらず`&mem_state=1`・`\|mem_state=1`という、
  本来ありえない結果（両方とも0であるべき）を観測
- 最小再現（`reg [1:0] x; wire o=\|x; wire a=&x;`を4つの`x`パターンで検証）で、
  `o`・`a`が常に同一値になり、かつその値が`x`の全体を無視して`NOT(x[0])`
  （＝`~x`のLSBのみ）と一致するパターンであることを突き止めた
- `crates/frontend/src/lower.rs`の`lower_unary_op`（2408-2417行目）を確認したところ、
  `+`/`-`/`!`/`~`のみ明示的に扱っており、それ以外（単項の`&`/`\|`/`^`/`~&`/`~\|`/`~^`、
  すなわちリダクション演算子全種）は`_ => Ok(UnOp::BitNot)`のcatch-allで無条件に
  `UnOp::BitNot`（ビット反転）へ丸め込まれていることを発見。`mir::ir::UnOp`には
  `RedAnd`/`RedNand`/`RedOr`/`RedNor`/`RedXor`/`RedXnor`が既に定義され
  `logicval.rs`の`reduce_and`/`reduce_or`等の実装も正しいにもかかわらず、frontendから
  一度も生成されず完全に死んでいた。プロジェクト全体でリダクション演算子が発見時点まで
  一度も正しく動作していなかったことを意味する、今回発見した中で最も基礎的で影響範囲の
  広いバグ

### Next

- A16（リダクション演算子の誤lowering）の修正をCodexへ委任する（ブランチは次セッションで
  新規作成）。`lower_unary_op`が実際のトークン文字列（`&`/`~&`/`\|`/`~\|`/`^`/`~^`または
  `^~`）を判定して対応する`UnOp::Red*`を返すよう修正し、回帰テスト
  （`reg [1:0] x; wire o=\|x; wire a=&x;`等、真理表ベース）を追加する
- 修正後、picorv32.vが`cpu_state_trap`を経ずに`cpu_state_ld_rs1`へ正しく進むか再確認し、
  `mem_do_prefetch`が最終的に立つか、picorv32スモークテストがPASSに到達するかを追跡する
  （推奨着手順11番、継続）
推奨着手順11番の継続。前セッションでA14（ANSIポート方向継承バグ）を修正後、
`mem_do_prefetch`が立たない原因を追う中で、A15・A16という2つの重大バグを発見・対応した。
このセッションはA16の対応記録（A15は並行ブランチ`fix/net-decl-assignment-dropped`
（PR #30、本セッション時点で未マージ）で対応済み。本ブランチ`fix/reduction-operator-lowering`は
`feat/m1-milestone`から分岐しているためA15の修正は含んでいない）。

### 原因調査（自分で実施、A15の要約）

- `wire mem_done = resetn && (...) && (...);`という宣言時代入（`assign`文と分けて
  書かず1文にまとめる`NetDeclAssignment`構文）の初期化式が`lower_net_decl`で完全に
  無視され、`mem_done`が永久にZに張り付いていたことをデバッグ計装で発見・修正
  （詳細はPR #30・DIARY該当ブランチの記録を参照）

### 原因調査（自分で実施、A16）

- A15修正後の再検証で、`cpu_state`が`cpu_state_fetch`(`01000000`)から
  `cpu_state_trap`(`10000000`)へ想定外の遷移をして止まることを発見
- `mem_done`の算出式が`&mem_state`（reduction AND）・`\|mem_state`（reduction OR）
  という2bit信号への単項リダクション演算子を使っていることに着目し、部分式を
  個別出力するデバッグ計装を追加
- `mem_state=00`（2bit both 0）にもかかわらず`&mem_state=1`・`\|mem_state=1`という、
  本来ありえない結果を観測
- 最小再現（`reg [1:0] x; wire o=\|x; wire a=&x;`を4パターンのxで検証）で、
  `o`・`a`が常に同一値になり、`x`全体を無視して`NOT(x[0])`（＝`~x`のLSBのみ）と
  一致するパターンであることを突き止めた
- `crates/frontend/src/lower.rs`の`lower_unary_op`を確認したところ、`+`/`-`/`!`/`~`
  のみ明示的に扱っており、それ以外（単項の`&`/`\|`/`^`/`~&`/`~\|`/`~^`、すなわち
  リダクション演算子全種）は`_ => Ok(UnOp::BitNot)`のcatch-allで無条件に`BitNot`へ
  丸め込まれていることを発見。`mir::ir::UnOp`には`RedAnd`等6バリアントが既に定義され
  `logicval.rs`の実装も正しいにもかかわらず、frontendから一度も生成されず完全に
  死んでいた。プロジェクト全体でリダクション演算子が発見時点まで一度も正しく
  動作していなかったことを意味する、今回発見した中で最も基礎的で影響範囲の広いバグ

### What was done（Codexに委任・実装完了を確認、A16）

- ブランチ`fix/reduction-operator-lowering`（`feat/m1-milestone`から分岐）を作成し、
  原因分析結果をそのままCodexへ委任
- `lower_unary_op`が実際のトークン文字列（`&`/`~&`/`\|`/`~\|`/`^`/`~^`/`^~`）を判定して
  対応する`UnOp::Red*`を返すよう修正。未知の演算子は無言フォールバックせず
  `FrontendError::UnsupportedConstruct`エラーに変更
- Codexが独自に発見・対応した波及範囲: `hir::UnOp`（`crates/hir/src/design.rs`）にも
  同6バリアントを追加、`elab::lower_unop`（HIR→MIR変換）と`elab::eval_const_hir_with`
  （コンパイル時定数畳み込み、genvar/localparam文脈で使用）にリダクション演算子の
  評価ロジックを追加（依頼時には明示していなかったが、HIR側にも同名の`UnOp`列挙が
  別途存在し、定数式評価器にも同様の対応が必要だったことをCodex側で正しく特定・対応）
- 回帰テスト`tests/integration/cases/reduction_ops/`（2bit/4bit、AND/NAND/OR/NOR/XOR/
  XNOR全6種、`~^`と`^~`両表記）を追加。このブランチはA15を含まないため、テストは
  `wire o = expr;`ではなく`assign`文の明示形式で記述（A15未統合による誤検知を回避）

Codex側の実装完了後、こちらで独立に検証:

- `git diff`で変更が依頼範囲（`lower_unary_op`本体＋波及した`hir`/`elab`側＋回帰テスト）に
  収まっていることを確認
- `cargo build --workspace`成功、`cargo fmt --check`・`cargo clippy -D warnings`とも警告なし
- `cargo test --workspace`で83テスト全通過（既存回帰なし、ignore 2件は意図通り）
- `samples/counter4`・`samples/fifo_sync`回帰なし確認
- 最小再現（`reg [1:0] x; assign o=\|x; assign a=&x;`）を再実行し、真理表通りの
  正しい結果（`x=00→or=0,and=0`／`x=01→or=1,and=0`／`x=11→or=1,and=1`／
  `x=10→or=1,and=0`）になることを確認
- 同ブランチで`wire o = \|x;`形式（宣言時代入）を試すと依然`o=z`になることを確認
  （A15未統合のため想定通り、リダクション演算子自体のバグではないことを再確認）

### Result

✅ A16（単項リダクション演算子が全て`~`として誤lowingされる、IEEE基本演算子の
   全面的な機能不全）を修正。真理表ベースの回帰テストで全6種を検証
✅ `cargo test --workspace`（83テスト）全通過を確認
⚠️ picorv32.vスモークはA16単独では依然PASS未到達（A15とA16は独立したバグで、
   picorv32.vのフル動作にはおそらく両方の統合が必要。A15マージ後に再確認予定）
✅ PLAN.md実装課題A節にA16を追加

### Next

- PR #30（A15）とこのブランチ（A16）を`feat/m1-milestone`へ統合した上で、
  picorv32.vスモークテストを再実行し、`PASS`まで到達するか、あるいはさらに別の
  未特定バグが残っているかを確認する（推奨着手順11番、継続）
## 2026-08-15 (3)

### Task

前回セッションに続き推奨着手順11番の継続。A15（PR #30）・A16（PR #31）を
ローカルで一時的に統合検証した結果、さらにA17という重大バグを発見・対応した。

### 原因調査（自分で実施、A15・A16の統合検証）

- ローカルに検証専用ブランチ`test/a15-a16-combined`（push対象外）を作成し、
  `feat/m1-milestone`に`origin/fix/net-decl-assignment-dropped`（A15）・
  `origin/fix/reduction-operator-lowering`（A16）をマージして統合状態を作成
  （PLAN.md/DIARY.mdのコンフリクトのみ発生、コード側は無競合）
- `cargo test --workspace`で85テスト全通過を確認後、デバッグ計装版picorv32.vを
  実行。`&mem_state`/`\|mem_state`が`mem_state=00`で正しく`0`/`0`になるようになった
  （A16の効果を確認）が、`cpu_state`が`cpu_state_fetch`のまま停止し続けることを発見
- `mem_do_rinst=1`・`case0`条件（`mem_do_prefetch||mem_do_rinst||mem_do_rdata`）=1・
  `mem_state==0`=true・`!resetn||trap`=false（つまりelse節=通常動作パスに入っている）
  が全て揃っているにもかかわらず、`case (mem_state) 0: begin ... mem_valid<=...;
  mem_state<=1; end`が一切実行されず`mem_valid`/`mem_state`が更新されないことを
  デバッグ計装で確認
- 最小再現（`reg [1:0] st; case(st) 0:...;1:...;2:...;3:...;endcase`）で、全ての
  `st`パターンでcase項が一度もマッチせず`out`がデフォルト値のまま変わらないことを
  確認。`crates/mir/src/logicval.rs`の`case_eq`（333-348行目付近）を確認したところ、
  `if self.width() != rhs.width() { return LogicVal::ZERO; }`という早期returnが
  あり、セレクタ（2bit）とcase項（サイズ指定なし10進リテラル、既定32bit幅）の幅が
  完全一致しない限り無条件で不一致と判定していることを発見。同ファイル内の通常の
  `==`用`eq`関数は`w = self.width().max(rhs.width())`で正しく幅の違いを吸収して
  いるのと対照的で、`case_eq`だけがこの処理を欠いていた。picorv32.vの
  `case (mem_state) 0: ...; 1: ...; 2: ...; 3: ...; endcase`（`mem_state`は2bit）が
  まさにこのパターンに該当し、これがA14・A15・A16を全て適用した後でも
  picorv32スモークがタイムアウトし続ける直接原因と特定した

### What was done（Codexに委任・実装完了を確認、A17）

- 検証専用ブランチを削除し、`feat/m1-milestone`から新規ブランチ
  `fix/case-width-mismatch`を作成、原因分析結果をCodexへ委任
- `case_eq`を`eq`と同じ`w = self.width().max(rhs.width())`方式に修正
  （X/Z平面の厳密一致判定ロジック自体は変更なし）
- 回帰テスト`tests/integration/cases/case_width_mismatch/`（2bitセレクタ対
  32bit既定幅リテラルのcase文、`===`/`!==`の幅不一致比較）を追加
- 調査中に`casez`/`casex`が同じ`case_eq`を呼んでおりワイルドカード（Z/X）マッチが
  未実装の疑いがある既知課題（PLAN.md F節に既記載）を再確認したが、スコープ外として
  今回は対応しないよう明示的に指示した

Codex側の実装完了後、こちらで独立に検証:

- `git diff`で変更が依頼範囲（`case_eq`本体3行＋回帰テスト）にちょうど絞られている
  ことを確認（`eq`との差分もほぼ同一の変更で、実装として自然であることを確認）
- `cargo build --workspace`成功、`cargo fmt --check`・`cargo clippy -D warnings`とも警告なし
- `cargo test --workspace`で83テスト全通過（既存回帰なし）
- `samples/counter4`・`samples/fifo_sync`回帰なし確認
- 最小再現（`reg [1:0] st; case(st) 0:...;endcase`）を再実行し、全4パターンで
  正しくcase項がマッチすることを確認（`st=00→out=11`等）

### Result

✅ A17（`case`文比較の幅不一致バグ、IEEE 9.5節違反）を修正。`case`文で
   サイズ指定なしリテラルを使う非常に一般的な書き方が軒並み壊れていた
   重大バグを解消
✅ `cargo test --workspace`（83テスト）全通過を確認
✅ picorv32.vの`mem_state`FSMが`00`から`01`へ進行するようになったことを確認
✅ PLAN.md実装課題A節にA17を追加

### Next

- PR #30（A15）・PR #31（A16）・今回のA17ブランチを`feat/m1-milestone`へ統合し、
  picorv32.vスモークテストを再実行して`PASS`まで到達するか確認する
  （推奨着手順11番、継続）
- casez/casexのワイルドカードマッチ未実装疑惑（PLAN.md F節）は、picorv32.v自体は
  plain caseのみ使用しているため今回のブロッカーではないが、別途優先度を検討する
  価値がある既知課題として記録済み
## 2026-08-15 (4)

### Task

前回セッションに続き推奨着手順11番の継続。A17（PR #32）をローカルでA15・A16と
統合検証した結果、さらにA18という重大バグを発見・対応した。

### 原因調査（自分で実施、A15+A16+A17の統合検証）

- ローカル検証専用ブランチ`test/all-fixes-combined`（push対象外）で
  `origin/fix/net-decl-assignment-dropped`（A15）・
  `origin/fix/reduction-operator-lowering`（A16）・
  `origin/fix/case-width-mismatch`（A17）を`feat/m1-milestone`へ順次マージ
  （PLAN.md/DIARY.mdのみコンフリクト、コード側は無競合）
- `cargo test --workspace`で88テスト全通過を確認後、デバッグ計装版picorv32.vを
  実行。`mem_state`が`00`から`01`へ正しく進行するようになった（A17の効果を確認）が、
  `mem_state=01`のまま停止し続けることを発見
- state 1のコード（`if (mem_xfer) begin ... mem_state <= mem_do_rinst ||
  mem_do_rdata ? 0 : 3; end`）を精査。`mem_valid`/`mem_ready`/`mem_xfer`は
  正しく1になり転送完了を示すが、その直後`mem_valid`は正しく0へ戻る一方
  `mem_state`が`01`のまま変化しないことをデバッグ計装で確認
- 最小再現（`out = a || b ? 2'd0 : 2'd3;`を3パターンのa/bで検証）で、
  `a || b ? 0 : 3`が`a || (b ? 0 : 3)`として誤評価されていることを特定
  （観測値が`a||(b?0:3)`という式の値と完全に一致するパターンだった）。
  `crates/frontend/src/lower.rs`の`flatten_binary_chain`（A10で導入、
  `E::Binary`以外を不透明な1オペランドとして扱う設計）が、sv-parserが
  `a || b ? c : d`を`E::Binary(a, "||", E::ConditionalExpression(b,c,d))`
  という形で返す（三項演算子全体が丸ごと`||`の右オペランドとしてネストされる、
  A10の右結合バグと同系統のsv-parser側の癖）ため、三項演算子全体を「ただの
  オペランド」として扱ってしまい、IEEE Table 5-4で最低優先順位のはずの`?:`が
  `||`より高優先度であるかのような結果になっていたことを特定。
  picorv32.vの`mem_state <= mem_do_rinst || mem_do_rdata ? 0 : 3;`が
  まさにこのパターンで、A14〜A17を全て適用した状態でもFSMがstate 1から
  一切進行しない直接原因と特定した

### What was done（Codexに委任・実装完了を確認、A18）

- 検証専用ブランチを削除し、`feat/m1-milestone`から新規ブランチ
  `fix/ternary-binop-precedence`を作成、具体的な実装方針（`flatten_binary_chain`
  の戻り値を`Result<Option<(Expr,Expr)>, FrontendError>`に変更し、チェーン末尾の
  裸の`E::ConditionalExpression`検出時は条件部をチェーンへ合流・then/else部を
  戻り値で呼び出し元へ伝播する具体的なコード案）込みでCodexへ委任
- Codexが依頼通りの実装に加え、**同型のバグを抱えていた定数式版
  `flatten_constant_binary_chain`（`CE::Ternary`、localparam/genvar文脈で使用、
  A11で導入）にも同じ修正を自発的に適用**（依頼文で「必ず確認すること」と
  明示していた箇所を正しく特定・対応）
- 回帰テスト`tests/integration/cases/ternary_binop_precedence/`
  （`||`・`&&`・`+`/`==`との組み合わせ、localparam定数式での`?:`優先順位）を追加

Codex側の実装完了後、こちらで独立に検証:

- `git diff`で変更が依頼範囲（`flatten_binary_chain`・`flatten_constant_binary_chain`
  ・両者の呼び出し元＋回帰テスト）に収まっていることを確認
- `cargo build --workspace`成功、`cargo fmt --check`・`cargo clippy -D warnings`とも警告なし
- `cargo test --workspace`で83テスト全通過（既存回帰なし）
- `samples/counter4`・`samples/fifo_sync`回帰なし確認
- 最小再現（`a || b ? 0 : 3`の3パターン）を再実行し、全て正しい結果
  （`(a||b)?0:3`通り）になることを確認

### Result

✅ A18（二項演算子チェーン末尾の裸の三項演算子の優先順位バグ、IEEE Table 5-4
   違反）を修正。`cond1 || cond2 ? then : else`という非常に一般的な書き方が
   軒並み壊れていた重大バグを解消。定数式（localparam/genvar）文脈の同型バグも
   合わせて解消
✅ `cargo test --workspace`（83テスト）全通過を確認
✅ PLAN.md実装課題A節にA18を追加

### Next

- PR #30（A15）・PR #31（A16）・PR #32（A17）・今回のA18ブランチを
  `feat/m1-milestone`へ統合し、picorv32.vスモークテストを再実行して`PASS`まで
  到達するか確認する（推奨着手順11番、継続。A14〜A18で計5件の重大バグを
  発見・修正したが、picorv32.vのフル動作確認はまだ未達成）

## 2026-08-15 (5)

### Task

A15〜A18（PR #30〜#33）の統合と、picorv32.vスモークテストの最終進捗確認。
併せて新たに発見した6件目の課題（A19候補）を記録する。

### What was done（自分で実施）

- 4つのPRをマージ前にローカルの検証専用ブランチ（`test/a15-a16-combined`→
  `test/all-fixes-combined`→`test/all-fixes-v2`、いずれもpush対象外、都度削除）で
  順次統合し、`cargo test --workspace`と`picorv32.v`スモークの進捗をその都度確認
  しながら次のバグ調査を進める、という反復ワークフローを採用した
- ユーザーに一区切りとするか確認を取った上で、4つのPRを`feat/m1-milestone`へ
  実際にマージする作業を実施:
  - PR #30（A15）はコンフリクトなくそのままマージ
  - PR #31（A16）・PR #32（A17）・PR #33（A18）はいずれも`PLAN.md`・`DIARY.md`
    で同一箇所への追記によるコンフリクトが発生（コード側は全て無競合）。
    `gh pr checkout`相当のローカルブランチ（`pr31-resolve`等）を作成し、
    最新の`origin/feat/m1-milestone`とマージしてPythonスクリプトで
    「A15〜A17の既存内容を残しつつ、当該PRの新規セクションを正しい順序で
    追記する」形にコンフリクトを解消。都度`cargo build`/`cargo test`で
    回帰がないことを確認してからpush・CI通過待ち・マージを繰り返した
    （PR間の依存関係上、後続PRのマージ前に必ず先行PRのマージを待つ必要が
    あり、都度`git fetch`し直してから解消をやり直した）
- 4PR統合後、`feat/m1-milestone`をローカルにpullし、`cargo build --workspace`・
  `cargo test --workspace`（91テスト全通過）・`cargo fmt --check`・
  `cargo clippy --workspace --all-targets -- -D warnings`を実行し回帰なしを確認
- 統合状態でpicorv32.vスモークテストを再実行（デバッグ計装版picorv32.vに
  `$display`を追加して調査）。結果、**5命令すべて（addi/addi/add/lui/sw）を
  正しくフェッチ・デコードし、reg_pcが0→4→8→c→10と正しく進行することを確認**
  （前回セッション終了時点では最初の命令フェッチにすら到達できていなかった
  ため、大きな前進）
- ただし`sw x3, 0(x4)`実行時、直前の`lui x4, 0x10000`で`x4`に設定されたはずの
  `0x10000000`が反映されず、実際のストア先アドレスが`mem_addr=00000000`に
  なり、`mem_wstrb`も`sw`本来の`1111`ではなく`0100`という不正な値になって
  いることをデバッグ計装で発見（Icarus Verilogでは同一テストベンチが
  `RESULT=42`で正しくPASSするため、rverilog側の未特定バグと判断）。
  レジスタ読み出しタイミング（LUIの書き戻しとSWの読み出しのハザード）か、
  ストアアドレス計算・wstrb生成ロジックのいずれかを疑うべき、との仮説を
  ユーザーへ報告した
- ユーザーの判断で、A19候補の深追いはせず4PRのマージ完了をもって
  このセッションの区切りとした

### Result

✅ A15・A16・A17・A18（PR #30〜#33）を`feat/m1-milestone`へ統合。
   `cargo test --workspace`（91テスト）・`fmt`・`clippy`全通過
✅ picorv32.vが5命令すべてを正しくフェッチ・デコードするようになったことを
   確認（reg_pcが正しく0→4→8→c→10と進行）。A14セッション開始時点
   （最初の命令フェッチにも到達できず）から大きく前進
⚠️ picorv32.vスモークテストは依然PASS未到達。SW命令実行時にストアアドレスが
   `x4`の値（`0x10000000`）ではなく`0`になる新たな不具合（A19候補）を発見・
   記録したが、原因未特定のまま次セッションへ持ち越し
✅ ローカルの一時検証ブランチ（`test/*`・`pr3[123]-resolve`）は全て削除済み、
   リモートの作業ブランチ（`fix/net-decl-assignment-dropped`等）は
   `--delete-branch`でPRマージと同時に削除済み

### Next

- A19候補（SW命令のストアアドレスが`x4`の値を反映せず`0`になる、
  `mem_wstrb`も`1111`でなく`0100`になる）の原因調査。レジスタ書き戻し
  タイミング（LUIの結果がSWの実行前にreg_op1へ正しく読み込まれているか）と、
  `mem_la_addr`/wstrb生成ロジック（`{reg_op1[31:2], 2'b00}`等）の両方を
  疑うべき。原因調査は自分で行い、実装はCodexへ委任する運用を継続する
  （推奨着手順11番、継続）

## 2026-09-03

### Task

推奨着手順11番の継続。A14〜A18を`feat/m1-milestone`へ統合済みの状態で
`test_picorv32_smoke`（`--ignored`付き）を再実行し、依然`TIMEOUT`で失敗することを確認。
新規セッションのため、調査をフォーク（サブエージェント）に委任し、原因特定後の実装は
Codexに委任する運用を継続した。

### 原因調査（フォークに委任、実測ベースで実施）

- 1回目のフォーク応答はツール呼び出し3回・16秒のみで実質的な調査をしていなかったため
  （委任の連鎖に迷い込んだ可能性）、「他エージェントに委任せず自分で手を動かして調査する
  こと」を明示して同一フォークを再開
- 再開後のフォークが`interp.rs`への一時的なデバッグ計装（調査後revert済み）とVCD波形観測で
  `cpu_state`が起動直後の`cpu_state_fetch`から一度も遷移しないことを確認し、
  `decoder_trigger`は正しく`1`になるにもかかわらず`if (decoder_trigger) ... cpu_state <=
  cpu_state_ld_rs1;`（picorv32.v 1557行目、3段の`else if`チェーンの最終節）が一度も
  評価されないことを特定
- 最小再現（`if(a) out=1; else if(b) out=2; else if(c) out=3;`、a=0,b=0,c=1）で`out`が
  Xのまま（`else if(c)`が実行されない）ことを確認して再現成功
- 根本原因: `frontend/src/lower.rs`の`lower_conditional`が、`sv_parser::
  ConditionalStatement.nodes`のうち`nodes.4`（中間の全`else if`節を保持する`Vec`）を
  一切参照しておらず、`nodes.2`（先頭if条件）・`nodes.3`（先頭then）・`nodes.5`
  （末尾else、あれば）しか見ていなかった。結果、末尾`else`がない場合は中間`else if`が
  全て消滅し、末尾`else`がある場合は`else if`の条件を一切評価せず先頭`if`が偽なら
  即座に末尾`else`へ飛んでいた。`case`文中心の設計のため`if/else if`が2段以上かつ
  実際に中間分岐が踏まれるテストケースがこれまで存在せず見逃されていたと推定
- **A19**としてPLAN.md実装課題A節に追加（IEEE 1364-2001 9.4節違反、最重要）

### What was done（Codexに委任・実装完了を確認、A19）

- 検証専用ブランチ`fix/else-if-chain-dropped`を`feat/m1-milestone`から作成し、
  具体的な修正方針（`cs.nodes.4`の各`else if`節を末尾から`Stmt::If`として畳み込み、
  `cs.nodes.5`を初期値としてfoldし、先頭`if`はその結果を`else`節として包む、
  A10/A18の二項演算子チェーン畳み込みと同型のfold処理）込みでCodexへ委任
- Codexは提案通り`lower_conditional`に`for (_, _, else_if_cond, else_if_stmt) in
  cs.nodes.4.iter().rev()`のfoldループを追加する最小差分で実装
- 回帰テスト`tests/integration/cases/else_if_chain/`（末尾elseなしで最終else-ifが
  効くケース、末尾elseありで中間else-if/最終else-if/最終elseそれぞれが効くケースの
  計4パターン）を追加

Codex側の実装完了後、こちらで独立に検証:

- `git diff`で変更が依頼範囲（`lower_conditional`本体8行＋回帰テスト＋PLAN.md）に
  収まっていることを確認
- `cargo build --workspace`成功、`cargo fmt --check`・`cargo clippy --workspace
  --all-targets -- -D warnings`とも警告なし
- `cargo test --workspace`で91テスト全通過（新規`test_else_if_chain`/
  `compare_else_if_chain`含む、既存回帰なし）
- `samples/counter4`・`samples/fifo_sync`回帰なし確認
- `test_picorv32_smoke`（`--ignored`）を再実行 → **依然`TIMEOUT`で失敗**
  （A19単独ではpicorv32.vのブロッカー解消に至らず、別の未特定原因が残っている）

### Result

✅ A19（`if`/`else if`チェーンの中間節が無言で消える、IEEE 1364-2001 9.4節違反）を修正。
   `case`文に隠れて見逃されていた重大バグを解消
✅ `cargo test --workspace`（91テスト）全通過を確認
✅ PLAN.md実装課題A節にA19を追加
❌ picorv32.vスモークテストは依然TIMEOUT。A19は必要条件だったが十分条件ではなかった

### Next

- `fix/else-if-chain-dropped`ブランチをPRとして`feat/m1-milestone`へマージする
- picorv32.vスモークテストのタイムアウト原因調査を継続（推奨着手順11番、継続）。
  今回と同じ調子（フォークで実測ベース調査→根本原因特定→Codexへ実装委任→独立検証）を
  次のセッションでも継続する