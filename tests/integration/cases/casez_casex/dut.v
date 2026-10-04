module dut;
  reg [3:0] zsel;
  reg [3:0] xsel;
  reg [7:0] out;

  task decode_casez;
    input [3:0] value;
    begin
      casez (value)
        4'b1???: out = 8'ha1;
        4'b01z1: out = 8'hb2;
        2'b?1:   out = 8'hc3;
        default: out = 8'hd0;
      endcase
    end
  endtask

  function [7:0] decode_casex;
    input [3:0] value;
    begin
      casex (value)
        4'b1100: decode_casex = 8'hea;
        4'b10x1: decode_casex = 8'he1;
        4'b0011: decode_casex = 8'heb;
        4'b0z1z: decode_casex = 8'he2;
        2'bx1:   decode_casex = 8'hf4;
        default: decode_casex = 8'hfd;
      endcase
    end
  endfunction

  initial begin
    // 優先エンコーダ形状のcasez。?とZ、セレクタ側Z、defaultを確認する。
    zsel = 4'b1010; decode_casez(zsel); $display("casez priority=%h", out);
    zsel = 4'b0111; decode_casez(zsel); $display("casez zpat=%h", out);
    zsel = 4'b1z10; decode_casez(zsel); $display("casez zsel=%h", out);
    zsel = 4'b0001; decode_casez(zsel); $display("casez width=%h", out);
    zsel = 4'b0010; decode_casez(zsel); $display("casez default=%h", out);

    // X/Zを含むパターンとセレクタ、幅不一致、defaultを確認する。
    xsel = 4'b1x00; out = decode_casex(xsel); $display("casex xsel=%h", out);
    xsel = 4'b1011; out = decode_casex(xsel); $display("casex xpat=%h", out);
    xsel = 4'b0z11; out = decode_casex(xsel); $display("casex zsel=%h", out);
    xsel = 4'b0110; out = decode_casex(xsel); $display("casex zpat=%h", out);
    xsel = 4'b0001; out = decode_casex(xsel); $display("casex width=%h", out);
    xsel = 4'b0000; out = decode_casex(xsel); $display("casex default=%h", out);
    $finish;
  end
endmodule
