module dut;
  reg en, d;
  trireg def;
  trireg (large) la;
  trireg (medium) me;
  trireg (small) sm;

  pullup (weak1) (la);
  pullup (weak1) (me);
  assign def = en ? d : 1'bz;
  assign la = en ? d : 1'bz;
  assign me = en ? d : 1'bz;
  assign sm = en ? d : 1'bz;

  initial begin
    en = 1; d = 1; #1;
    $display("%0t drive def=%v la=%v me=%v sm=%v", $time, def, la, me, sm);
    en = 0; #1;
    $display("%0t hold def=%v la=%v me=%v sm=%v", $time, def, la, me, sm);
    en = 1; d = 0; #1;
    $display("%0t drive def=%v la=%v me=%v sm=%v", $time, def, la, me, sm);
    en = 0; #1;
    $display("%0t resolve def=%v la=%v me=%v sm=%v", $time, def, la, me, sm);
    $finish;
  end
endmodule
