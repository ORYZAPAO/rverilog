module dut;
  reg [3:0] a, b, c;
  reg [7:0] w8;
  reg [4:0] x5;
  reg [15:0] x16;
  reg [3:0] x4;
  reg signed [3:0] sa, sb;
  reg signed [7:0] sx;
  reg signed [4:0] sx5;
  reg [63:0] big;
  reg [127:0] x128;
  wire [4:0] cw;
  wire [8:0] cw9;
  reg r1;

  function [7:0] f8;
    input [3:0] p, q;
    begin f8 = p + q; end
  endfunction

  assign cw = a + b;
  assign cw9 = {1'b0, a} * b + c;

  initial begin
    a = 4'd12; b = 4'd10; c = 4'd9; w8 = 8'd200;
    sa = -4'sd3; sb = 4'sd5;

    // 算術: LHS幅が文脈になる
    x5 = a + b;           $display("add5=%0d", x5);
    x16 = a * b;          $display("mul16=%0d", x16);
    x16 = a * b * c;      $display("mul3=%0d", x16);
    x5 = a - b - c;       $display("sub5=%0d", x5);
    x4 = a + b;           $display("add4=%0d", x4);
    x16 = (a + b) + (c + c); $display("nest=%0d", x16);
    x16 = a + b + 16'd0;  $display("with16=%0d", x16);

    // ビット演算・単項
    x16 = ~a;             $display("not16=%b", x16);
    x16 = -a;             $display("neg16=%0d", x16);
    x16 = a & ~b;         $display("andnot=%b", x16);
    x5 = a ^ {1'b1, b};   $display("xor=%b", x5);

    // 比較: オペランドは互いの最大幅で評価
    $display("cmp1=%b", (a + b) > 5'd15);
    $display("cmp2=%b", (a + b) == 5'd22);
    $display("cmp3=%b", a + b == 22);
    $display("cmp4=%b", (a * b) > 8'd100);
    r1 = (a + b) > c + 5'd0;  $display("cmp5=%b", r1);

    // 自己決定: $displayの引数、連結内、条件、添字、リダクション
    $display("self=%0d", a + b);
    $display("concat=%b", {a + b, 1'b1});
    $display("concat2=%b", {1'b0, a + b});
    $display("red=%b", &(a + b));
    $display("lognot=%b", !(a + b));
    $display("cond=%0d", (a + b) ? 8'd1 : 8'd2);
    w8 = 8'd5; x16 = w8[a + b - 4'd10]; $display("idx=%b", x16);

    // ?: の枝は文脈幅
    x16 = 1'b1 ? a + b : 5'd0;  $display("tern=%0d", x16);
    x16 = 1'b0 ? 4'd0 : a + b;  $display("tern2=%0d", x16);

    // シフト
    x16 = a << 4;                 $display("shl=%0d", x16);
    x16 = (a + b) << 1;           $display("shl2=%0d", x16);
    x16 = (a << 2) + (b << 2);    $display("shl3=%0d", x16);
    x5 = a >> 1;                  $display("shr=%0d", x5);

    // signed
    sx = sa + sb;                 $display("s1=%0d", sx);
    sx = sa * sb;                 $display("s2=%0d", sx);
    sx = sa - 4'sd7;              $display("s3=%0d", sx);
    sx5 = sa + sb + sb;           $display("s4=%0d", sx5);
    sx = -sa;                     $display("s5=%0d", sx);
    sx = sa + a;                  $display("mixed=%0d", sx);   // unsignedが混ざるとunsigned式
    x16 = sa + sb;                $display("s6=%0d", x16);
    sx = sa >>> 1;                $display("s7=%0d", sx);
    $display("scmp=%b", (sa + sb) < 5'sd3);

    // 連続代入・関数
    #1 $display("cw=%0d cw9=%0d", cw, cw9);
    x16 = f8(a, b);               $display("f8=%0d", x16);

    // 64bit超
    big = 64'hffff_ffff_ffff_ffff;
    x128 = big + 64'd1;           $display("big=%h", x128);
    x128 = big * 64'd2;           $display("big2=%h", x128);
    $finish;
  end
endmodule
