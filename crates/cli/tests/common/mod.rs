#![allow(dead_code)]
use rverilog_elab::elaborate;
use rverilog_frontend::parse_files;
use rverilog_sim::Interpreter;
use std::path::{Path, PathBuf};

pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap() // crates/
        .parent()
        .unwrap() // workspace root
        .to_path_buf()
}

pub fn run_sim(top: &str, files: &[PathBuf]) -> String {
    let design = parse_files(files, &[], &[]).expect("parse failed");
    let elaborated = elaborate(&design, top, &[]).expect("elaborate failed");
    let mut interp = Interpreter::new(elaborated);
    interp.run();
    interp.output().to_string()
}

pub fn expected_stdout(case: &str) -> String {
    let path = workspace_root()
        .join("tests/integration/cases")
        .join(case)
        .join("expected.stdout");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", path.display(), e))
}
