// 4-bit counter module
module counter_4bit (
    input wire clk,
    input wire rst,
    input wire en,
    output reg [3:0] count
);

always @(posedge clk or posedge rst) begin
    if (rst)
        count <= 4'b0;
    else if (en)
        count <= count + 1;
end

endmodule

// Synchronous FIFO module (simple version)
module sync_fifo #
(
    parameter WIDTH = 8,
    parameter DEPTH = 16
)
(
    input wire clk,
    input wire rst,
    input wire push,
    input wire pop,
    input wire [WIDTH-1:0] data_in,
    output wire full,
    output wire empty,
    output wire [WIDTH-1:0] data_out
);

reg [WIDTH-1:0] fifo [0:DEPTH-1];
reg [WIDTH-1:0] count;
reg [4:0] read_ptr;
reg [4:0] write_ptr;

assign full = (count == DEPTH);
assign empty = (count == 0);
assign data_out = fifo[read_ptr];

always @(posedge clk or posedge rst) begin
    if (rst) begin
        count <= 0;
        read_ptr <= 0;
        write_ptr <= 0;
    end else begin
        if (push && !full) begin
            fifo[write_ptr] <= data_in;
            write_ptr <= write_ptr + 1;
            count <= count + 1;
        end
        if (pop && !empty) begin
            read_ptr <= read_ptr + 1;
            count <= count - 1;
        end
    end
end

endmodule

// Top-level testbench with FIFO and counter
module top;
    reg clk;
    reg rst;
    
    counter_4bit u_counter (
        .clk(clk),
        .rst(rst),
        .en(1'b1),
        .count(counter_val)
    );
    
    reg [7:0] fifo_data;
    reg fifo_push;
    reg fifo_pop;
    wire fifo_full;
    wire fifo_empty;
    wire [7:0] fifo_out;
    
    sync_fifo #(.WIDTH(8), .DEPTH(16)) u_fifo (
        .clk(clk),
        .rst(rst),
        .push(fifo_push),
        .pop(fifo_pop),
        .data_in(fifo_data),
        .full(fifo_full),
        .empty(fifo_empty),
        .data_out(fifo_out)
    );
    
    initial begin
        $dumpfile("output.vcd");
        $dumpvars(0, top);
        
        clk = 0;
        rst = 1;
        fifo_push = 0;
        fifo_pop = 0;
        fifo_data = 0;
        
        #10 rst = 0;
        
        #100 $display("Counter value: %d", counter_val);
        
        fifo_push = 1;
        fifo_data = 8'hAA;
        #10;
        fifo_push = 0;
        
        #100 $display("FIFO out: %h, Full: %d", fifo_out, fifo_full);
        
        #100 $display("Final counter: %d", counter_val);
        
        #10 $finish;
    end
    
    always #5 clk = ~clk;
    
endmodule
