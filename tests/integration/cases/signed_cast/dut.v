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
