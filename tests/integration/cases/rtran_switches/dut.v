module dut;
  // 抵抗性双方向スイッチ: 通過する信号のstrengthが減衰する
  reg d1, d2;
  wire a, b;
  assign a = d1;
  assign b = d2;
  rtran r1(a, b);

  // 直列2段（減衰が2回かかる）
  wire s1, s2, s3;
  reg e1, e2, e3;
  assign s1 = e1 ? d1 : 1'bz;
  assign s2 = e2 ? d2 : 1'bz;
  assign s3 = e3 ? d1 : 1'bz;
  rtran r2(s1, s2);
  rtran r3(s2, s3);

  // 条件つき
  reg c;
  wire x, y, x0, y0;
  assign x = d1;
  assign y = d2;
  rtranif1 r4(x, y, c);
  assign x0 = d1;
  assign y0 = d2;
  rtranif0 r5(x0, y0, c);

  // weak駆動との競合: 減衰したstrong(pull)がweakに勝つ
  wire w1, w2;
  assign (weak1, weak0) w1 = d2;
  assign w2 = d1;
  rtran r6(w1, w2);

  // pull併用とtran/rtran混在
  tri1 p1;
  wire p2, p3;
  assign p3 = d1;
  rtran r7(p1, p2);
  tran t1(p2, p3);

  integer i, j, k;
  initial begin
    for (i = 0; i < 4; i = i + 1) begin
      for (j = 0; j < 4; j = j + 1) begin
        case (i) 0: d1 = 1'b0; 1: d1 = 1'b1; 2: d1 = 1'bx; 3: d1 = 1'bz; endcase
        case (j) 0: d2 = 1'b0; 1: d2 = 1'b1; 2: d2 = 1'bx; 3: d2 = 1'bz; endcase
        for (k = 0; k < 4; k = k + 1) begin
          case (k) 0: c = 1'b0; 1: c = 1'b1; 2: c = 1'bx; 3: c = 1'bz; endcase
          e1 = k[0]; e2 = 1'b1; e3 = k[1];
          #1 $display("d1=%b d2=%b c=%b e1=%b e3=%b rtran=%b,%b chain=%b,%b,%b if1=%b,%b if0=%b,%b weak=%b,%b pull=%b,%b,%b",
                      d1, d2, c, e1, e3, a, b, s1, s2, s3, x, y, x0, y0, w1, w2, p1, p2, p3);
        end
      end
    end
    $finish;
  end
endmodule
