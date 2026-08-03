module dut;
  reg [3:0] a;
  reg [3:0] b;
  reg [4:0] y;

  always @* begin
    y = a + b;
    $display("t=%0t y=%d", $time, y);
  end

  initial begin
    a = 1;
    b = 2;
    #1 a = 3;
    #1 b = 4;
    #1 $finish;
  end
endmodule
