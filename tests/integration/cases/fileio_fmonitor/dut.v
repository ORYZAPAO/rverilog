module dut;
  reg x;
  initial begin
    x = 0;
    $display("display");
    $fdisplay(1, "fdisplay");
    $fwrite(1, "write");
    $display("-display");
    $fmonitor(1, "monitor %0d", x);
    $fstrobe(1, "strobe %0d", x);
    #1 x = 1;
    #1 $finish;
  end
endmodule
