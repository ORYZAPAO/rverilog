module dut;
  reg d, c, n, p;
  wire o_nmos, o_pmos, o_rnmos, o_rpmos, o_cmos, o_rcmos;
  nmos  g1(o_nmos, d, c);
  pmos  g2(o_pmos, d, c);
  rnmos g3(o_rnmos, d, c);
  rpmos g4(o_rpmos, d, c);
  cmos  g5(o_cmos, d, n, p);
  rcmos g6(o_rcmos, d, n, p);

  // 複数スイッチでバス共有
  reg e1, e2, d1, d2;
  wire bus;
  nmos (bus, d1, e1);
  pmos (bus, d2, e2);

  integer i, j, k;
  reg [3:0] v;

  initial begin
    for (i = 0; i < 4; i = i + 1) begin
      for (j = 0; j < 4; j = j + 1) begin
        case (i) 0: d = 1'b0; 1: d = 1'b1; 2: d = 1'bx; 3: d = 1'bz; endcase
        case (j) 0: c = 1'b0; 1: c = 1'b1; 2: c = 1'bx; 3: c = 1'bz; endcase
        #1 $display("d=%b c=%b nmos=%b pmos=%b rnmos=%b rpmos=%b", d, c, o_nmos, o_pmos, o_rnmos, o_rpmos);
      end
    end
    // cmos: 入力 0/1/z、制御は 0/1 の4通り
    for (i = 0; i < 3; i = i + 1) begin
      for (j = 0; j < 4; j = j + 1) begin
        case (i) 0: d = 1'b0; 1: d = 1'b1; 2: d = 1'bz; endcase
        {n, p} = j[1:0];
        #1 $display("d=%b n=%b p=%b cmos=%b rcmos=%b", d, n, p, o_cmos, o_rcmos);
      end
    end
    for (i = 0; i < 16; i = i + 1) begin
      {e1, e2, d1, d2} = i[3:0];
      #1 $display("e1=%b e2=%b d1=%b d2=%b bus=%b", e1, e2, d1, d2, bus);
    end
    $finish;
  end
endmodule
