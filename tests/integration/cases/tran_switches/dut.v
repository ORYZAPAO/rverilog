module dut;
  // 双方向スイッチ: tran / tranif1 / tranif0（rtran系は未対応）
  reg ea, eb, da, db;
  wire a, b;
  assign a = ea ? da : 1'bz;
  assign b = eb ? db : 1'bz;
  tran t1(a, b);

  reg c;
  wire x, y, x0, y0;
  reg dx, dy;
  assign x = dx;
  assign y = dy;
  tranif1 t2(x, y, c);
  assign x0 = dx;
  assign y0 = dy;
  tranif0 t3(x0, y0, c);

  // 片側のみ駆動（もう一方は導通時のみ値を受ける）
  wire u, v;
  assign u = dx;
  tranif1 t5(u, v, c);

  // 多段チェーン
  reg dw;
  reg ew;
  wire w1, w2, w3;
  assign w1 = ew ? dw : 1'bz;
  tran t6(w1, w2);
  tran t7(w2, w3);

  // pull併用
  tri1 p1;
  wire p2;
  reg ep;
  assign p2 = ep ? 1'b0 : 1'bz;
  tran t8(p1, p2);

  integer i, j, k;
  initial begin
    for (i = 0; i < 4; i = i + 1) begin
      for (j = 0; j < 4; j = j + 1) begin
        ea = i[1]; da = i[0]; eb = j[1]; db = j[0];
        #1 $display("ea=%b da=%b eb=%b db=%b a=%b b=%b", ea, da, eb, db, a, b);
      end
    end
    for (i = 0; i < 4; i = i + 1) begin
      for (j = 0; j < 4; j = j + 1) begin
        for (k = 0; k < 4; k = k + 1) begin
          case (i) 0: dx = 1'b0; 1: dx = 1'b1; 2: dx = 1'bx; 3: dx = 1'bz; endcase
          case (j) 0: dy = 1'b0; 1: dy = 1'b1; 2: dy = 1'bx; 3: dy = 1'bz; endcase
          case (k) 0: c = 1'b0; 1: c = 1'b1; 2: c = 1'bx; 3: c = 1'bz; endcase
          #1 $display("dx=%b dy=%b c=%b if1=%b,%b if0=%b,%b one=%b,%b",
                      dx, dy, c, x, y, x0, y0, u, v);
        end
      end
    end
    for (i = 0; i < 8; i = i + 1) begin
      ew = i[2]; dw = i[0];
      #1 $display("ew=%b dw=%b w=%b %b %b", ew, dw, w1, w2, w3);
    end
    for (i = 0; i < 2; i = i + 1) begin
      ep = i;
      #1 $display("ep=%b p1=%b p2=%b", ep, p1, p2);
    end
    $finish;
  end
endmodule
