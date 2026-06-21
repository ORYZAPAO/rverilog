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
