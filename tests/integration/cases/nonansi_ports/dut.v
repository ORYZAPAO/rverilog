// 非ANSIポート宣言: 方向・幅・net型を本体側の宣言から取る
module adder(a, b, s, co);
  parameter W = 4;
  input [W-1:0] a;
  input [W-1:0] b;
  output [W-1:0] s;
  output co;
  assign {co, s} = {1'b0, a} + {1'b0, b};
endmodule

module regout(clk, d, q);
  input clk;
  input [7:0] d;
  output [7:0] q;
  reg [7:0] q;
  always @(posedge clk) q <= d;
endmodule

module regout2(clk, d, q);
  input clk;
  input [7:0] d;
  output reg [7:0] q;
  always @(posedge clk) q <= d + 8'd1;
endmodule

module wiredand(a, b, y);
  input [3:0] a, b;
  output wand [3:0] y;
  assign y = a;
  assign y = b;
endmodule

module pad(io, oe, d, q);
  inout [3:0] io;
  input oe;
  input [3:0] d;
  output [3:0] q;
  assign io = oe ? d : 4'bz;
  assign q = io;
endmodule

// 親にも同名のネットがあるが、子の内部では子のポートが使われる
module dut;
  reg clk;
  reg [7:0] d;
  reg [3:0] a, b;
  wire [3:0] y;
  wire [7:0] q;
  wire [3:0] s0, s1;
  wire c0, c1;
  wire [7:0] q1, q2;
  wire [3:0] wy;
  wire [3:0] line, pq;
  reg poe;
  reg [3:0] pd;
  reg eoe;
  reg [3:0] ed;

  adder #(.W(4)) u0 (a, b, s0, c0);               // 位置結線
  adder #(.W(4)) u1 (.b(a), .a(b), .co(c1), .s(s1)); // 名前結線（順序入替）
  regout u2 (clk, d, q1);
  regout2 u3 (.clk(clk), .d(d), .q(q2));
  wiredand u4 (a, b, wy);
  pad u5 (line, poe, pd, pq);
  assign line = eoe ? ed : 4'bz;

  initial begin
    clk = 0; d = 8'h10; a = 4'b1100; b = 4'b1010; poe = 0; eoe = 0; pd = 4'h5; ed = 4'h9;
    #1 $display("s0=%b c0=%b s1=%b c1=%b wy=%b", s0, c0, s1, c1, wy);
    a = 4'd9; b = 4'd8; #1 $display("s0=%b c0=%b s1=%b c1=%b wy=%b", s0, c0, s1, c1, wy);
    #1 clk = 1; #1 $display("q1=%h q2=%h", q1, q2);
    d = 8'h7f; clk = 0; #1 clk = 1; #1 $display("q1=%h q2=%h", q1, q2);
    $display("line=%b pq=%b", line, pq);
    poe = 1; #1 $display("line=%b pq=%b", line, pq);
    eoe = 1; #1 $display("line=%b pq=%b", line, pq);
    poe = 0; #1 $display("line=%b pq=%b", line, pq);
    $finish;
  end
endmodule
