module dut;
  reg [1:0] x;
  reg [3:0] y;

  wire x_and;
  wire x_nand;
  wire x_or;
  wire x_nor;
  wire x_xor;
  wire x_xnor_tilde_caret;
  wire x_xnor_caret_tilde;

  wire y_and;
  wire y_nand;
  wire y_or;
  wire y_nor;
  wire y_xor;
  wire y_xnor_tilde_caret;
  wire y_xnor_caret_tilde;

  assign x_and = &x;
  assign x_nand = ~&x;
  assign x_or = |x;
  assign x_nor = ~|x;
  assign x_xor = ^x;
  assign x_xnor_tilde_caret = ~^x;
  assign x_xnor_caret_tilde = ^~x;

  assign y_and = &y;
  assign y_nand = ~&y;
  assign y_or = |y;
  assign y_nor = ~|y;
  assign y_xor = ^y;
  assign y_xnor_tilde_caret = ~^y;
  assign y_xnor_caret_tilde = ^~y;

  initial begin
    x = 2'b00; y = 4'b0000; #1;
    $display("x=%b &= %b ~&= %b |= %b ~|= %b ^= %b ~^= %b ^~= %b", x, x_and, x_nand, x_or, x_nor, x_xor, x_xnor_tilde_caret, x_xnor_caret_tilde);
    $display("y=%b &= %b ~&= %b |= %b ~|= %b ^= %b ~^= %b ^~= %b", y, y_and, y_nand, y_or, y_nor, y_xor, y_xnor_tilde_caret, y_xnor_caret_tilde);
    x = 2'b01; y = 4'b0001; #1;
    $display("x=%b &= %b ~&= %b |= %b ~|= %b ^= %b ~^= %b ^~= %b", x, x_and, x_nand, x_or, x_nor, x_xor, x_xnor_tilde_caret, x_xnor_caret_tilde);
    $display("y=%b &= %b ~&= %b |= %b ~|= %b ^= %b ~^= %b ^~= %b", y, y_and, y_nand, y_or, y_nor, y_xor, y_xnor_tilde_caret, y_xnor_caret_tilde);
    x = 2'b11; y = 4'b1111; #1;
    $display("x=%b &= %b ~&= %b |= %b ~|= %b ^= %b ~^= %b ^~= %b", x, x_and, x_nand, x_or, x_nor, x_xor, x_xnor_tilde_caret, x_xnor_caret_tilde);
    $display("y=%b &= %b ~&= %b |= %b ~|= %b ^= %b ~^= %b ^~= %b", y, y_and, y_nand, y_or, y_nor, y_xor, y_xnor_tilde_caret, y_xnor_caret_tilde);
    x = 2'b10; y = 4'b1010; #1;
    $display("x=%b &= %b ~&= %b |= %b ~|= %b ^= %b ~^= %b ^~= %b", x, x_and, x_nand, x_or, x_nor, x_xor, x_xnor_tilde_caret, x_xnor_caret_tilde);
    $display("y=%b &= %b ~&= %b |= %b ~|= %b ^= %b ~^= %b ^~= %b", y, y_and, y_nand, y_or, y_nor, y_xor, y_xnor_tilde_caret, y_xnor_caret_tilde);
    $finish;
  end
endmodule
