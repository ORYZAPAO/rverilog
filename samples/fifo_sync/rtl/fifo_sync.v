module fifo_sync
#(
    parameter WIDTH = 8,
    parameter DEPTH = 16
)
(
    input wire clk,
    input wire rst,
    input wire wr_en,
    input wire rd_en,
    input wire [WIDTH-1:0] din,
    output reg [WIDTH-1:0] dout,
    output wire full,
    output wire empty
);

localparam ADDR_WIDTH = $clog2(DEPTH);

reg [ADDR_WIDTH-1:0] wr_ptr;
reg [ADDR_WIDTH-1:0] rd_ptr;
reg [WIDTH-1:0] mem [0:DEPTH-1];
reg [ADDR_WIDTH:0] count;

always @(posedge clk or posedge rst) begin
    if (rst) begin
        wr_ptr <= 0;
        rd_ptr <= 0;
        count <= 0;
        dout <= 0;
    end else begin
        if (wr_en && !full) begin
            mem[wr_ptr] <= din;
            wr_ptr <= wr_ptr + 1;
            count <= count + 1;
        end
        if (rd_en && !empty) begin
            dout <= mem[rd_ptr];
            rd_ptr <= rd_ptr + 1;
            count <= count - 1;
        end
    end
end

assign full = (count == DEPTH);
assign empty = (count == 0);

endmodule
