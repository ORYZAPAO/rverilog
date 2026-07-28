mod common;
use common::{workspace_root, run_sim, expected_stdout};

#[test]
fn test_counter4() {
    let root = workspace_root();
    let files = vec![
        root.join("samples/counter4/tb.v"),
        root.join("samples/counter4/counter4.v"),
    ];
    let got = run_sim("tb_counter4", &files);
    let expected = expected_stdout("counter4");
    assert_eq!(got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}", expected, got);
}

#[test]
fn test_func_task() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/func_task/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("func_task");
    assert_eq!(got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}", expected, got);
}

#[test]
fn test_gates() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/gates/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("gates");
    assert_eq!(got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}", expected, got);
}

#[test]
fn test_generate() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/generate/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("generate");
    assert_eq!(got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}", expected, got);
}

#[test]
fn test_disable_fork() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/disable_fork/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("disable_fork");
    assert_eq!(got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}", expected, got);
}

#[test]
fn test_readmem_random() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/readmem_random/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("readmem_random");
    assert_eq!(got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}", expected, got);
}

#[test]
fn test_format_xz() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/format_xz/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("format_xz");
    assert_eq!(got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}", expected, got);
}

#[test]
fn test_signed() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/signed/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("signed");
    assert_eq!(got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}", expected, got);
}

#[test]
fn test_signed_cast() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/signed_cast/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("signed_cast");
    assert_eq!(got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}", expected, got);
}

#[test]
fn test_lvalue_select() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/lvalue_select/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("lvalue_select");
    assert_eq!(got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}", expected, got);
}

#[test]
fn test_indexed_part_select() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/indexed_part_select/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("indexed_part_select");
    assert_eq!(got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}", expected, got);
}

#[test]
fn test_binop_precedence() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/binop_precedence/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("binop_precedence");
    assert_eq!(got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}", expected, got);
}

#[test]
fn test_edge_x() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/edge_x/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("edge_x");
    assert_eq!(got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}", expected, got);
}

#[test]
fn test_monitor() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/monitor/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("monitor");
    assert_eq!(got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}", expected, got);
}

#[test]
fn test_delay0() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/delay0/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("delay0");
    assert_eq!(got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}", expected, got);
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
    assert_eq!(got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}", expected, got);
}
