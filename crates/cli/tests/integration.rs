use std::path::{Path, PathBuf};
use rverilog_frontend::parse_files;
use rverilog_elab::elaborate;
use rverilog_sim::Interpreter;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()  // crates/
        .parent().unwrap()  // workspace root
        .to_path_buf()
}

fn run_sim(top: &str, files: &[PathBuf]) -> String {
    let design = parse_files(files, &[], &[]).expect("parse failed");
    let elaborated = elaborate(&design, top, &[]).expect("elaborate failed");
    let mut interp = Interpreter::new(elaborated);
    interp.run();
    interp.output().to_string()
}

fn expected_stdout(case: &str) -> String {
    let path = workspace_root()
        .join("tests/integration/cases")
        .join(case)
        .join("expected.stdout");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", path.display(), e))
}

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
