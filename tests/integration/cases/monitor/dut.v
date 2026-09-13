module dut;
  reg [3:0] cnt;

  initial begin
    cnt = 0;
    $monitor("t=%0t cnt=%d", $time, cnt);
  end

  initial begin
    #1 cnt = 1;
    #1 cnt = 1; // unchanged: should not print again
    #1 cnt = 2;
    #1;
    $finish;
  end
endmodule
