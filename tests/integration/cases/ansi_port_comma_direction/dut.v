module child(
  input clk, rst,
  output reg [3:0] cnt
);
  always @(posedge clk) begin
    if (!rst) cnt <= 4'd0;
    else cnt <= cnt + 1;
  end
endmodule

module top;
  reg clk, rst;
  wire [3:0] cnt;
  child dut(.clk(clk), .rst(rst), .cnt(cnt));

  initial begin
    clk = 0;
    rst = 0;
    #10 rst = 1;
  end

  always #5 clk = ~clk;

  initial begin
    #100;
    $display("cnt=%d", cnt);
    $finish;
  end
endmodule
