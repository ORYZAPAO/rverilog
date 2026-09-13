module dut #(
  parameter ENABLE_A = 1,
  parameter ENABLE_B = 0,
  parameter ENABLE_C = 1
);

  localparam OFFSET = ENABLE_A ? 32 : 16;
  localparam SIZE = (ENABLE_A ? 32 : 16) + 4 * ENABLE_B * ENABLE_C;
  localparam WITH_ANY = ENABLE_A || ENABLE_B || ENABLE_C;

  initial begin
    $display("OFFSET=%0d SIZE=%0d WITH_ANY=%0d", OFFSET, SIZE, WITH_ANY);
    $finish;
  end
endmodule
