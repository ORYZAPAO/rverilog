module dut;
  reg e1, e2, e3;
  reg [3:0] d1, d2, d3;

  // 強度の異なる2ドライバ（強い方が勝つ）
  wire [3:0] sw;
  assign (strong1, strong0) sw = e1 ? d1 : 4'bz;
  assign (weak1, weak0)     sw = e2 ? d2 : 4'bz;

  // pull対weak/strong
  wire [3:0] pw;
  assign (weak1, weak0) pw = e1 ? d1 : 4'bz;
  pullup (pw[0]);
  pulldown (pw[1]);
  pullup (pw[2]);
  pulldown (pw[3]);

  // 0側/1側で強度が異なる駆動: (strong1, weak0)
  wire [3:0] sp;
  assign (strong1, weak0) sp = e1 ? d1 : 4'bz;
  assign (weak1, strong0) sp = e2 ? d2 : 4'bz;

  // supply > strong > pull > weak、同強度の競合
  wire [3:0] lv;
  assign (supply1, supply0) lv = e1 ? d1 : 4'bz;
  assign (strong1, strong0) lv = e2 ? d2 : 4'bz;
  assign (pull1, pull0)     lv = e3 ? d3 : 4'bz;

  wire [3:0] eq;
  assign (pull1, pull0) eq = e1 ? d1 : 4'bz;
  assign (pull1, pull0) eq = e2 ? d2 : 4'bz;

  // highz側: (highz0, strong1) は1だけ駆動、0はZ
  wire [3:0] hz;
  assign (highz0, strong1) hz = e1 ? d1 : 4'bz;
  assign (weak1, weak0)    hz = e2 ? d2 : 4'bz;

  // wire宣言時の代入と強度
  wire (weak1, weak0) wd = d3[0];
  assign (strong1, strong0) wd = e1 ? d1[0] : 1'bz;

  // ゲートの強度
  wire g1, g2;
  and (strong1, strong0) ga(g1, d1[0], d2[0]);
  and (weak1, weak0) gb(g1, d1[1], d2[1]);
  bufif1 (weak1, weak0) (g2, d1[2], e1);
  bufif1 (strong1, strong0) (g2, d2[2], e2);

  // pullup/pulldownの強度指定
  wire pp, pd, pq;
  pullup (weak1) up1(pp);
  assign (weak1, weak0) pp = e1 ? d1[0] : 1'bz;
  pulldown (strong0) dn1(pd);
  assign (pull1, pull0) pd = e2 ? d2[0] : 1'bz;
  pullup (supply1) up2(pq);
  assign (strong1, strong0) pq = e3 ? d3[0] : 1'bz;

  // 64bit超
  wire [99:0] wide;
  reg [99:0] w1, w2;
  assign (strong1, strong0) wide = e1 ? w1 : 100'bz;
  assign (weak1, weak0)     wide = e2 ? w2 : 100'bz;

  task show;
    begin
      $display("%0t e=%b%b%b d=%b %b %b sw=%b pw=%b sp=%b lv=%b eq=%b hz=%b wd=%b g1=%b g2=%b wide=%h pp=%b pd=%b pq=%b",
               $time, e1, e2, e3, d1, d2, d3, sw, pw, sp, lv, eq, hz, wd, g1, g2, wide, pp, pd, pq);
    end
  endtask

  integer i;
  initial begin
    d1 = 4'b1100; d2 = 4'b1010; d3 = 4'b0110;
    w1 = 100'hf0f0f0f0f0f0f0f0f0f0f0f0f; w2 = 100'h0ff00ff00ff00ff00ff00ff00;
    for (i = 0; i < 8; i = i + 1) begin
      {e1, e2, e3} = i[2:0];
      #1 show;
    end
    d1 = 4'bx1z0; d2 = 4'b1xz0; d3 = 4'bzzx1;
    for (i = 0; i < 8; i = i + 1) begin
      {e1, e2, e3} = i[2:0];
      #1 show;
    end
    $finish;
  end
endmodule
