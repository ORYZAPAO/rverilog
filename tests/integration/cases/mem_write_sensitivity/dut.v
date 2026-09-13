module dut;
  reg clk;
  reg [7:0] mem [0:0];
  reg [7:0] observed;

  always #5 clk = ~clk;

  always @(posedge clk)
    mem[0] <= 8'ha5;

  always @*
    observed = mem[0];

  initial begin
    clk = 0;
    #6 $display("observed=%h", observed);
    $finish;
  end
endmodule
