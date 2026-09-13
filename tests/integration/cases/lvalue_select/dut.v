module dut;
  reg [7:0] q;
  reg [3:0] a, b;
  reg [7:0] hi;
  reg [3:0] lo;

  initial begin
    // LHS part-select: blocking
    q = 8'h00;
    q[7:4] = 4'ha;
    $display("q1=%h", q);

    // LHS part-select: nonblocking
    q = 8'h00;
    q[3:0] <= 4'hb;
    #1 $display("q2=%h", q);

    // LHS concat: blocking
    a = 4'h0;
    b = 4'h0;
    {a, b} = 8'hcd;
    $display("a1=%h b1=%h", a, b);

    // LHS concat: nonblocking
    a = 4'h0;
    b = 4'h0;
    {a, b} <= 8'hef;
    #1 $display("a2=%h b2=%h", a, b);

    // LHS concat: 3-way, mixed widths (8bit + 4bit)
    hi = 8'h00;
    lo = 4'h0;
    {hi, lo} = 12'hfed;
    $display("hi=%h lo=%h", hi, lo);

    $finish;
  end
endmodule
