module dut;
  reg c;
  reg [7:0] a, b;
  reg [127:0] wa, wb;
  initial begin
    a = 8'b1010_1100; b = 8'b1010_0101;
    wa = 128'hf0f0_0000_ffff_0000_1234_5678_9abc_def0;
    wb = 128'hf0f0_ffff_ffff_0000_1234_5678_9abc_def1;
    c = 1'bx;
    $display("x: %b %h", c ? a : b, c ? wa : wb);
    c = 1'bz;
    $display("z: %b %h", c ? a : b, c ? wa : wb);
    a = 8'bxz01_xz01; b = 8'bxz01_zx10;
    c = 1'bx;
    $display("xz: %b", c ? a : b);
    c = 1'b1;
    $display("1: %b", c ? a : b);
    c = 1'b0;
    $display("0: %b", c ? a : b);
    // 幅が異なる枝
    c = 1'bx;
    $display("w: %b", c ? 4'b1010 : 8'b0000_1010);
    $finish;
  end
endmodule
