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
fn test_while_loop_is_unsupported() {
    let result = parse_source(
        "while_loop",
        r#"
            module top;
                initial while (1) begin end
            endmodule
        "#,
    );

    assert!(matches!(
        &result,
        Err(rverilog_frontend::FrontendError::UnsupportedConstruct(_))
    ));
    assert!(format!("{}", result.unwrap_err()).contains("loop statement variant"));
}

#[test]
fn test_repeat_loop_is_unsupported() {
    let result = parse_source(
        "repeat_loop",
        r#"
            module top;
                initial repeat (2) begin end
            endmodule
        "#,
    );

    assert!(matches!(
        &result,
        Err(rverilog_frontend::FrontendError::UnsupportedConstruct(_))
    ));
    assert!(format!("{}", result.unwrap_err()).contains("loop statement variant"));
}

#[test]
fn test_forever_loop_is_unsupported() {
    let result = parse_source(
        "forever_loop",
        r#"
            module top;
                initial forever begin end
            endmodule
        "#,
    );

    assert!(matches!(
        &result,
        Err(rverilog_frontend::FrontendError::UnsupportedConstruct(_))
    ));
    assert!(format!("{}", result.unwrap_err()).contains("loop statement variant"));
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
