module dut;
  reg a;

  initial a = 0;
  initial a <= 1;

  initial begin
    #0;
    $display("inactive: a=%b", a);
  end

  initial begin
    #1;
    $display("after: a=%b", a);
    $finish;
  end
endmodule
