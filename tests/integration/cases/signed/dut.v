module dut;
  reg signed [3:0] a;
  reg signed [3:0] b;
  reg [3:0] ua;
  reg [3:0] ub;
  integer i;

  initial begin
    a = -1;   // 4'b1111
    b = 1;    // 4'b0001
    ua = -1;  // 4'b1111 (unsigned reinterpretation)
    ub = 1;

    if (a < b) $display("signed: a<b true"); else $display("signed: a<b false");
    if (ua < ub) $display("unsigned: ua<ub true"); else $display("unsigned: ua<ub false");

    $display("a=%d ua=%d", a, ua);

    i = -7;
    $display("div=%d mod=%d", i / 2, i % 2);

    $display("a>>>1=%b ua>>>1=%b", a >>> 1, ua >>> 1);

    $finish;
  end
endmodule
