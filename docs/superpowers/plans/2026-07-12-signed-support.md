# $signed / $unsigned サポート実装プラン

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Verilogの `$signed` / `$unsigned` システム関数をサポートし、代入時・演算時の符号拡張を整備して picorv32.v の $signed 使用箇所(25箇所)を通す。

**Architecture:** frontend で `SysFuncKind::Signed/Unsigned` として HIR 化し、elab で inner 式のトップ `Expr` を `alloc_expr_signed` で複製登録する(MIRノード追加なし=アプローチB)。シミュレータは (1) 代入時に RHS が signed なら `extend_sign` で LHS 幅へ符号拡張、(2) `apply_binop` で幅の異なる signed オペランド同士を max 幅へ符号拡張、の2点を追加する。

**Tech Stack:** Rust workspace(crates/frontend, hir, elab, sim, cli)。検証は integration テスト + iverilog 出力比較(iverilog は `/usr/local/bin/iverilog` に導入済み)。

**Spec:** `docs/superpowers/specs/2026-07-12-signed-support-design.md`

## Global Constraints

- 既存テストを壊さないこと(`cargo test --workspace` 全パス)
- MIR に新ノードを追加しない(アプローチB、承認済み)
- コミットメッセージ末尾: `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`
- コード内コメントは既存スタイル(日本語、制約の説明のみ)に合わせる

---

### Task 0: ブランチ準備

**Files:** なし(git操作のみ)

- [ ] **Step 1: 最新の feat/m1-milestone から作業ブランチを作成**

```bash
cd /Users/zosan/github/rverilog
git checkout feat/m1-milestone && git pull
git checkout -b feat/signed-impl
```

Expected: `Switched to a new branch 'feat/signed-impl'`

---

### Task 1: 統合テストケース作成(失敗するテスト)

**Files:**
- Create: `tests/integration/cases/signed_cast/dut.v`
- Create: `tests/integration/cases/signed_cast/expected.stdout`
- Modify: `crates/cli/tests/integration.rs`(`test_signed` の直後にテスト追加)
- Modify: `crates/cli/tests/iverilog_compare.rs`(`compare_signed` の直後にテスト追加)

**Interfaces:**
- Produces: テストケース名 `signed_cast`。Task 2〜4 はこのテストの段階的なパスで検証する。

- [ ] **Step 1: DUT を作成**

`tests/integration/cases/signed_cast/dut.v`:

```verilog
module dut;
  reg [11:0] imm;
  reg [31:0] r32;
  reg [31:0] nbr;
  reg [3:0] a;
  reg signed [3:0] sa;
  reg signed [7:0] sb;
  reg clk;
  wire [31:0] w32;

  // 継続代入での $signed(部分選択) → 32bit 符号拡張
  assign w32 = $signed(imm[11:8]);

  // nonblocking 代入での $signed(部分選択) → 32bit 符号拡張(picorv32型)
  always @(posedge clk) nbr <= $signed(imm[11:4]);

  initial begin
    clk = 0;
    imm = 12'h800;

    // blocking 代入: $signed による符号拡張 vs 既定のゼロ拡張
    r32 = $signed(imm);
    $display("sext=%h", r32);
    r32 = imm;
    $display("zext=%h", r32);

    // $signed(連結) → 7bit を 32bit へ符号拡張
    r32 = $signed({imm[11], imm[10:5]});
    $display("cat=%h", r32);

    // $signed による符号付き比較
    a = 4'b1111;
    if ($signed(a) < 0) $display("cmp: neg"); else $display("cmp: pos");
    if (a < 0) $display("ucmp: neg"); else $display("ucmp: pos");

    // $unsigned による符号解除 / signed reg の代入時符号拡張
    sa = -1;
    r32 = $unsigned(sa);
    $display("usx=%h", r32);
    r32 = sa;
    $display("ssx=%h", r32);

    // $signed による符号付き除算
    $display("div=%d", $signed(a) / 2);

    // 幅が異なる signed オペランド同士の加算(4bit + 8bit)
    sa = -2;
    sb = 3;
    $display("add=%d", sa + sb);

    #1 clk = 1;
    #1 $display("nba=%h", nbr);
    $display("cont=%h", w32);
    $finish;
  end
endmodule
```

- [ ] **Step 2: iverilog で期待値の根拠を確認し expected.stdout を作成**

