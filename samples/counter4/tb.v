module tb_counter4;

wire [3:0] count;
reg clk;
reg rst;

counter4 uut (
    .clk(clk),
    .rst(rst),
    .count(count)
);

initial begin
    $display("Starting counter test");
    clk = 0;
    rst = 1;
    #10;
    rst = 0;
    #100;
    $display("Final count: %d", count);
    $finish;
end

always #5 clk = ~clk;

endmodule
