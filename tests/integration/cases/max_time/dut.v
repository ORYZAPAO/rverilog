module dut;
  always begin
    #1 $display("tick %0t", $time);
  end
endmodule
