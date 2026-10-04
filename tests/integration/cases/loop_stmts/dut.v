module dut;
  integer i;
  integer n;
  integer sum;
  reg [7:0] cnt;

  task wait_cycles;
    input integer k;
    begin
      while (k > 0) begin
        #1;
        k = k - 1;
      end
    end
  endtask

  function integer fact;
    input integer v;
    integer r;
    begin
      r = 1;
      while (v > 1) begin
        r = r * v;
        v = v - 1;
      end
      fact = r;
    end
  endfunction

  initial begin
    // while: カウントアップ
    i = 0; sum = 0;
    while (i < 5) begin
      sum = sum + i;
      i = i + 1;
    end
    $display("while sum=%0d i=%0d", sum, i);

    // while: 条件が最初から偽なら0回
    i = 10;
    while (i < 5) i = i + 1;
    $display("while zero i=%0d", i);

    // repeat: 定数
    cnt = 0;
    repeat (4) cnt = cnt + 3;
    $display("repeat const cnt=%0d", cnt);

    // repeat: 変数（回数式は開始時に一度だけ評価）
    n = 3; cnt = 0;
    repeat (n) begin
      n = n + 1;
      cnt = cnt + 1;
    end
    $display("repeat var cnt=%0d n=%0d", cnt, n);

    // repeat: 0回・負数・X
    cnt = 0;
    repeat (0) cnt = cnt + 1;
    n = -2;
    repeat (n) cnt = cnt + 1;
    repeat (1'bx) cnt = cnt + 1;
    $display("repeat zero cnt=%0d", cnt);

    // ネストしたrepeat
    cnt = 0;
    repeat (3) repeat (2) cnt = cnt + 1;
    $display("repeat nested cnt=%0d", cnt);

    // 遅延を含むループ
    wait_cycles(3);
    $display("task while t=%0t", $time);

    // function内のwhile
    $display("fact(5)=%0d", fact(5));

    // forever + 名前付きブロックのdisableで脱出
    i = 0;
    begin : outer
      forever begin
        #2;
        i = i + 1;
        if (i == 4) disable outer;
      end
    end
    $display("forever i=%0d t=%0t", i, $time);

    $finish;
  end
endmodule
