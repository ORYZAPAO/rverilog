module dut;
  reg [7:0] a, b;
  reg [7:0] r;

  function [7:0] add8;
    input [7:0] x;
    input [7:0] y;
    begin
      add8 = x + y;
    end
  endfunction

  task show_sum;
    input [7:0] x;
    input [7:0] y;
    output [7:0] sum;
    begin
      sum = x + y;
      $display("show_sum: %d + %d = %d", x, y, sum);
    end
  endtask

  initial begin
    a = 3;
    b = 4;
    r = add8(a, b);
    $display("add8(%d,%d) = %d", a, b, r);
    show_sum(a, b, r);
    $display("after task r=%d", r);
    $finish;
  end
endmodule
