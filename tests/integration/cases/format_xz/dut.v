module dut;
  reg [7:0] mem[0:3];
  integer i;

  initial begin
    $readmemh("../../tests/integration/cases/format_xz/mem.hex", mem);
    for (i = 0; i < 4; i = i + 1) begin
      $display("mem[%0d]: h=%h b=%b d=%d", i, mem[i], mem[i], mem[i]);
    end
    $finish;
  end
endmodule
