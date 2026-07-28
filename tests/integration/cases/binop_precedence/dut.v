module dut;
  reg [31:0] a, b, c, d;

  initial begin
    // PLAN.md A10: sv-parser の Expression::Binary は演算子優先順位を無視し
    // 常に右結合の木を返す既知バグの再発防止用リグレッションテスト。
    // 「優先順位を無視した右結合」と「正しい優先順位＋左結合」で結果が
    // 食い違う組み合わせを選んでいる。

    // 同一優先順位の左結合（減算の連鎖）
    a = 10; b = 3; c = 2;
    $display("a-b-c=%0d", a - b - c);       // (a-b)-c = 5, バグ時は a-(b-c) = 9
    $display("a-b+c=%0d", a - b + c);       // (a-b)+c = 9, バグ時は a-(b+c) = 5

    // 乗算 > 加算
    a = 2; b = 3; c = 4;
    $display("a*b+c=%0d", a * b + c);       // (a*b)+c = 10, バグ時は a*(b+c) = 14
    $display("a+b*c=%0d", a + b * c);       // a+(b*c) = 14（両方一致するが回帰確認用）

    // ビット論理: & > |
    a = 6; b = 3; c = 1;
    $display("a&b|c=%0d", a & b | c);       // (a&b)|c = 3, バグ時は a&(b|c) = 2

    // 論理: && > ||
    a = 0; b = 1; c = 1;
    $display("a&&b||c=%0d", a && b || c);   // (a&&b)||c = 1, バグ時は a&&(b||c) = 0

    // 加算 > 比較
    a = 1; b = 1; c = 3;
    $display("a+b==c=%0d", a + b == c);     // (a+b)==c = 0, バグ時は a+(b==c) = 1

    // 4項混合: 乗算 > 減算/加算（左結合）
    a = 10; b = 2; c = 3; d = 1;
    $display("a-b*c+d=%0d", a - b * c + d); // (a-(b*c))+d = 5, バグ時は a-(b*(c+d)) = 2

    $finish;
  end
endmodule