```bash
cd /Users/zosan/github/rverilog
iverilog -g2001 -o /tmp/signed_cast.vvp tests/integration/cases/signed_cast/dut.v
vvp /tmp/signed_cast.vvp
```

Expected(iverilog出力、最終行の $finish 通知を除く):

```
sext=fffff800
zext=00000800
cat=ffffffc0
cmp: neg
ucmp: pos
usx=0000000f
ssx=ffffffff
div=          0
add=   1
nba=ffffff80
cont=fffffff8
```

`expected.stdout` には上記に rverilog 形式の finish 行を加えて保存する
(既存ケース `tests/integration/cases/signed/expected.stdout` と同形式。
$finish は time 2 で呼ばれる):

```
sext=fffff800
zext=00000800
cat=ffffffc0
cmp: neg
ucmp: pos
usx=0000000f
ssx=ffffffff
div=          0
add=   1
nba=ffffff80
cont=fffffff8
$finish at time 2
```

(末尾に改行1つ。iverilog の実出力と `div=` / `add=` の空白パディングが
異なる場合は iverilog の実出力を正とする)

- [ ] **Step 3: integration.rs にテスト追加**

`crates/cli/tests/integration.rs` の `test_signed` の直後に:

```rust
#[test]
fn test_signed_cast() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/signed_cast/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("signed_cast");
    assert_eq!(got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}", expected, got);
}
```

`crates/cli/tests/iverilog_compare.rs` の `compare_signed` の直後に:

```rust
#[test]
fn compare_signed_cast() {
    let root = workspace_root();
    compare_case("dut", &[root.join("tests/integration/cases/signed_cast/dut.v")]);
}
```

- [ ] **Step 4: テストが失敗することを確認**

```bash
cargo test -p rverilog-cli --test integration test_signed_cast 2>&1 | tail -20
```

Expected: FAIL(`parse failed` / `system function $signed in expression` を含む panic)

- [ ] **Step 5: コミット**

```bash
git add tests/integration/cases/signed_cast/ crates/cli/tests/integration.rs crates/cli/tests/iverilog_compare.rs
git commit -m "test: \$signed/\$unsigned の統合テストケースを追加(現状は失敗)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 2: HIR/Frontend/Elab — $signed/$unsigned をパイプライン貫通

**Files:**
- Modify: `crates/hir/src/design.rs:12`(`SysFuncKind` に variant 追加)
- Modify: `crates/frontend/src/lower.rs:1640-1648`(SystemTfCall の腕追加)
- Modify: `crates/elab/src/elaborate.rs:795-804` 付近(式lowering の腕追加)
- Modify: `crates/elab/src/elaborate.rs:1096-1102` 付近(定数式パスの腕追加)

**Interfaces:**
- Consumes: 既存の `alloc_expr_signed(expr, signed) -> ExprId`(elaborate.rs:102)
- Produces: `SysFuncKind::Signed` / `SysFuncKind::Unsigned`(HIR)。
  elab 後は inner と同じ MIR `Expr` が `expr_signed=true/false` で複製登録される。

- [ ] **Step 1: HIR に variant 追加**

`crates/hir/src/design.rs` の `SysFuncKind`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SysFuncKind {
    Clog2,
    Random,
    Signed,
    Unsigned,
}
```

- [ ] **Step 2: frontend で $signed/$unsigned をパース**

`crates/frontend/src/lower.rs` の `SubroutineCall::SystemTfCall` の match
(`"$random"` の腕の直後)に追加:

```rust
"$signed" | "$unsigned" => {
    let args = collect_syscall_args(tree, sys);
    if args.len() != 1 {
        return Err(FrontendError::ParseError(
            format!("{} requires exactly 1 argument", name_text)));
    }
    let kind = if name_text == "$signed" { SysFuncKind::Signed }
               else { SysFuncKind::Unsigned };
    Ok(Expr::SysFunc(kind, args))
}
```

- [ ] **Step 3: elab の式loweringに腕を追加**

`crates/elab/src/elaborate.rs` の `lower_expr` 内、
`HirExpr::SysFunc(SysFuncKind::Random, args)` の腕の直後に追加。
match の他の腕は `mir` 変数に値を作って末尾で `ctx.alloc_expr(mir)` するが、
この腕は signedness を明示指定するため早期 return する
(`HirExpr::SignedConst` の腕と同じパターン):

