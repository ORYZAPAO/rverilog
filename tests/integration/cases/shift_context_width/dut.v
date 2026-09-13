module dut;
  reg  [31:0] instr;
  reg  [31:0] imm_blocking;
  reg  [31:0] imm_nba;
  wire [31:0] imm_cont;
  reg  [19:0] narrow;

  assign imm_cont = instr[31:12] << 12;

  initial begin
    instr = 32'h100000B7;
    imm_blocking = instr[31:12] << 12;
    narrow = instr[31:12];
    #1;
    imm_nba <= narrow << 12;
    #1;
    $display("imm_blocking=%08h imm_nba=%08h imm_cont=%08h", imm_blocking, imm_nba, imm_cont);
    $finish;
  end
endmodule
