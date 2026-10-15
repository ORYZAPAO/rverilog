module dut;
  initial begin
    $displayb("b", 4'b10xz, 4'bzzzz);
    $displayh("h", 4'b10xz, 4'bzzzz);
    $displayo("o", 4'b10xz, 4'bzzzz);
    $writeb("wb", 4'b10xz); $write("\n");
    $writeh("wh", 4'b10xz); $write("\n");
    $writeo("wo", 4'b10xz); $write("\n");
    $finish;
  end
endmodule
