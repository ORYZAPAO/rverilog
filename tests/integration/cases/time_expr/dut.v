module dut;
  reg [63:0] wide;
  reg [31:0] narrow;
  initial begin
    #7;
    wide = $time + 1;
    narrow = $stime + 1;
    $display("%0t %0t %0d %0d", $time, $stime, wide, narrow);
    $finish;
  end
endmodule
