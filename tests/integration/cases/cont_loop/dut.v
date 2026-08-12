module dut;
  reg a;

  assign a = ~a;

  initial begin
    a = 0;
    #1 $finish;
  end
endmodule
