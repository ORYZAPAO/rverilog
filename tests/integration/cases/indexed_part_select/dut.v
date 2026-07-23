module dut;
  reg [31:0] src;
  reg [7:0] got;
  reg [31:0] q;
  integer i;

  initial begin
    // RHS indexed part-select: `+:` (実行時変数をbaseに使用)
    src = 32'h12345678;
    for (i = 0; i < 4; i = i + 1) begin
      got = src[i*8 +: 8];
      $display("plus[%0d]=%h", i, got);
    end

    // RHS indexed part-select: `-:`
    for (i = 0; i < 4; i = i + 1) begin
      got = src[7+i*8 -: 8];
      $display("minus[%0d]=%h", i, got);
    end

    // LHS indexed part-select: `+:`, blocking（動的baseで各バイトへ書き込み）
    q = 32'h00000000;
    for (i = 0; i < 4; i = i + 1) begin
      q[i*8 +: 8] = i + 1;
    end
    $display("q1=%h", q);

    // LHS indexed part-select: `-:`, nonblocking（baseはスケジュール時の値を使用、
    // ループ内で複数回nonblocking代入すると評価タイミングの既知の制約に触れるため
    // 単発の代入で確認する）
    q = 32'h00000000;
    i = 2;
    q[7+i*8 -: 8] <= 8'hbb;
    #1 $display("q2=%h", q);

    $finish;
  end
endmodule
