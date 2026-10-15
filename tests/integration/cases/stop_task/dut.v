module dut;
  initial begin
    $display("before");
    #5 $stop;
    $display("after");
  end
endmodule
