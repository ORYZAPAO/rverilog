module dut;
  reg [7:0] mem[0:7];
  integer i;

  initial begin
    $readmemh("../../tests/integration/cases/readmem_random/mem.hex", mem);
    for (i = 0; i < 6; i = i + 1) begin
      $display("mem[%0d]=%h", i, mem[i]);
    end
    $display("rand0=%0d", $random);
    $display("rand1=%0d", $random);
    $display("rand_seeded=%0d", $random(42));
    $finish;
  end
endmodule
