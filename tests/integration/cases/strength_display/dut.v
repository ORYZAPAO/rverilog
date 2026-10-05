module dut;
  reg r, rx, rz;
  reg [3:0] rv;
  wire a, b, c, d, e, f, g, h;
  wire [1:0] v;
  wire [3:0] pv;
  assign a = 1'b1;
  assign (weak1, weak0) b = 1'b0;
  assign (supply1, supply0) c = 1'b1;
  assign (pull1, strong0) d = 1'bx;
  assign e = 1'bz;
  assign (strong1, highz0) f = 1'b0;
  assign (pull1, pull0) g = 1'b0;
  assign v = 2'b10;
  assign pv[3:2] = 2'b01;
  and (strong1, weak0) ga(h, a, c);

  // 競合・pull・net型
  wire m1, m2, m3, m4;
  assign m1 = 1'b1;
  assign m2 = 1'b0;
  assign m3 = 1'b1;
  assign (weak1, weak0) m3 = 1'b0;
  assign (weak1, weak0) m4 = 1'b0;
  wire p1, p0;
  pullup (p1);
  pulldown (p0);
  tri0 t0;
  tri1 t1;
  supply0 s0;
  supply1 s1;
  wand wa;
  assign wa = 1'b0;
  assign (weak1, weak0) wa = 1'b1;
  wire cf;
  assign cf = m1;
  assign cf = m2;
  wire pw;
  pullup (pw);
  assign (weak1, weak0) pw = 1'b0;

  initial begin
    r = 1'b1; rx = 1'bx; rz = 1'bz; rv = 4'b10x1;
    #1;
    $display("%v %v %v %v %v %v %v %v", a, b, c, d, e, f, g, h);
    $display("%v %v %v", v, pv, rv);
    $display("%v %v %v", r, rx, rz);
    $display("%v %v %v %v %v %v %v", m3, m4, p1, p0, t0, t1, wa);
    $display("%v %v %v %v", s0, s1, cf, pw);
    $finish;
  end
endmodule
