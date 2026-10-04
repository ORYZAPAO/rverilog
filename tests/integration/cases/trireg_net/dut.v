module dut;
  reg en1, en2;
  reg [7:0] d1, d2;
  trireg [7:0] t;
  trireg ts;
  trireg [127:0] tw;
  reg [127:0] wd;

  assign t = en1 ? d1 : 8'bz;
  assign t = en2 ? d2 : 8'bz;
  assign ts = en1 ? d1[0] : 1'bz;
  assign tw = en1 ? wd : 128'bz;

  task show;
    begin
      $display("%0t en=%b%b t=%b ts=%b tw=%h", $time, en1, en2, t, ts, tw);
    end
  endtask

  initial begin
    en1 = 0; en2 = 0; d1 = 8'hA5; d2 = 8'h3C; wd = 128'h0123_4567_89ab_cdef_fedc_ba98_7654_3210;
    #1 show;                    // 駆動なし: 初期値x
    en1 = 1; #1 show;           // 駆動
    en1 = 0; #1 show;           // 切り離し: 値を保持
    d1 = 8'hFF; #1 show;        // 切り離し中の入力変化は無視
    en2 = 1; #1 show;           // 別ドライバ
    en2 = 0; #1 show;           // 保持
    en1 = 1; en2 = 1; #1 show;  // 競合 -> X
    en1 = 0; #1 show;
    en2 = 0; #1 show;           // Xを保持
    d1 = 8'h0F; en1 = 1; #1 show;
    $finish;
  end
endmodule
