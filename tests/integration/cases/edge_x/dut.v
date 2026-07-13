module dut;
  reg sig;
  integer pos_count;
  integer neg_count;

  initial begin
    pos_count = 0;
    neg_count = 0;
  end

  always @(posedge sig) pos_count = pos_count + 1;
  always @(negedge sig) neg_count = neg_count + 1;

  initial begin
    #1;
    sig = 1'b0; #1; // x->0
    sig = 1'bx; #1; // 0->x
    sig = 1'b1; #1; // x->1
    sig = 1'bx; #1; // 1->x
    sig = 1'b0; #1; // x->0
    sig = 1'b1; #1; // 0->1
    sig = 1'b0; #1; // 1->0
    $display("pos_count=%d neg_count=%d", pos_count, neg_count);
    $finish;
  end
endmodule
