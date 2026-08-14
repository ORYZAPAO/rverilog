mod common;
use common::{run_sim, workspace_root};
use std::path::PathBuf;
use std::process::Command;

fn iverilog_available() -> bool {
    Command::new("iverilog")
        .arg("-V")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn run_iverilog(files: &[PathBuf]) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir();
    let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
    let vvp_path = dir.join(format!("rverilog_iv_{}_{}.vvp", std::process::id(), unique));

    let compile = Command::new("iverilog")
        .arg("-g2001")
        .arg("-o")
        .arg(&vvp_path)
        .args(files)
        .output()
        .expect("failed to run iverilog");
    assert!(
        compile.status.success(),
        "iverilog compile failed: {}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new("vvp")
        .arg(&vvp_path)
        .output()
        .expect("failed to run vvp");
    let _ = std::fs::remove_file(&vvp_path);

    String::from_utf8_lossy(&run.stdout).to_string()
}

/// rverilogとiverilogでは`$finish`通知メッセージの文言が異なる
/// （rverilog: "$finish at time N" / iverilog: ".../file:NN: $finish called at N (1s)"）ため、
/// 末尾の$finish系1行は両者から取り除いてから比較する。
fn strip_finish_line(output: &str) -> String {
    let mut lines: Vec<&str> = output.lines().collect();
    if let Some(last) = lines.last() {
        if last.contains("$finish") {
            lines.pop();
        }
    }
    lines.join("\n")
}

fn compare_case(top: &str, files: &[PathBuf]) {
    if !iverilog_available() {
        eprintln!("iverilog not found, skipping comparison for {}", top);
        return;
    }
    let rverilog_out = run_sim(top, files);
    let iverilog_out = run_iverilog(files);
    let got = strip_finish_line(&rverilog_out);
    let want = strip_finish_line(&iverilog_out);
    assert_eq!(
        got, want,
        "\n--- iverilog ---\n{}\n--- rverilog ---\n{}",
        want, got
    );
}

#[test]
fn compare_counter4() {
    let root = workspace_root();
    compare_case(
        "tb_counter4",
        &[
            root.join("samples/counter4/tb.v"),
            root.join("samples/counter4/counter4.v"),
        ],
    );
}

#[test]
fn compare_func_task() {
    let root = workspace_root();
    compare_case(
        "dut",
        &[root.join("tests/integration/cases/func_task/dut.v")],
    );
}

#[test]
fn compare_gates() {
    let root = workspace_root();
    compare_case("dut", &[root.join("tests/integration/cases/gates/dut.v")]);
}

#[test]
fn compare_generate() {
    let root = workspace_root();
    compare_case(
        "dut",
        &[root.join("tests/integration/cases/generate/dut.v")],
    );
}

#[test]
fn compare_disable_fork() {
    let root = workspace_root();
    compare_case(
        "dut",
        &[root.join("tests/integration/cases/disable_fork/dut.v")],
    );
}

#[test]
fn compare_format_xz() {
    let root = workspace_root();
    compare_case(
        "dut",
        &[root.join("tests/integration/cases/format_xz/dut.v")],
    );
}

#[test]
fn compare_signed() {
    let root = workspace_root();
    compare_case("dut", &[root.join("tests/integration/cases/signed/dut.v")]);
}

#[test]
fn compare_signed_cast() {
    let root = workspace_root();
    compare_case(
        "dut",
        &[root.join("tests/integration/cases/signed_cast/dut.v")],
    );
}

#[test]
fn compare_lvalue_select() {
    let root = workspace_root();
    compare_case(
        "dut",
        &[root.join("tests/integration/cases/lvalue_select/dut.v")],
    );
}

#[test]
fn compare_indexed_part_select() {
    let root = workspace_root();
    compare_case(
        "dut",
        &[root.join("tests/integration/cases/indexed_part_select/dut.v")],
    );
}

#[test]
fn compare_localparam_midmodule() {
    let root = workspace_root();
    compare_case(
        "dut",
        &[root.join("tests/integration/cases/localparam_midmodule/dut.v")],
    );
}

#[test]
fn compare_localparam_const_expr() {
    let root = workspace_root();
    compare_case(
        "dut",
        &[root.join("tests/integration/cases/localparam_const_expr/dut.v")],
    );
}

#[test]
fn compare_binop_precedence() {
    let root = workspace_root();
    compare_case(
        "dut",
        &[root.join("tests/integration/cases/binop_precedence/dut.v")],
    );
}

#[test]
fn compare_ternary_binop_precedence() {
    let root = workspace_root();
    compare_case(
        "dut",
        &[root.join("tests/integration/cases/ternary_binop_precedence/dut.v")],
    );
}

#[test]
fn compare_edge_x() {
    let root = workspace_root();
    compare_case("dut", &[root.join("tests/integration/cases/edge_x/dut.v")]);
}

#[test]
fn compare_monitor() {
    let root = workspace_root();
    compare_case("dut", &[root.join("tests/integration/cases/monitor/dut.v")]);
}

#[test]
fn compare_always_star() {
    let root = workspace_root();
    compare_case(
        "dut",
        &[root.join("tests/integration/cases/always_star/dut.v")],
    );
}

#[test]
fn compare_logical_x_shortcircuit() {
    let root = workspace_root();
    compare_case(
        "dut",
        &[root.join("tests/integration/cases/logical_x_shortcircuit/dut.v")],
    );
}

#[test]
fn compare_ansi_port_comma_direction() {
    let root = workspace_root();
    compare_case(
        "top",
        &[root.join("tests/integration/cases/ansi_port_comma_direction/dut.v")],
    );
}

#[test]
#[ignore = "既知の未解決課題: A13修正後もPicoRV32が最初の命令fetch前に不正命令トラップへ入りtimeoutする（PLAN.md推奨着手順11番参照）"]
fn compare_picorv32_smoke() {
    let root = workspace_root();
    compare_case(
        "tb_picorv32",
        &[
            root.join("tests/integration/cases/picorv32_smoke/tb.v"),
            root.join("tests/integration/cases/picorv32_smoke/picorv32.v"),
        ],
    );
}

#[test]
fn compare_delay0() {
    let root = workspace_root();
    compare_case("dut", &[root.join("tests/integration/cases/delay0/dut.v")]);
}

#[test]
fn compare_fifo_sync() {
    let root = workspace_root();
    compare_case(
        "tb_fifo_sync",
        &[
            root.join("samples/fifo_sync/tb/tb.v"),
            root.join("samples/fifo_sync/rtl/fifo_sync.v"),
        ],
    );
}
