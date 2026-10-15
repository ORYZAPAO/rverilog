module dut;
  reg value;
  initial begin
    value = 0;
    $monitor("m %0t %b", $time, value);
    #1 $monitoroff;
    value = 1;
    #1 $monitoron;
    #1 $finish;
  end
endmodule
