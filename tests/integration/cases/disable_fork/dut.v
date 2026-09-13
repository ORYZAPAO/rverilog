module dut;
  reg [31:0] i;
  reg [31:0] a, b;

  initial begin
    for (i = 0; i < 10; i = i + 1) begin : loop_body
      if (i == 4) disable loop_body;
      $display("i=%0d", i);
    end
    $display("after loop i=%0d", i);
  end

  initial begin
    a = 0;
    b = 0;
    fork
      begin
        #5 a = 1;
        $display("branch a done at %0t", $time);
      end
      begin
        #2 b = 2;
        $display("branch b done at %0t", $time);
      end
    join
    $display("joined at %0t a=%0d b=%0d", $time, a, b);
    #1 $finish;
  end
endmodule
