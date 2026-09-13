module dut;
  reg clk;
  reg [7:0] dummy;

  // picorv32.v の状態機械と同じ配置パターン: 通常の宣言・alwaysブロックに
  // 挟まれてモジュール中盤で localparam を宣言する（PLAN.md A11参照）。
  initial clk = 0;
  always @(posedge clk) dummy <= dummy + 1;

  // サイズ付き基数リテラルで初期化される localparam（PLAN.md A11の再発防止）。
  // 単純な10進数値ではなくone-hotの2進リテラルであることが重要
  // （parse_simple_const_expr が旧実装ではこの形式を識別子と誤認していた）。
  localparam state_trap  = 8'b10000000;
  localparam state_fetch = 8'b01000000;
  localparam state_exec  = 8'b00001000;

  reg [7:0] state;

  initial begin
    state = state_fetch;
    case (state)
      state_trap:  $display("trap");
      state_fetch: $display("fetch");
      state_exec:  $display("exec");
      default:     $display("default, state=%b", state);
    endcase

    state = state_exec;
    case (state)
      state_trap:  $display("trap");
      state_fetch: $display("fetch");
      state_exec:  $display("exec");
      default:     $display("default, state=%b", state);
    endcase

    $finish;
  end
endmodule
