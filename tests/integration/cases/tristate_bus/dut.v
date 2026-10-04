module drv(input en, input [7:0] d, output [7:0] y);
  assign y = en ? d : 8'bz;
endmodule

module dut;
  reg en1, en2, en3;
  reg [7:0] d1, d2, d3;
  wire [7:0] bus;
  wire [7:0] split;
  wire [127:0] wide;
  wire [7:0] mod_bus;
  reg [3:0] lo_d, hi_d;
  reg lo_en, hi_en;
  reg [127:0] wd1, wd2;

  // 3ドライバのバス
  assign bus = en1 ? d1 : 8'bz;
  assign bus = en2 ? d2 : 8'bz;
  assign bus = en3 ? d3 : 8'bz;

  // 部分選択・ビット選択による分割駆動
  assign split[3:0] = lo_en ? lo_d : 4'bz;
  assign split[7:4] = hi_en ? hi_d : 4'bz;

  // 64bit超の幅
  assign wide = en1 ? wd1 : 128'bz;
  assign wide = en2 ? wd2 : 128'bz;

  // 子モジュールのoutput同士の合流
  drv u1(.en(en1), .d(d1), .y(mod_bus));
  drv u2(.en(en2), .d(d2), .y(mod_bus));

  task show;
    begin
      $display("%0t en=%b%b%b bus=%b split=%b mod=%b wide=%h",
               $time, en1, en2, en3, bus, split, mod_bus, wide);
    end
  endtask

  initial begin
    en1 = 0; en2 = 0; en3 = 0;
    d1 = 8'hA5; d2 = 8'h5A; d3 = 8'hFF;
    lo_en = 0; hi_en = 0; lo_d = 4'h3; hi_d = 4'hC;
    wd1 = 128'hdeadbeef_cafef00d_01234567_89abcdef;
    wd2 = 128'hdeadbeef_cafef00d_01234567_89abcdef;
    #1 show;                       // 全Z
    en1 = 1; #1 show;              // 1ドライバ
    en2 = 1; #1 show;              // 競合 (A5 vs 5A -> 全ビットX)
    d2 = 8'hA5; #1 show;           // 一致 -> 値
    d2 = 8'hA4; #1 show;           // 1ビットだけ競合
    en1 = 0; #1 show;              // 別のドライバだけ
    en3 = 1; d3 = 8'h5B; #1 show;  // d2=A4 vs d3=5B 全ビット競合
    en2 = 0; en3 = 0; #1 show;     // 全Zへ戻る
    lo_en = 1; #1 show;            // 下位だけ駆動
    hi_en = 1; #1 show;            // 全体駆動
    lo_en = 0; #1 show;            // 上位だけ
    wd2 = 128'hdeadbeef_cafef00d_01234567_89abcdee;
    en1 = 1; en2 = 1; #1 show;     // wide: 最下位ビットだけ競合
    $finish;
  end
endmodule
