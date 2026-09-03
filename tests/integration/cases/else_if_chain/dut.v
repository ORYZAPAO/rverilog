module dut;
  reg a, b, c;
  reg [2:0] out;

  initial begin
    // The final else-if must be evaluated when no final else exists.
    a = 0; b = 0; c = 1;
    out = 3'bxxx;
    if (a) out = 1;
    else if (b) out = 2;
    else if (c) out = 3;
    $display("no_else_final_else_if=%0d", out);

    // A middle else-if must be evaluated before later clauses.
    a = 0; b = 1; c = 1;
    if (a) out = 1;
    else if (b) out = 2;
    else if (c) out = 3;
    else out = 4;
    $display("with_else_middle_else_if=%0d", out);

    // The final else-if must win over the final else.
    a = 0; b = 0; c = 1;
    if (a) out = 1;
    else if (b) out = 2;
    else if (c) out = 3;
    else out = 4;
    $display("with_else_final_else_if=%0d", out);

    // The final else remains reachable when all predicates are false.
    a = 0; b = 0; c = 0;
    if (a) out = 1;
    else if (b) out = 2;
    else if (c) out = 3;
    else out = 4;
    $display("with_else_fallback=%0d", out);

    $finish;
  end
endmodule
