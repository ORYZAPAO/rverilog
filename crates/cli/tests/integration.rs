mod common;
use common::{expected_stdout, run_sim, workspace_root};

#[test]
fn test_counter4() {
    let root = workspace_root();
    let files = vec![
        root.join("samples/counter4/tb.v"),
        root.join("samples/counter4/counter4.v"),
    ];
    let got = run_sim("tb_counter4", &files);
    let expected = expected_stdout("counter4");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_func_task() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/func_task/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("func_task");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_gates() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/gates/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("gates");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_generate() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/generate/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("generate");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_disable_fork() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/disable_fork/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("disable_fork");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_readmem_random() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/readmem_random/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("readmem_random");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_format_xz() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/format_xz/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("format_xz");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_signed() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/signed/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("signed");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_signed_cast() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/signed_cast/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("signed_cast");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_lvalue_select() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/lvalue_select/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("lvalue_select");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_indexed_part_select() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/indexed_part_select/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("indexed_part_select");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_localparam_midmodule() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/localparam_midmodule/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("localparam_midmodule");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_localparam_const_expr() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/localparam_const_expr/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("localparam_const_expr");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_binop_precedence() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/binop_precedence/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("binop_precedence");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_edge_x() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/edge_x/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("edge_x");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_monitor() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/monitor/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("monitor");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_always_star() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/always_star/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("always_star");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_case_width_mismatch() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/case_width_mismatch/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("case_width_mismatch");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_logical_x_shortcircuit() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/logical_x_shortcircuit/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("logical_x_shortcircuit");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_ansi_port_comma_direction() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/ansi_port_comma_direction/dut.v")];
    let got = run_sim("top", &files);
    let expected = expected_stdout("ansi_port_comma_direction");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
#[ignore = "既知の未解決課題: A13修正後もPicoRV32が最初の命令fetch前に不正命令トラップへ入りtimeoutする（PLAN.md推奨着手順11番参照）"]
fn test_picorv32_smoke() {
    let root = workspace_root();
    let files = vec![
        root.join("tests/integration/cases/picorv32_smoke/tb.v"),
        root.join("tests/integration/cases/picorv32_smoke/picorv32.v"),
    ];
    let got = run_sim("tb_picorv32", &files);
    assert!(
        got.contains("PASS: picorv32 executed addi/add/lui/sw correctly"),
        "picorv32 smoke test did not pass:\n{}",
        got
    );
    assert!(
        !got.contains("TIMEOUT") && !got.contains("FAIL"),
        "picorv32 smoke test reported failure:\n{}",
        got
    );
}

#[test]
fn test_max_time() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/max_time/dut.v")];
    let design = rverilog_frontend::parse_files(&files, &[], &[]).expect("parse failed");
    let elaborated = rverilog_elab::elaborate(&design, "dut", &[]).expect("elaborate failed");
    let mut interp = rverilog_sim::Interpreter::new(elaborated);
    interp.set_max_time(Some(5));
    interp.run();
    assert_eq!(interp.output(), expected_stdout("max_time"));
}

#[test]
fn test_cont_loop() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/cont_loop/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("cont_loop");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_delay0() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/delay0/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("delay0");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_fifo_sync() {
    let root = workspace_root();
    let files = vec![
        root.join("samples/fifo_sync/tb/tb.v"),
        root.join("samples/fifo_sync/rtl/fifo_sync.v"),
    ];
    let got = run_sim("tb_fifo_sync", &files);
    let expected = expected_stdout("fifo_sync");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}
