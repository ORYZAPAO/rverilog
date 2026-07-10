module tb_fifo_sync;

localparam WIDTH = 8;
localparam DEPTH = 16;

wire [WIDTH-1:0] dout;
reg clk;
reg rst;
reg wr_en;
reg rd_en;
reg [WIDTH-1:0] din;
wire full;
wire empty;
integer i;

fifo_sync #(.WIDTH(WIDTH), .DEPTH(DEPTH)) dut (
    .clk(clk),
    .rst(rst),
    .wr_en(wr_en),
    .rd_en(rd_en),
    .din(din),
    .dout(dout),
    .full(full),
    .empty(empty)
);

initial begin
  $dumpvars(0);
  $display("Starting FIFO test");
    clk = 0;
    rst = 1;
    wr_en = 0;
    rd_en = 0;
    din = 0;
    #10;
    rst = 0;

    // Write DEPTH values
    #20;
    wr_en = 1;
    for (i = 0; i < DEPTH; i = i + 1) begin
        din = i;
        #10;
    end
    wr_en = 0;
    $display("Write done, full=%b empty=%b", full, empty);

    // Read back and verify
    #20;
    rd_en = 1;
    for (i = 0; i < DEPTH; i = i + 1) begin
        #10;
        $display("read[%0d] = %0d", i, dout);
    end
    rd_en = 0;
    $display("Read done, full=%b empty=%b", full, empty);

    $display("FIFO test completed");
    $finish;
end

always #5 clk = ~clk;

endmodule
