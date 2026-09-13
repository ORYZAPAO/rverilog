module dut;
  reg a, b;
  wire c = a & b;
  wire x = 1, y = x & 1;
  wire z;

  initial begin
    a = 1;
    b = 0;
    #10 b = 1;
    #10 $display("c=%b x=%b y=%b z=%b", c, x, y, z);
    $finish;
  end
endmodule
