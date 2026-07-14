module dut;
  reg clk;

  always @(posedge clk)
    $display("posedge clk at %0t", $time);

  always @(negedge clk)
    $display("negedge clk at %0t", $time);

  initial begin
    // clk starts as X (uninitialized reg)
    #1 clk = 1'bx;  // X->X, no edge
    #1 clk = 1;     // X->1, IEEE: posedge
    #1 clk = 1'bz;  // 1->Z, no edge
    #1 clk = 0;     // Z->0, IEEE: negedge
    #1 clk = 1'bx;  // 0->X, no edge
    #1 clk = 1;     // X->1, posedge
    #1 clk = 0;     // 1->0, negedge
    #1 $finish;
  end
endmodule
