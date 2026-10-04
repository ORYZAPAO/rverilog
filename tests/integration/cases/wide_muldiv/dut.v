module dut;
  reg [127:0] a, b;
  reg signed [127:0] sa, sb;
  reg [199:0] wa, wb;

  // 64bit超の%h表示は未対応のため、64bit以下に分割して表示する
  task p128;
    input [127:0] v;
    begin
      $display("%h_%h", v[127:64], v[63:0]);
    end
  endtask

  task p200;
    input [199:0] v;
    begin
      $display("%h_%h_%h_%h", v[199:150], v[149:100], v[99:50], v[49:0]);
    end
  endtask

  initial begin
    a = 128'hdeadbeef_cafef00d_01234567_89abcdef;
    b = 128'h00000000_00000000_00000001_00000003;
    $write("mul="); p128(a * b);
    $write("div="); p128(a / b);
    $write("mod="); p128(a % b);

    b = 128'd1234567;
    $write("div64="); p128(a / b);
    $write("mod64="); p128(a % b);
    $write("small_div_big="); p128(b / a);
    $write("small_mod_big="); p128(b % a);

    b = 128'd0;
    $write("div0="); p128(a / b);
    $write("mod0="); p128(a % b);

    sa = -128'sh00000000033b2e3c9fd0803ce8000000;
    sb = 128'sd7000000000000;
    $write("sdiv="); p128(sa / sb);
    $write("smod="); p128(sa % sb);
    sb = -128'sd7000000000000;
    $write("sdiv_nn="); p128(sa / sb);
    $write("smod_nn="); p128(sa % sb);
    sa = 128'sh00000000033b2e3c9fd0803ce8000000;
    $write("sdiv_pn="); p128(sa / sb);
    $write("smod_pn="); p128(sa % sb);

    wa = 200'hab_0000000000005678_0000000000001234_ffffffffffffffff;
    wb = 200'h77_00000000ffff0001;
    $write("wmul="); p200(wa * wb);
    $write("wdiv="); p200(wa / wb);
    $write("wmod="); p200(wa % wb);

    $finish;
  end
endmodule
