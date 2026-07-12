# $signed / $unsigned サポート設計書

- 日付: 2026-07-12
- ステータス: 承認済み
- 動機: picorv32.v が `$signed` を25箇所使用しており、現状は frontend で
  「system function $signed in expression」の unsupported エラーになる。

## 背景と現状

- `frontend/src/lower.rs` — 式中のシステム関数は `$random` のみ対応。
  `$signed` / `$unsigned` は unsupported エラー。
- `hir/src/design.rs` — `SysFuncKind` は `Clog2` / `Random` のみ。
- `elab/src/elaborate.rs` — 式ごとの符号情報 `expr_signed: Vec<bool>` を
  ExprId 単位で保持する仕組みが既にある(`alloc_expr_signed` で明示指定可能)。
- `sim/src/interp.rs` — `apply_binop` が `expr_signed` を参照し、
  比較/除算/剰余/`>>>` で符号付き演算を選択済み。
- **未実装の本質**: 代入時の幅合わせは `resize`(ゼロ拡張)のみで符号拡張が
  存在しない(`interp.rs` `write_lvalue`)。picorv32 の典型例
  `decoded_imm <= $signed(mem_rdata_q[31:20]);`(12bit→32bit)は代入時の
  符号拡張が必要。

## スコープ

「一般整備込み」: `$signed` / `$unsigned` のパースに加え、代入時の符号拡張と、
幅が異なる signed オペランド同士の演算時の符号拡張まで整備する。

## 設計

### 1. Frontend(HIR層)

- `SysFuncKind` に `Signed` / `Unsigned` を追加。
- `lower.rs` のシステム関数呼び出し処理(SystemTfCall)に
  `"$signed"` / `"$unsigned"` の腕を追加。
- 引数は1個のみ許可。0個または2個以上は `FrontendError`。

### 2. Elaboration(MIR生成)— アプローチB: 既存機構再利用

- `HirExpr::SysFunc(Signed/Unsigned, args)` の腕を式loweringに追加。
- inner式を通常どおり lower した後、**トップの `Expr` を複製して
  `alloc_expr_signed(expr, true/false)` で再登録する**。
  MIR に新ノードは追加しない(評価器・感度解析・VCDは無変更)。

```rust
HirExpr::SysFunc(SysFuncKind::Signed, args) => {
    let inner_id = lower_expr(ctx, &args[0])?;
    let expr = ctx.exprs[inner_id.0 as usize].clone();
    Ok(ctx.alloc_expr_signed(expr, true))
}
```

- 結果幅は inner と同じ(IEEE 1364-2005 17.9.3: 引数は self-determined)。
- 定数式パス(パラメータ評価用)にも同様の腕を追加。

### 3. シミュレータ: 代入時の符号拡張

- 継続代入・blocking・nonblocking の各代入実行箇所で、RHS ExprId の
  `expr_signed` が true かつ値幅 < LHS幅の場合、`LogicVal::extend_sign` で
  拡張してから `write_lvalue` に渡す。
- `write_lvalue` 内の `resize`(ゼロ拡張)は fallback として維持。
- 関数の引数受け渡し・戻り値代入が同じパスを通ることを確認し、
  同一の規則を適用する。

### 4. シミュレータ: 演算オペランドの符号拡張

- `apply_binop` で `both_signed` かつ両辺の幅が異なる場合、max幅へ
  `extend_sign` してから演算する(Add/Sub/Mul/Div/Mod/ビット演算/比較)。
- 既存の `lt_signed` 等が64bit超の幅でも正しいか確認し、
  `extend_sign` ベースに統一する。

### 5. エラー処理

- `$signed()` / `$unsigned()` の引数個数不正 → frontend でエラー。
- それ以外の評価時エラーは既存の LogicVal 演算規則(X伝播)に従う。

## テスト計画

- 統合テスト(`crates/cli/tests/integration.rs`):
  - `$signed({a, b[6:2]})` の nonblocking 代入(狭→広の符号拡張、picorv32型)
  - `$signed(x) < $signed(y)` の符号付き比較
  - `$signed(a) / $signed(b)` の符号付き除算
  - `$unsigned` によるsigned解除
  - 幅が異なる signed オペランド同士の加算
- iverilog比較テスト(`crates/cli/tests/iverilog_compare.rs`)に同ケースを
  追加し、iverilog と出力一致を検証。
- 最終確認: `picorv32.v` が parse / elaboration を通過すること。

## スコープ外(既知の割り切り)

- IEEE完全準拠の文脈幅伝播(`(a+b)>>1` の中間キャリー等)は既存の制限のまま。
- signedリテラル(`8'sh80` 等)の扱いは既存実装(`SignedConst`)のまま。
