mod common;
use common::{expected_stdout, run_sim, workspace_root};
use std::sync::atomic::{AtomicU64, Ordering};

fn fileio_path(label: &str, ext: &str) -> std::path::PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "rverilog_fileio_{}_{}_{}.{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed),
        label,
        ext
    ))
}

fn fileio_source(name: &str, source: String) -> std::path::PathBuf {
    let path = fileio_path(name, "v");
    std::fs::write(&path, source).expect("一時Verilogソースの作成");
    path
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
fn test_ternary_binop_precedence() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/ternary_binop_precedence/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("ternary_binop_precedence");
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
fn test_mem_write_sensitivity() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/mem_write_sensitivity/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("mem_write_sensitivity");
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
fn test_casez_casex() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/casez_casex/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("casez_casex");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_loop_stmts() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/loop_stmts/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("loop_stmts");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_wide_muldiv() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/wide_muldiv/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("wide_muldiv");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_wide_display() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/wide_display/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("wide_display");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_wide_partial_write() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/wide_partial_write/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("wide_partial_write");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_tristate_bus() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/tristate_bus/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("tristate_bus");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_inout_port() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/inout_port/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("inout_port");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_net_types() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/net_types/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("net_types");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_enable_gates() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/enable_gates/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("enable_gates");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_mos_switches() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/mos_switches/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("mos_switches");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_net_ports_pulls_concat() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/net_ports_pulls_concat/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("net_ports_pulls_concat");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

// iverilogがtriregを未対応のため、期待値はIEEE 1364の規則から手で導出（比較テストなし）
#[test]
fn test_trireg_net() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/trireg_net/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("trireg_net");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

// iverilogがcharge strength付きtriregを未対応のため、期待値はIEEE 1364の規則から手で導出。
#[test]
fn test_trireg_charge_strength() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/trireg_charge_strength/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("trireg_charge_strength");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_ternary_x_cond() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/ternary_x_cond/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("ternary_x_cond");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_nonansi_ports() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/nonansi_ports/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("nonansi_ports");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

// `ref` ポートは親の変数を共有する（inoutと同じくネットのaliasで実現）。SystemVerilog構文のためiverilog比較なし。
#[test]
fn test_ref_port() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/ref_port/dut.sv")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("ref_port");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_expr_context_width() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/expr_context_width/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("expr_context_width");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_drive_strength() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/drive_strength/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("drive_strength");
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
fn test_reduction_ops() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/reduction_ops/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("reduction_ops");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_x_propagation() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/x_propagation/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("x_propagation");
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
fn test_net_decl_assignment() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/net_decl_assignment/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("net_decl_assignment");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_assign_delay() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/assign_delay/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("assign_delay");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_else_if_chain() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/else_if_chain/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("else_if_chain");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_shift_context_width() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/shift_context_width/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("shift_context_width");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
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

#[test]
fn test_tran_switches() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/tran_switches/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("tran_switches");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_rtran_switches() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/rtran_switches/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("rtran_switches");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_strength_display() {
    let root = workspace_root();
    let files = vec![root.join("tests/integration/cases/strength_display/dut.v")];
    let got = run_sim("dut", &files);
    let expected = expected_stdout("strength_display");
    assert_eq!(
        got, expected,
        "\n--- expected ---\n{}\n--- got ---\n{}",
        expected, got
    );
}

#[test]
fn test_time_expr() {
    let root = workspace_root();
    let got = run_sim(
        "dut",
        &[root.join("tests/integration/cases/time_expr/dut.v")],
    );
    assert_eq!(got, expected_stdout("time_expr"));
}

#[test]
fn test_stop_task() {
    let root = workspace_root();
    let got = run_sim(
        "dut",
        &[root.join("tests/integration/cases/stop_task/dut.v")],
    );
    assert_eq!(got, expected_stdout("stop_task"));
}

#[test]
fn test_strobe() {
    let root = workspace_root();
    let got = run_sim("dut", &[root.join("tests/integration/cases/strobe/dut.v")]);
    assert_eq!(got, expected_stdout("strobe"));
}

#[test]
fn test_display_radix() {
    let root = workspace_root();
    let got = run_sim(
        "dut",
        &[root.join("tests/integration/cases/display_radix/dut.v")],
    );
    assert_eq!(got, expected_stdout("display_radix"));
}

#[test]
fn test_monitor_onoff() {
    let root = workspace_root();
    let got = run_sim(
        "dut",
        &[root.join("tests/integration/cases/monitor_onoff/dut.v")],
    );
    assert_eq!(got, expected_stdout("monitor_onoff"));
}

#[test]
fn test_fileio_stdout() {
    let root = workspace_root();
    let got = run_sim(
        "dut",
        &[root.join("tests/integration/cases/fileio_stdout/dut.v")],
    );
    assert_eq!(got, expected_stdout("fileio_stdout"));
}

#[test]
fn test_fileio_files_and_readmemh() {
    let first = fileio_path("first", "txt");
    let second = fileio_path("second", "txt");
    let hex = fileio_path("hex", "txt");
    let source = fileio_source(
        "files",
        format!(
            "module dut; integer a,b,c; reg [7:0] mem [0:0]; initial begin a=$fopen(\"{}\"); b=$fopen(\"{}\"); $fdisplay(a|b, \"mcd\"); c=$fopen(\"{}\", \"w\"); $fwrite(c, \"0a\\n\"); $fflush(c); $readmemh(\"{}\", mem); $display(\"mem=%h\", mem[0]); $finish; end endmodule",
            first.display(), second.display(), hex.display(), hex.display()
        ),
    );
    let output = run_sim("dut", std::slice::from_ref(&source));
    assert!(output.contains("mem=0a"));
    assert_eq!(
        std::fs::read_to_string(&first).expect("最初の出力ファイルを読む"),
        "mcd\n"
    );
    assert_eq!(
        std::fs::read_to_string(&second).expect("二番目の出力ファイルを読む"),
        "mcd\n"
    );
    assert_eq!(
        std::fs::read_to_string(&hex).expect("hex出力ファイルを読む"),
        "0a\n"
    );
    std::fs::remove_file(source).expect("一時Verilogソースの削除");
    std::fs::remove_file(first).expect("最初の出力ファイルの削除");
    std::fs::remove_file(second).expect("二番目の出力ファイルの削除");
    std::fs::remove_file(hex).expect("hex出力ファイルの削除");
}

#[test]
fn test_fileio_descriptor_values_and_invalid_fd() {
    let path = fileio_path("values", "txt");
    let source = fileio_source(
        "values",
        format!(
            "module dut; integer a,b,c,d; initial begin a=$fopen(\"{}\"); b=$fopen(\"{}\", \"w\"); c=$fopen(\"\"); $fclose(a); d=$fopen(\"{}\"); $display(\"%h %h %h %h\", a,b,c,d); $fdisplay(-1, \"bad\"); $finish; end endmodule",
            path.display(), path.display(), path.display()
        ),
    );
    let output = run_sim("dut", std::slice::from_ref(&source));
    assert!(output.contains("00000002 80000003 00000000 00000002"));
    assert!(output.contains("WARNING: invalid file descriptor (0xffffffff) given to $fdisplay()."));
    std::fs::remove_file(source).expect("一時Verilogソースの削除");
    std::fs::remove_file(path).expect("出力ファイルの削除");
}

/// 未オープンのMCDビットを含むマスクは、有効なビット(stdout含む)にも書かず警告のみ。
#[test]
fn test_fileio_mixed_invalid_mcd_writes_nothing() {
    let path = fileio_path("mixedmcd", "txt");
    let source = fileio_source(
        "mixedmcd",
        format!(
            "module dut; integer a; initial begin a=$fopen(\"{}\"); $fdisplay(a|1|32'h40, \"x\"); $fdisplay(0, \"y\"); $finish; end endmodule",
            path.display()
        ),
    );
    let output = run_sim("dut", std::slice::from_ref(&source));
    assert!(output.contains("WARNING: invalid MCD (0x43) given to $fdisplay()."));
    assert!(!output.contains("x\n") && !output.contains("y\n"));
    assert_eq!(
        std::fs::read_to_string(&path).expect("出力ファイルを読む"),
        ""
    );
    std::fs::remove_file(source).expect("一時Verilogソースの削除");
    std::fs::remove_file(path).expect("出力ファイルの削除");
}
