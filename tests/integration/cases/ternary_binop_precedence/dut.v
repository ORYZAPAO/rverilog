module dut;
  localparam CONST_OR_TRUE = 1 || 0 ? 2'd0 : 2'd3;
  localparam CONST_OR_FALSE = 0 || 0 ? 2'd0 : 2'd3;
  reg a, b;
  reg [1:0] out;

  // `?:` は全ての二項演算子より低優先順位であることの回帰テスト。
  initial begin
    $display("const_or_true=%0d", CONST_OR_TRUE);
    $display("const_or_false=%0d", CONST_OR_FALSE);

    a = 1; b = 0;
    out = a || b ? 2'd0 : 2'd3;
    $display("or_10=%b", out);

    a = 0; b = 0;
    out = a || b ? 2'd0 : 2'd3;
    $display("or_00=%b", out);

    a = 0; b = 1;
    out = a || b ? 2'd0 : 2'd3;
    $display("or_01=%b", out);

    a = 1; b = 0;
    out = a && b ? 2'd0 : 2'd3;
    $display("and_10=%b", out);

    a = 1; b = 1;
    out = a && b ? 2'd0 : 2'd3;
    $display("and_11=%b", out);

    out = 1 + 1 == 2 ? 2'd0 : 2'd3;
    $display("add_eq=%b", out);

    out = 1 + 1 == 3 ? 2'd0 : 2'd3;
    $display("add_ne=%b", out);

    $finish;
  end
endmodule
