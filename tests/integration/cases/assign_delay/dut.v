module dut;
  reg a, b, en;
  wire delayed;
  wire #3 declared = a & b;
  wire zero;
  wire bus;

  // 短いパルスは慣性遅延で吸収される。
  assign #5 delayed = a;
  assign #0 zero = a;
  // 遅延ドライバと即時ドライバを同じtri-stateネットで解決する。
  assign #4 bus = en ? a : 1'bz;
  assign bus = b ? 1'b1 : 1'bz;

  task show;
    begin
      $display("%0t a=%b b=%b en=%b delayed=%b declared=%b zero=%b bus=%b",
               $time, a, b, en, delayed, declared, zero, bus);
    end
  endtask

  initial begin
    a = 0; b = 0; en = 0;
    #6 show;
    #1 a = 1;
    #1 a = 0;
    #2 show;
    #1 a = 1; b = 1; en = 1;
    #6 show;
    en = 0; b = 0;
    #2 show;
    #3 show;
    $finish;
  end
endmodule
