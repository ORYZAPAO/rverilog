module dut;
  reg d, en;
  wire b1, b0, n1, n0;
  bufif1 g1(b1, d, en);
  bufif0 g2(b0, d, en);
  notif1 g3(n1, d, en);
  notif0 g4(n0, d, en);

  // 複数ゲートでバス共有
  reg e1, e2, d1, d2;
  wire bus;
  bufif1 (bus, d1, e1);
  bufif1 (bus, d2, e2);
  wire nbus;
  notif1 (nbus, d1, e1), (nbus, d2, e2);

  integer i, j;
  reg [3:0] vals;

  initial begin
    vals = 0;
    // 入力 {0,1,x,z} x enable {0,1,x,z}
    for (i = 0; i < 4; i = i + 1) begin
      for (j = 0; j < 4; j = j + 1) begin
        case (i)
          0: d = 1'b0;
          1: d = 1'b1;
          2: d = 1'bx;
          3: d = 1'bz;
        endcase
        case (j)
          0: en = 1'b0;
          1: en = 1'b1;
          2: en = 1'bx;
          3: en = 1'bz;
        endcase
        #1 $display("d=%b en=%b bufif1=%b bufif0=%b notif1=%b notif0=%b", d, en, b1, b0, n1, n0);
      end
    end

    for (i = 0; i < 16; i = i + 1) begin
      {e1, e2, d1, d2} = i[3:0];
      #1 $display("e1=%b e2=%b d1=%b d2=%b bus=%b nbus=%b", e1, e2, d1, d2, bus, nbus);
    end
    $finish;
  end
endmodule
