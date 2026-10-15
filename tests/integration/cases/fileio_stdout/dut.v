module dut;
  reg x;
  initial begin
    x = 0;
    $display("display");
    $fdisplay(1, "fdisplay");
    $fwrite(1, "write");
    $display("-display");
    $fstrobe(1, "strobe %0d", x);
    #1 x = 1;
    #1 $finish;
  end
endmodule
