module inc(ref logic [3:0] x, input logic go);
  always @(posedge go) x = x + 4'd1;
endmodule
module dut;
  logic [3:0] v;
  logic go;
  inc u(.x(v), .go(go));
  initial begin
    v = 4'd5; go = 0;
    #1 go = 1; #1 go = 0;
    #1 go = 1; #1 go = 0;
    $display("v=%0d", v);
    $finish;
  end
endmodule
