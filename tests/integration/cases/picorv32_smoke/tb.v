module tb_picorv32;
  reg clk;
  reg resetn;

  wire trap;
  wire mem_valid;
  wire mem_instr;
  wire mem_ready;
  wire [31:0] mem_addr;
  wire [31:0] mem_wdata;
  wire [3:0] mem_wstrb;
  wire [31:0] mem_rdata;

  // 256語のワードアドレス・メモリ（命令・データ共用）。
  // mem_addr[9:2] でインデクス化（4バイトアラインのワードアクセスのみ想定）。
  reg [31:0] mem [0:255];

  // このアドレスへのストアを「テスト終了・結果出力」として横取りする。
  localparam FINISH_ADDR = 32'h1000_0000;

  // ゼロウェイトステートのメモリモデル（mem_valid の同サイクルで mem_ready を返す）。
  assign mem_ready = mem_valid;
  assign mem_rdata = mem[mem_addr[9:2]];

  picorv32 dut (
    .clk(clk),
    .resetn(resetn),
    .trap(trap),
    .mem_valid(mem_valid),
    .mem_instr(mem_instr),
    .mem_ready(mem_ready),
    .mem_addr(mem_addr),
    .mem_wdata(mem_wdata),
    .mem_wstrb(mem_wstrb),
    .mem_rdata(mem_rdata),
    .mem_la_read(),
    .mem_la_write(),
    .mem_la_addr(),
    .mem_la_wdata(),
    .mem_la_wstrb(),
    .pcpi_valid(),
    .pcpi_insn(),
    .pcpi_rs1(),
    .pcpi_rs2(),
    .pcpi_wr(1'b0),
    .pcpi_rd(32'b0),
    .pcpi_wait(1'b0),
    .pcpi_ready(1'b0),
    .irq(32'b0),
    .eoi(),
    .trace_valid(),
    .trace_data()
  );

  // テストプログラム（RV32I、手アセンブル。x1=5, x2=37, x3=x1+x2=42 を
  // 0x10000000 へストアして終了を通知する）。
  initial begin
    mem[0] = 32'h00500093; // addi x1, x0, 5
    mem[1] = 32'h02500113; // addi x2, x0, 37
    mem[2] = 32'h002081b3; // add  x3, x1, x2
    mem[3] = 32'h10000237; // lui  x4, 0x10000
    mem[4] = 32'h00322023; // sw   x3, 0(x4)
  end

  always @(posedge clk) begin
    if (mem_valid && mem_ready && mem_addr == FINISH_ADDR) begin
      $display("RESULT=%0d", mem_wdata);
      if (mem_wdata == 32'd42)
        $display("PASS: picorv32 executed addi/add/lui/sw correctly");
      else
        $display("FAIL: unexpected result");
      $finish;
    end else if (mem_valid && mem_ready && mem_wstrb != 4'b0000) begin
      mem[mem_addr[9:2]] <= mem_wdata;
    end
  end

  initial begin
    clk = 0;
    resetn = 0;
    #10 resetn = 1;
  end
  always #5 clk = ~clk;

  initial begin
    #2000;
    $display("TIMEOUT: simulation did not finish");
    $finish;
  end
endmodule
