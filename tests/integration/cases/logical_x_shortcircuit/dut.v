module dut;
  reg trap;
  reg resetn;
  reg [3:0] mixed;

  initial begin
    $display("or=%b", 1'b1 || 1'bx);
    $display("and=%b", 1'b0 && 1'bx);
    if (1'b1 && 1'b1 && 1'bx && (|2'b00))
      $display("FAIL: X && 0 condition took true branch");
    else
      $display("PASS: X && 0 condition took false branch");

    mixed = 4'b1x0x;
    if (mixed)
      $display("PASS: mixed X condition took true branch");
    else
      $display("FAIL: mixed X condition took false branch");

    resetn = 0;
    if (!resetn || trap)
      $display("PASS: reset branch taken");
    else
      $display("FAIL: reset branch skipped");
  end
endmodule
