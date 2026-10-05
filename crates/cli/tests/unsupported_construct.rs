fn parse_source(
    name: &str,
    source: &str,
) -> Result<rverilog_hir::Design, rverilog_frontend::FrontendError> {
    let path = std::env::temp_dir().join(format!("rverilog_test_{}.v", name));
    std::fs::write(&path, source).expect("write temporary Verilog source");
    let result = rverilog_frontend::parse_files(std::slice::from_ref(&path), &[], &[]);
    std::fs::remove_file(path).expect("remove temporary Verilog source");
    result
}

#[test]
fn test_defparam_is_unsupported() {
    let result = parse_source(
        "defparam",
        r#"
            module child #(parameter WIDTH = 1) (); endmodule
            module top;
                child u_child();
                defparam u_child.WIDTH = 8;
            endmodule
        "#,
    );

    assert!(
        matches!(
            &result,
            Err(rverilog_frontend::FrontendError::UnsupportedConstruct(_))
        ),
        "expected UnsupportedConstruct, got {result:?}"
    );
    assert!(format!("{}", result.unwrap_err()).contains("defparam"));
}

#[test]
fn test_specify_block_is_unsupported() {
    let result = parse_source(
        "specify",
        r#"
            module top(input a, output y);
                assign y = a;
                specify
                    (a => y) = 1;
                endspecify
            endmodule
        "#,
    );

    assert!(matches!(
        &result,
        Err(rverilog_frontend::FrontendError::UnsupportedConstruct(_))
    ));
    assert!(format!("{}", result.unwrap_err()).contains("specify"));
}

#[test]
fn test_udp_instantiation_is_unsupported() {
    let result = parse_source(
        "udp",
        r#"
            primitive and_udp (out, a, b);
                output out;
                input a, b;
                table
                    0 ? : 0;
                    ? 0 : 0;
                    1 1 : 1;
                endtable
            endprimitive

            module top(input a, input b, output y);
                and_udp (strong0, strong1) u_and(y, a, b);
            endmodule
        "#,
    );

    assert!(matches!(
        &result,
        Err(rverilog_frontend::FrontendError::UnsupportedConstruct(_))
    ));
    let message = format!("{}", result.unwrap_err());
    assert!(message.contains("UDP") || message.contains("udp"));
}

#[test]
fn test_power_operator_is_unsupported() {
    let result = parse_source(
        "power_operator",
        r#"
            module top;
                reg [7:0] value;
                initial value = 2 ** 3;
            endmodule
        "#,
    );

    assert!(matches!(
        &result,
        Err(rverilog_frontend::FrontendError::UnsupportedConstruct(_))
    ));
    assert!(format!("{}", result.unwrap_err()).contains("binary op '**'"));
}

#[test]
fn test_function_call_in_expression_is_accepted() {
    let result = parse_source(
        "function_call_in_expression",
        r#"
            module top;
                function [3:0] increment;
                    input [3:0] value;
                    begin
                        increment = value + 1;
                    end
                endfunction

                reg [3:0] result;
                initial result = increment(4'd1);
            endmodule
        "#,
    );

    assert!(
        result.is_ok(),
        "expected function call expression to parse: {result:?}"
    );
}

#[test]
fn test_inout_non_plain_net_connection_is_unsupported() {
    let design = parse_source(
        "inout_bit_select",
        r#"
            module child(inout p); endmodule
            module top;
                wire [3:0] bus;
                child u(.p(bus[1]));
            endmodule
        "#,
    )
    .expect("parse");
    let result = rverilog_elab::elaborate(&design, "top", &[]);
    assert!(
        matches!(
            &result,
            Err(rverilog_elab::ElabError::UnsupportedConstruct(m)) if m.contains("inout")
        ),
        "expected UnsupportedConstruct(inout), got {:?}",
        result.err()
    );
}

fn assert_unsupported(name: &str, source: &str, needle: &str) {
    let result = parse_source(name, source);
    match &result {
        Err(rverilog_frontend::FrontendError::UnsupportedConstruct(m)) => {
            assert!(
                m.contains(needle),
                "message `{m}` should contain `{needle}`"
            )
        }
        other => panic!("expected UnsupportedConstruct({needle}), got {other:?}"),
    }
}

#[test]
fn test_bidirectional_switches_are_unsupported() {
    assert_unsupported(
        "tran_gate",
        "module top; wire [1:0] a; wire b; tran (a[0], b); endmodule",
        "bidirectional switch terminal",
    );
    assert_unsupported(
        "tranif_gate",
        "module top; wire [1:0] a; wire b, c; tranif1 (a[1], b, c); endmodule",
        "bidirectional switch terminal",
    );
}

#[test]
fn test_unsupported_gate_inside_generate_is_reported() {
    assert_unsupported(
        "generate_tran",
        r#"module top;
            wire [1:0] a; wire b;
            generate if (1) begin : g tran (a[0], b); end endgenerate
        endmodule"#,
        "bidirectional switch terminal",
    );
}

#[test]
fn test_nonansi_port_without_direction_is_reported() {
    let result = parse_source(
        "nonansi_no_dir",
        "module m(a, b); input a; endmodule module top; endmodule",
    );
    match &result {
        Err(rverilog_frontend::FrontendError::ParseError(m)) => {
            assert!(m.contains("`b`"), "unexpected message: {m}")
        }
        other => panic!("expected ParseError for port without direction, got {other:?}"),
    }
}

#[test]
fn test_nonansi_declaration_not_in_port_list_is_reported() {
    let result = parse_source(
        "nonansi_extra",
        "module m(a); input a; output y; endmodule module top; endmodule",
    );
    assert!(
        matches!(&result, Err(rverilog_frontend::FrontendError::ParseError(m)) if m.contains("`y`")),
        "expected ParseError naming `y`, got {result:?}"
    );
}
