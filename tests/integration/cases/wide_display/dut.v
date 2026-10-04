module dut;
  reg [127:0] a;
  reg signed [127:0] s;
  reg [71:0] w72;
  reg [99:0] xz;
  reg [199:0] big;

  initial begin
    // 64bit超の10進リテラル
    a = 128'd1000000000000000000000000000;
    $display("dec lit h=%h", a);
    $display("dec lit d=%d", a);
    s = -128'sd1000000000000000000000000000;
    $display("neg lit h=%h", s);
    $display("neg lit d=%d", s);
    a = 128'd340282366920938463463374607431768211455;
    $display("max h=%h", a);
    $display("max d=%d", a);
    // 幅を超える値は切り詰められる
    w72 = 72'd4722366482869645213695;
    $display("w72 h=%h d=%d", w72, w72);

    // 各基数・幅修飾子
    a = 128'hdeadbeef_cafef00d_01234567_89abcdef;
    $display("h=%h", a);
    $display("0h=%0h", a);
    $display("o=%o", a);
    $display("b=%b", a);
    $display("d=%d", a);
    $display("0d=%0d", a);
    $display("50d=%50d", a);
    $display("050d=%050d", a);
    a = 128'd5;
    $display("small d=%d 0d=%0d h=%h", a, a, a);
    s = -128'sd5;
    $display("sd=%d 0d=%0d", s, s);

    // X / Z
    xz = 100'bx;
    $display("x h=%h d=%d b=%b", xz, xz, xz);
    xz = 100'bz;
    $display("z h=%h d=%d", xz, xz);
    xz = {50'h3ffffffffffff, 50'bx};
    $display("mix h=%h d=%d", xz, xz);

    big = 200'hab_0000000000005678_0000000000001234_ffffffffffffffff;
    $display("big h=%h", big);
    $display("big d=%d", big);
    $finish;
  end
endmodule
