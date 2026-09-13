module dut;
  reg [1:0] st;
  reg [7:0] out;

  always @(*) begin
    out = 8'hff;
    case (st)
      0: out = 8'h11;
      1: out = 8'h22;
      2: out = 8'h33;
      3: out = 8'h44;
    endcase
  end

  initial begin
    st = 2'b00; #1 $display("st=%b out=%h", st, out);
    st = 2'b01; #1 $display("st=%b out=%h", st, out);
    st = 2'b10; #1 $display("st=%b out=%h", st, out);
    st = 2'b11; #1 $display("st=%b out=%h", st, out);
    $display("caseeq=%b casene=%b", 4'd5 === 32'd5, 4'd5 !== 32'd5);
    $finish;
  end
endmodule
