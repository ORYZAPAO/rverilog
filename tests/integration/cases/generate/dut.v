module gate_unit(input a, input b, output o);
  parameter OP = 0; // 0:and 1:or
  assign o = (OP == 0) ? (a & b) : (a | b);
endmodule

module dut #(parameter WIDTH = 4);
  reg [WIDTH-1:0] a, b;
  wire [WIDTH-1:0] o_and, o_or;

  genvar i;
  generate
    for (i = 0; i < WIDTH; i = i + 1) begin
      gate_unit #(.OP(0)) u_and(.a(a[i]), .b(b[i]), .o(o_and[i]));
      gate_unit #(.OP(1)) u_or(.a(a[i]), .b(b[i]), .o(o_or[i]));
    end
  endgenerate

  generate
    if (WIDTH > 2) begin
      initial $display("WIDTH>2 branch taken");
    end else begin
      initial $display("WIDTH<=2 branch taken");
    end
  endgenerate

  initial begin
    a = 4'b1010;
    b = 4'b0110;
    #1 $display("a=%b b=%b and=%b or=%b", a, b, o_and, o_or);
    $finish;
  end
endmodule
