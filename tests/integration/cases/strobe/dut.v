module dut;
  reg [1:0] value;
  initial begin
    value = 0;
    $monitor("monitor %0t %b", $time, value);
    $strobe("strobe %0t %b", $time, value);
    value <= 2;
    #1 $strobe("last %0t %b", $time, value);
    $finish;
  end
endmodule
