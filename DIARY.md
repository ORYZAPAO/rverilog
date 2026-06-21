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
