module dut;
  reg a, b;
  wire o_and, o_or, o_nand, o_nor, o_xor, o_xnor, o_buf, o_not;

  and  g1(o_and, a, b);
  or   g2(o_or, a, b);
  nand g3(o_nand, a, b);
  nor  g4(o_nor, a, b);
  xor  g5(o_xor, a, b);
  xnor g6(o_xnor, a, b);
  buf  g7(o_buf, a);
  not  g8(o_not, a);

  initial begin
    a = 0; b = 0; #1;
    $display("a=%b b=%b and=%b or=%b nand=%b nor=%b xor=%b xnor=%b buf=%b not=%b",
      a, b, o_and, o_or, o_nand, o_nor, o_xor, o_xnor, o_buf, o_not);
    a = 1; b = 0; #1;
    $display("a=%b b=%b and=%b or=%b nand=%b nor=%b xor=%b xnor=%b buf=%b not=%b",
      a, b, o_and, o_or, o_nand, o_nor, o_xor, o_xnor, o_buf, o_not);
    a = 1; b = 1; #1;
    $display("a=%b b=%b and=%b or=%b nand=%b nor=%b xor=%b xnor=%b buf=%b not=%b",
      a, b, o_and, o_or, o_nand, o_nor, o_xor, o_xnor, o_buf, o_not);
    $finish;
  end
endmodule
