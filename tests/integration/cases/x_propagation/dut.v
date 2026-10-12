module dut;
  reg [3:0] a;
  reg [3:0] amount;
  reg [7:0] result;
  reg signed [3:0] sa;
  reg [64:0] wide;

  initial begin
    $display("eq=%b ne=%b eqx=%b", 4'b0x00 == 4'b1x00, 4'b0x00 != 4'b1x00, 4'b0x00 == 4'b0x00);

    result = 4'b0x00 + 4'b0001;
    $display("add=%b", result);
    result = 4'b0x00 - 4'b0001;
    $display("sub=%b", result);

    a = 4'b10xz;
    amount = 1;
    $display("shl=%b shr=%b ashl=%b", a << amount, a >> amount, a <<< amount);
    sa = 4'b10xz;
    $display("ashr=%b", sa >>> amount);
    sa = 4'bx100;
    $display("ashr_x=%b", sa >>> amount);
    sa = 4'bz100;
    $display("ashr_z=%b", sa >>> amount);
    amount = 4'bx001;
    $display("shift_x=%b", a << amount);

    $display("not=%b not_x=%b and=%b and_x=%b", !4'b1x00, !4'b0x00, 4'b1x00 && 1'b1, 4'b0x00 && 1'b1);
    $display("reduce=%b%b%b%b", &4'b0x11, |4'b1x00, ~&4'b0x11, ~|4'b1x00);

    wide = 65'bx;
    $display("wide_add=%b wide_shr=%b", wide + 1'b1, wide >> 1);
    wide = {65{1'b0}};
    $display("wide_and=%b", &wide);
    $finish;
  end
endmodule
