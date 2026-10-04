// ANSIポートヘッダのnet型（output wand）: 子モジュール内の2ドライバをwandで解決
module wandmod(input [3:0] a, input [3:0] b, input ea, input eb, output wand [3:0] y);
  assign y = ea ? a : 4'bz;
  assign y = eb ? b : 4'bz;
endmodule

module wormod(input [3:0] a, input [3:0] b, output wor [3:0] y);
  assign y = a;
  assign y = b;
endmodule

module dut;
  reg [3:0] a, b;
  reg ea, eb;
  wire [3:0] yw, yo;
  wandmod u1(.a(a), .b(b), .ea(ea), .eb(eb), .y(yw));
  wormod u2(.a(a), .b(b), .y(yo));

  // pullup/pulldown のビット選択・部分選択
  wire [7:0] bus;
  reg [7:0] bd;
  reg [1:0] be;
  assign bus[3:0] = be[0] ? bd[3:0] : 4'bz;
  assign bus[7:4] = be[1] ? bd[7:4] : 4'bz;
  pullup (bus[0]);
  pulldown (bus[3]);
  pulldown (bus[2]);
  pullup (bus[7]);

  // 連結lvalueと通常ドライバの混在
  wire [3:0] hi, lo;
  reg [7:0] cv;
  reg [3:0] ov;
  reg ce, oe;
  assign {hi, lo} = ce ? cv : 8'bz;
  assign hi = oe ? ov : 4'bz;
  wire [11:0] big;
  assign {big[11:8], big[3:0]} = ce ? cv : 8'bz;
  assign big[7:4] = oe ? ov : 4'bz;

  task show;
    begin
      $display("%0t yw=%b yo=%b bus=%b hi=%b lo=%b big=%b", $time, yw, yo, bus, hi, lo, big);
    end
  endtask

  initial begin
    a = 4'b1100; b = 4'b1010; ea = 0; eb = 0;
    be = 0; bd = 8'hA5; ce = 0; oe = 0; cv = 8'hC3; ov = 4'h6;
    #1 show;
    ea = 1; be = 2'b01; ce = 1; #1 show;
    eb = 1; be = 2'b10; oe = 1; #1 show;
    ea = 0; be = 2'b11; ce = 0; #1 show;
    a = 4'b0110; b = 4'bx1z0; be = 0; oe = 0; ce = 1; cv = 8'b1x0z_01xz; #1 show;
    $finish;
  end
endmodule
