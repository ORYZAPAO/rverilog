module dut;
  reg [7:0] a, b;
  reg ea, eb;

  wand [7:0] wa;
  triand [7:0] wta;
  wor [7:0] wo;
  trior [7:0] wto;
  tri0 [7:0] t0;
  tri1 [7:0] t1;
  tri0 t0_scalar;
  wire pu;
  wire pd;
  supply0 gnd;
  supply1 vdd;
  wand [127:0] wide_and;
  wor [127:0] wide_or;
  reg [127:0] wa128, wb128;

  assign wa = ea ? a : 8'bz;
  assign wa = eb ? b : 8'bz;
  assign wta = ea ? a : 8'bz;
  assign wta = eb ? b : 8'bz;
  assign wo = ea ? a : 8'bz;
  assign wo = eb ? b : 8'bz;
  assign wto = ea ? a : 8'bz;
  assign wto = eb ? b : 8'bz;
  assign t0 = ea ? a : 8'bz;
  assign t1 = ea ? a : 8'bz;
  assign t1 = eb ? b : 8'bz;
  assign t0_scalar = ea ? a[0] : 1'bz;
  assign pu = ea ? a[0] : 1'bz;
  assign pd = eb ? b[0] : 1'bz;
  assign wide_and = ea ? wa128 : 128'bz;
  assign wide_and = eb ? wb128 : 128'bz;
  assign wide_or = ea ? wa128 : 128'bz;
  assign wide_or = eb ? wb128 : 128'bz;
  pullup (pu);
  pulldown (pd);

  task show;
    begin
      $display("%0t ea=%b eb=%b", $time, ea, eb);
      $display("  wand=%b triand=%b wor=%b trior=%b", wa, wta, wo, wto);
      $display("  tri0=%b tri1=%b t0s=%b pu=%b pd=%b", t0, t1, t0_scalar, pu, pd);
      $display("  gnd=%b vdd=%b", gnd, vdd);
      $display("  wide_and=%h wide_or=%h", wide_and, wide_or);
    end
  endtask

  initial begin
    ea = 0; eb = 0; a = 8'b1100_1010; b = 8'b1010_0110;
    wa128 = 128'hf0f0_f0f0_0000_ffff_1234_5678_9abc_def0;
    wb128 = 128'h0ff0_f00f_ffff_0000_ffff_ffff_0000_0000;
    #1 show;                 // 全て駆動なし
    ea = 1; #1 show;         // 1ドライバ
    eb = 1; #1 show;         // 2ドライバ（解決）
    ea = 0; #1 show;         // もう一方だけ
    a = 8'bxz01_xz01; ea = 1; #1 show;   // X/Zを含むドライバ
    $finish;
  end
endmodule
