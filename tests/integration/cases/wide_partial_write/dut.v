module dut;
  reg [127:0] r;
  reg [199:0] big;
  wire [99:0] w;
  reg [127:0] q;
  integer i;
  reg [7:0] hi8, lo8;
  reg [127:0] cc;

  initial begin
    // 初期値は全ビットX（上位チャンクも）
    $display("init r=%h", r);
    $display("init w=%h", w);

    r = 128'h0;
    r[0] = 1'b1;
    r[63] = 1'b1;
    r[64] = 1'b1;
    r[127] = 1'b1;
    $display("bit r=%h", r);
    r[64] = 1'bx;
    r[65] = 1'bz;
    $display("bitxz r=%h", r);

    // 動的ビット選択（64をまたぐ位置・範囲外）
    r = 128'h0;
    for (i = 60; i < 70; i = i + 1) r[i] = 1'b1;
    $display("dynbit r=%h", r);
    r[200] = 1'b1;
    r[i + 100] = 1'b1;
    $display("oob r=%h", r);

    // 部分選択（チャンク境界またぎ）
    r = 128'h0;
    r[71:56] = 16'hbeef;
    $display("part r=%h", r);
    r[127:100] = 28'hfffffff;
    $display("part hi r=%h", r);
    r[127:0] = 128'h0123456789abcdef_fedcba9876543210;
    $display("part full r=%h", r);
    r[95:32] = 64'hxxxxxxxx_zzzzzzzz;
    $display("part xz r=%h", r);

    // 200bit
    big = 200'h0;
    big[199:130] = 70'h3_ffff_ffff_ffff_ffff;
    big[129] = 1'b1;
    big[10:3] = 8'ha5;
    $display("big=%h", big);

    // indexed part-select 書き込み
    q = 128'h0;
    i = 60;
    q[i +: 12] = 12'habc;
    $display("idx+ q=%h", q);
    i = 100;
    q[i -: 20] = 20'h12345;
    $display("idx- q=%h", q);

    // 連結lvalueへの書き込み
    {hi8, cc[127:64], lo8} = 80'h11_aabbccddeeff0011_22;
    $display("concat hi8=%h lo8=%h cc=%h", hi8, lo8, cc);
    $finish;
  end
endmodule