```rust
// $signed/$unsigned: inner のトップ Expr を複製し signedness を明示して再登録する
// （アプローチB: MIR に cast ノードは追加しない。幅は inner と同じ = self-determined）
HirExpr::SysFunc(kind @ (SysFuncKind::Signed | SysFuncKind::Unsigned), args) => {
    let arg = args.first().ok_or_else(|| ElabError::UnsupportedConstruct(
        "$signed/$unsigned requires exactly 1 argument".into()))?;
    let inner_id = lower_expr(ctx, scope, arg)?;
    let expr = ctx.exprs[inner_id.0 as usize].clone();
    return Ok(ctx.alloc_expr_signed(expr, *kind == SysFuncKind::Signed));
}
```

(注: `ctx` の実際の変数名・`lower_expr` のシグネチャは既存コードに合わせる。
`kind @` パターンが借用の都合で使えない場合は Signed / Unsigned の2腕に分ける)

- [ ] **Step 4: elab の定数式パスに腕を追加**

`crates/elab/src/elaborate.rs` の `eval_const_hir_with` 内、
`SysFuncKind::Clog2` の腕の直後・`_ =>` フォールバックの前に追加:

```rust
// 定数文脈の $signed/$unsigned は u64 値としては素通し（幅情報を持たないため）
HirExpr::SysFunc(SysFuncKind::Signed | SysFuncKind::Unsigned, args) => {
    let a = args.first().ok_or_else(|| ElabError::UnsupportedConstruct(
        "$signed/$unsigned requires exactly 1 argument".into()))?;
    eval_const_hir_with(ctx, scope, a, extra)
}
```

- [ ] **Step 5: ビルドとテスト実行(部分パスの確認)**

```bash
cargo build --workspace 2>&1 | tail -5
cargo test -p rverilog-cli --test integration test_signed_cast 2>&1 | tail -30
```

Expected: ビルド成功。テストはまだ FAIL だが、出力差分が変わる:
- `cmp: neg` / `ucmp: pos` / `usx=0000000f` / `div=          0` は一致(比較・除算は既存の expr_signed 機構で動く)
- `sext=` / `cat=` / `ssx=` / `nba=` / `cont=` はゼロ拡張のまま不一致(Task 3 で修正)
- `add=` は不一致(Task 4 で修正)

- [ ] **Step 6: 既存テストが壊れていないことを確認**

```bash
cargo test --workspace 2>&1 | grep -E 'test result|FAILED' | head -20
```

Expected: `test_signed_cast` / `compare_signed_cast` 以外は全パス

- [ ] **Step 7: コミット**

