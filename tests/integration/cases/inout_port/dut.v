// 双方向パッド: oeが1なら dout を外へ出し、常に外の値を din として読む
module pad(inout [7:0] io, input oe, input [7:0] dout, output [7:0] din);
  assign io = oe ? dout : 8'bz;
  assign din = io;
endmodule

// inoutを素通しする中間階層
module wrap(inout [7:0] p, input oe, input [7:0] dout, output [7:0] din);
  pad u(.io(p), .oe(oe), .dout(dout), .din(din));
endmodule

module dut;
  wire [7:0] line;
  reg ext_oe, pad_oe;
  reg [7:0] ext_d, pad_d;
  wire [7:0] din;

  // 外部ドライバ（親側）
  assign line = ext_oe ? ext_d : 8'bz;

  wrap w(.p(line), .oe(pad_oe), .dout(pad_d), .din(din));

  task show;
    begin
      $display("%0t ext_oe=%b pad_oe=%b line=%b din=%b", $time, ext_oe, pad_oe, line, din);
    end
  endtask

  initial begin
    ext_oe = 0; pad_oe = 0; ext_d = 8'h3C; pad_d = 8'hC3;
    #1 show;                    // 両方Z
    ext_oe = 1; #1 show;        // 親が駆動 -> 子が読める
    ext_oe = 0; pad_oe = 1; #1 show;  // 子が駆動 -> 親が読める
    ext_oe = 1; #1 show;        // 競合
    pad_d = 8'h3C; #1 show;     // 一致
    pad_oe = 0; ext_d = 8'h81; #1 show;
    $finish;
  end
endmodule