```bash
git add crates/hir/src/design.rs crates/frontend/src/lower.rs crates/elab/src/elaborate.rs
git commit -m "feat: \$signed/\$unsigned を frontend/HIR/elab でサポート

inner 式のトップ Expr を alloc_expr_signed で複製登録し、
MIR ノードを追加せずに signedness を上書きする（アプローチB）。

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: シミュレータ — 代入時の符号拡張

**Files:**
- Modify: `crates/sim/src/interp.rs:38,74`(`nba_queue` の型に signed フラグ追加)
- Modify: `crates/sim/src/interp.rs:653-716`(`write_lvalue` に `rhs_signed` 引数追加)
- Modify: `crates/sim/src/interp.rs:137-141,297-308,595-599,758-771`(呼び出し元)

**Interfaces:**
- Consumes: `self.design.expr_signed[expr_id.0 as usize]`(ExprId 単位の signedness)、
  `LogicVal::extend_sign(new_width)`(logicval.rs:430)
- Produces: `fn write_lvalue(&mut self, lval: &LValue, val: LogicVal, rhs_signed: bool)`

- [ ] **Step 1: write_lvalue のシグネチャ変更と符号拡張**

`LValue::Net` の腕(interp.rs:655-661)を変更:

```rust
fn write_lvalue(&mut self, lval: &LValue, val: LogicVal, rhs_signed: bool) {
    match lval {
        LValue::Net(id) => {
            let net_id = *id;
            let w = self.design.get_net(net_id).width;
            // RHS が signed 文脈なら LHS 幅へ符号拡張、それ以外はゼロ拡張（IEEE 1364-2001 4.5）
            let val = if val.width() == w { val }
                else if rhs_signed && val.width() < w { val.extend_sign(w) }
                else { val.resize(w) };
            self.net_values.insert(net_id, val.clone());
            self.vcd_record_net_change(net_id, &val);
        }
```

`LValue::PartSelect` の腕(picorv32 の `mem_rdata_q[31:12] <= $signed(...)` 対応。
`let va = val.pad_to_width(sel_w);` の直前に挿入):

```rust
                let sel_w = hi - lo + 1;
                let val = if rhs_signed && val.width() < sel_w {
                    val.extend_sign(sel_w)
                } else { val };
```

`LValue::MemWrite` の腕(要素幅への符号拡張):

```rust
            LValue::MemWrite(mem_id, idx_id) => {
                let idx = self.eval_expr(*idx_id).pad_to_width(32) as u32;
                let ew = self.design.get_mem(*mem_id).elem_width;
                let val = if rhs_signed && val.width() < ew { val.extend_sign(ew) } else { val };
                self.mem_values.insert((mem_id.0, idx), val);
            }
```

`LValue::BitSelect` / `LValue::DynBitSelect` は1bit書き込みのため変更不要
(`rhs_signed` は未使用のまま)。

- [ ] **Step 2: nba_queue に signed フラグを追加**

```rust
// line 38
nba_queue: Vec<(LValue, LogicVal, bool)>,
```

NBA コミット(line 137-141):

```rust
if !self.nba_queue.is_empty() {
    let nba: Vec<_> = std::mem::take(&mut self.nba_queue);
    for (lval, val, signed) in nba {
        let old = self.get_lval_val(&lval);   // 既存コードの形に合わせる
        self.write_lvalue(&lval, val.clone(), signed);
        ...
```

- [ ] **Step 3: 呼び出し元に expr_signed を渡す**

`Stmt::BlockingAssign`(line 297-303)と `Stmt::NbaAssign`(line 305-308):

```rust
Stmt::BlockingAssign(lval, expr_id) => {
    let val = self.eval_expr(expr_id);
    let signed = self.design.expr_signed[expr_id.0 as usize];
    let old = self.get_lval_val(&lval);
    self.write_lvalue(&lval, val.clone(), signed);
    self.trigger_sensitivity(&lval, old.as_ref(), &val);
    StepResult::Continue
}

Stmt::NbaAssign(lval, expr_id) => {
    let val = self.eval_expr(expr_id);
    let signed = self.design.expr_signed[expr_id.0 as usize];
    self.nba_queue.push((lval, val, signed));
    StepResult::Continue
}
```

`exec_sync_stmt` 内の同パターン(line 595-599)も同様に変更。

`eval_conts`(line 758-771):

```rust
let val = self.eval_expr(cont.expr);
let signed = self.design.expr_signed[cont.expr.0 as usize];
...
self.write_lvalue(&lval, val.clone(), signed);
```

上記以外の `write_lvalue` 呼び出しがコンパイルエラーで見つかった場合、
RHS の ExprId が特定できる箇所は `expr_signed` を渡し、
特定できない箇所(初期化等)は `false` を渡す。

- [ ] **Step 4: テスト実行**

```bash
cargo test -p rverilog-cli --test integration test_signed_cast 2>&1 | tail -30
```

Expected: `add=` の行のみ不一致(Task 4 で修正)。
`sext=fffff800` / `cat=ffffffc0` / `ssx=ffffffff` / `nba=ffffff80` / `cont=fffffff8` が一致。

- [ ] **Step 5: 既存テスト確認**

```bash
cargo test --workspace 2>&1 | grep -E 'test result|FAILED' | head -20
```

Expected: `test_signed_cast` / `compare_signed_cast` 以外は全パス

- [ ] **Step 6: コミット**

```bash
git add crates/sim/src/interp.rs
git commit -m "feat: 代入時に RHS が signed の場合は LHS 幅へ符号拡張する

blocking/nonblocking/継続代入・part-select 代入・メモリ書き込みが対象。
picorv32 の 'decoded_imm <= \$signed(...)' パターンに必要。

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 4: シミュレータ — 演算オペランドの符号拡張

**Files:**
- Modify: `crates/sim/src/interp.rs`(`apply_binop`、line 1038 付近)

**Interfaces:**
- Consumes: `LogicVal::extend_sign`、既存の `apply_binop(op, l, r, l_signed, r_signed)`
- Produces: 幅が異なる signed オペランド同士の算術/ビット/比較演算が
  max 幅への符号拡張後に行われる

- [ ] **Step 1: apply_binop 冒頭で符号拡張**

```rust
fn apply_binop(op: BinOp, l: &LogicVal, r: &LogicVal, l_signed: bool, r_signed: bool) -> LogicVal {
    let both_signed = l_signed && r_signed;
    // 幅が異なる signed オペランド同士は max 幅へ符号拡張してから演算する
    // （シフトは右辺が self-determined、論理演算は真偽値のみを見るため対象外）
    let needs_ext = both_signed && l.width() != r.width()
        && !matches!(op, BinOp::Shl | BinOp::Shr | BinOp::Ashl | BinOp::Ashr
                        | BinOp::LogAnd | BinOp::LogOr);
    let (le, re);
    let (l, r) = if needs_ext {
        let w = l.width().max(r.width());
        le = l.extend_sign(w);
        re = r.extend_sign(w);
        (&le, &re)
    } else {
        (l, r)
    };
    let _ = both_signed; // 以降の match は既存のまま
    match op {
        ...(既存の match をそのまま)...
    }
}
```

(比較・除算・剰余は `cmp_signed` / `as_i64` が内部で符号拡張済みのため
二重拡張になるが、拡張済みの値の再拡張は no-op であり無害)

- [ ] **Step 2: テスト実行(全行一致の確認)**

```bash
cargo test -p rverilog-cli --test integration test_signed_cast 2>&1 | tail -10
cargo test -p rverilog-cli --test iverilog_compare compare_signed_cast 2>&1 | tail -10
```

Expected: 両方 PASS(iverilog 比較で `div=`/`add=` のパディング含め完全一致)

- [ ] **Step 3: 既存テスト確認**

```bash
cargo test --workspace 2>&1 | grep -E 'test result|FAILED' | head -20
```

Expected: 全パス

- [ ] **Step 4: コミット**

```bash
git add crates/sim/src/interp.rs
git commit -m "feat: 幅が異なる signed オペランド同士を max 幅へ符号拡張してから演算する

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 5: picorv32.v スモークチェックと仕上げ

**Files:**
- Modify: `PLAN.md` / `DIARY.md`(進捗の追記、リポジトリ既存の記法に合わせる)

- [ ] **Step 1: picorv32.v の parse/elab 通過確認**

```bash
cd /Users/zosan/github/rverilog
cargo run -p rverilog-cli --release -- --top picorv32 picorv32.v 2>&1 | head -20
```

Expected: `$signed` 起因のエラー(`system function $signed in expression`)が
出ないこと。$signed 以外の未対応構文でエラーになる場合は、そのエラー内容を
記録して報告する(本プランのスコープ外)。
(注: CLI の引数形式が異なる場合は `cargo run -p rverilog-cli -- --help` で確認)

- [ ] **Step 2: 全テストの最終確認**

```bash
cargo test --workspace 2>&1 | tail -15
```

Expected: 全パス

- [ ] **Step 3: PLAN.md / DIARY.md に進捗を追記してコミット**

PLAN.md の該当項目($signed サポート)を完了に更新し、DIARY.md に
2026-07-12 のエントリとして実装内容(アプローチB採用、代入時符号拡張、
演算時符号拡張、picorv32 スモーク結果)を追記する。

```bash
git add PLAN.md DIARY.md
git commit -m "docs: \$signed/\$unsigned サポートの実装完了を PLAN.md/DIARY.md に反映

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

- [ ] **Step 4: プッシュして PR 作成**

```bash
git push -u origin feat/signed-impl
gh pr create --base feat/m1-milestone --head feat/signed-impl \
  --title "feat: \$signed/\$unsigned システム関数のサポート" \
  --body "設計書 docs/superpowers/specs/2026-07-12-signed-support-design.md の実装。

- frontend/HIR/elab: \$signed/\$unsigned のパース(アプローチB: MIRノード追加なし)
- sim: 代入時の符号拡張(blocking/NBA/継続代入/part-select/メモリ)
- sim: 幅が異なる signed オペランド同士の演算時符号拡張
- テスト: signed_cast 統合テスト + iverilog 出力比較

🤖 Generated with [Claude Code](https://claude.com/claude-code)"
```
