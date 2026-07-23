use std::collections::{HashMap, BinaryHeap};
use std::cmp::Reverse;
use std::path::PathBuf;
use rverilog_mir::*;
use rverilog_vcd::VcdWriter;

// Execution frame: (stmt_list, next_index, block_id of NamedBlock this frame represents, for `disable`)
type Frame = (Vec<StmtId>, usize, Option<u32>);

/// indexed part-select (`base +: width` / `base -: width`) の (hi, lo) を計算する。
/// base が負、または hi < lo となる不正な範囲の場合は None。
fn indexed_part_select_bounds(base: i64, width: u32, plus_dir: bool) -> Option<(i64, i64)> {
    let (hi, lo) = if plus_dir { (base + width as i64 - 1, base) } else { (base, base - width as i64 + 1) };
    if lo < 0 || hi < lo { None } else { Some((hi, lo)) }
}

struct ProcState {
    id: ProcessId,
    kind: ProcessKind,
    frames: Vec<Frame>,
    /// `fork`の分岐として起動された場合のfork ID。完了時にjoinカウンタを減算する。
    fork_ctx: Option<u32>,
}

enum StepResult {
    Continue,
    Done,
    Delay(u64),
    Wait(Sensitivity),
    Finish,
    /// `fork`文を実行した。引数はfork ID。全分岐の完了までこのプロセスを止める。
    ForkJoin(u32),
}

pub struct Interpreter {
    pub design: ElaboratedDesign,
    pub net_values: HashMap<NetId, LogicVal>,
    pub mem_values: HashMap<(u32, u32), LogicVal>,
    now: u64,
    seq: u32,
    active: Vec<ProcState>,
    future: BinaryHeap<Reverse<(u64, u32)>>,
    future_procs: HashMap<u32, ProcState>,
    event_waiters: Vec<(Sensitivity, ProcState)>,
    nba_queue: Vec<(LValue, LogicVal, bool)>,
    finished: bool,
    vcd: Option<VcdWriter>,
    vcd_path: Option<PathBuf>,   // set by $dumpfile
    vcd_active: bool,            // enabled by $dumpvars
    output_buf: String,
    fork_seq: u32,
    /// fork ID → 未完了の分岐数
    fork_remaining: HashMap<u32, u32>,
    /// fork ID → join待ちの親プロセス
    fork_waiters: HashMap<u32, ProcState>,
    /// `$random` 用の固定シードPRNG状態（テストの決定性を保つため固定シード）。
    rng_state: u64,
    /// `$monitor` の現在の登録（IEEE 1364: シミュレーション全体で1つだけアクティブ、
    /// 新しい呼び出しが前の登録を置き換える）。
    monitor_args: Option<Vec<ExprId>>,
    /// 前回monitorが印字した際の監視対象値スナップショット（`args[1..]`の評価結果）。
    monitor_last: Option<Vec<LogicVal>>,
}

impl Interpreter {
    pub fn new(design: ElaboratedDesign) -> Self {
        let mut net_values = HashMap::new();
        for (i, net) in design.nets.iter().enumerate() {
            let x_mask = if net.width >= 64 { u64::MAX } else { (1u64 << net.width) - 1 };
            let init = match net.kind {
                NetKind::Wire => LogicVal::new(net.width as u16, 0, x_mask), // Z
                _ => LogicVal::new(net.width as u16, x_mask, x_mask),        // X
            };
            net_values.insert(NetId(i as u32), init);
        }
        Interpreter {
            design,
            net_values,
            mem_values: HashMap::new(),
            now: 0,
            seq: 0,
            active: Vec::new(),
            future: BinaryHeap::new(),
            future_procs: HashMap::new(),
            event_waiters: Vec::new(),
            nba_queue: Vec::new(),
            finished: false,
            vcd: None,
            vcd_path: None,
            vcd_active: false,
            output_buf: String::new(),
            fork_seq: 0,
            fork_remaining: HashMap::new(),
            fork_waiters: HashMap::new(),
            rng_state: 0x2545_F491_4F6C_DD1D,
            monitor_args: None,
            monitor_last: None,
        }
    }

    pub fn run(&mut self) {
        // Initialize VCD if output path was set before run()
        if let Some(path) = self.vcd_path.take() {
            self.init_vcd_from_path(path);
        }

        // Seed all processes at time 0
        for i in 0..self.design.processes.len() {
            let proc = &self.design.processes[i];
            let state = ProcState {
                id: ProcessId(i as u32),
                kind: proc.kind,
                frames: vec![(vec![proc.body], 0, None)],
                fork_ctx: None,
            };
            // Always with non-empty sensitivity: wait for first event
            let sens = proc.sensitivity.clone();
            if proc.kind == ProcessKind::Always {
                if let Sensitivity::Items(ref items) = sens {
                    if !items.is_empty() {
                        self.event_waiters.push((sens, state));
                        continue;
                    }
                }
            }
            self.active.push(state);
        }

        let mut guard = 0u64;
        'outer: loop {
            if self.finished { break; }
            guard += 1;
            if guard > 50_000_000 {
                eprintln!("sim: iteration limit reached at time {}", self.now);
                break;
            }

            // ACTIVE region: 完全収束するまでdrain（連続代入の伝播含む）
            loop {
                while !self.active.is_empty() {
                    let procs: Vec<ProcState> = std::mem::take(&mut self.active);
                    for proc in procs {
                        self.exec_proc(proc);
                        if self.finished { break 'outer; }
                    }
                }

                // Re-evaluate continuous assigns (may trigger sensitivity and add to active)
                self.eval_conts();

                // If cont assigns triggered new active processes, loop again
                if self.active.is_empty() { break; }
            }

            // INACTIVE region: 同時刻(#0)で待っているプロセスをNBA適用前に再開する
            // （IEEE 1364: active → inactive → NBA の順序）
            let mut moved_inactive = false;
            while let Some(&Reverse((t, _))) = self.future.peek() {
                if t != self.now { break; }
                let Reverse((_, seq)) = self.future.pop().unwrap();
                if let Some(p) = self.future_procs.remove(&seq) {
                    self.active.push(p);
                    moved_inactive = true;
                }
            }
            if moved_inactive { continue 'outer; }

            // NBA region
            if !self.nba_queue.is_empty() {
                let nba: Vec<_> = std::mem::take(&mut self.nba_queue);
                for (lval, val, signed) in nba {
                    let old = self.get_lval_val(&lval);
                    self.write_lvalue(&lval, val.clone(), signed);
                    self.trigger_sensitivity(&lval, old.as_ref(), &val);
                }
                continue 'outer;
            }

            // MONITOR region: active/inactive/NBAが完全収束した時点で1回だけ評価
            self.flush_monitor();

            // Advance to next future event
            if self.future.is_empty() { break; }
            let Reverse((t, seq)) = self.future.pop().unwrap();
            let old_time = self.now;
            self.now = t;
            self.vcd_advance_time(old_time, t);
            if let Some(p) = self.future_procs.remove(&seq) {
                self.active.push(p);
            }
            // Drain same-time events
            while let Some(&Reverse((t2, _))) = self.future.peek() {
                if t2 != self.now { break; }
                let Reverse((_, s2)) = self.future.pop().unwrap();
                if let Some(p) = self.future_procs.remove(&s2) {
                    self.active.push(p);
                }
            }
        }
        self.vcd_finish();
    }

    fn exec_proc(&mut self, mut state: ProcState) {
        loop {
            match self.step(&mut state) {
                StepResult::Continue => {}
                StepResult::Done => {
                    if let Some(fid) = state.fork_ctx {
                        self.fork_branch_done(fid);
                        return;
                    }
                    if state.kind == ProcessKind::Always {
                        let proc = &self.design.processes[state.id.0 as usize];
                        let body = proc.body;
                        let sens = proc.sensitivity.clone();
                        state.frames = vec![(vec![body], 0, None)];
                        if let Sensitivity::Items(ref items) = sens {
                            if !items.is_empty() {
                                self.event_waiters.push((sens, state));
                                return;
                            }
                        }
                        // always without sensitivity: loop immediately
                    } else {
                        return;
                    }
                }
                StepResult::ForkJoin(fid) => {
                    self.fork_waiters.insert(fid, state);
                    return;
                }
                StepResult::Delay(t) => {
                    self.seq += 1;
                    let wake = self.now + t;
                    self.future.push(Reverse((wake, self.seq)));
                    self.future_procs.insert(self.seq, state);
                    return;
                }
                StepResult::Wait(sens) => {
                    self.event_waiters.push((sens, state));
                    return;
                }
                StepResult::Finish => {
                    let msg = format!("$finish at time {}", self.now);
                    println!("{}", msg);
                    self.output_buf.push_str(&msg);
                    self.output_buf.push('\n');
                    self.finished = true;
                    return;
                }
            }
        }
    }

    fn step(&mut self, state: &mut ProcState) -> StepResult {
        // Pop next stmt_id from frame stack
        let stmt_id = loop {
            match state.frames.last_mut() {
                None => return StepResult::Done,
                Some((stmts, idx, _)) => {
                    if *idx < stmts.len() {
                        let id = stmts[*idx];
                        *idx += 1;
                        break id;
                    } else {
                        state.frames.pop();
                    }
                }
            }
        };

        match self.design.get_stmt(stmt_id).clone() {
            Stmt::Null => StepResult::Continue,

            Stmt::Block(stmts) => {
                if !stmts.is_empty() {
                    state.frames.push((stmts, 0, None));
                }
                StepResult::Continue
            }

            Stmt::If(cond_id, then_id, else_id) => {
                let cond = self.eval_expr(cond_id);
                let nonzero = cond.pad_to_width(cond.width()) != 0;
                let known = cond.is_known();
                if known && nonzero {
                    state.frames.push((vec![then_id], 0, None));
                } else if known {
                    if let Some(e) = else_id {
                        state.frames.push((vec![e], 0, None));
                    }
                }
                StepResult::Continue
            }

            Stmt::Case { sel, arms, default, kind } => {
                let sel_val = self.eval_expr(sel);
                let mut matched = false;
                'outer: for (pats, body) in &arms {
                    for &pid in pats {
                        let pv = self.eval_expr(pid);
                        let eq = match kind {
                            CaseKind::Case => {
                                sel_val.case_eq(&pv)
                            }
                            CaseKind::CaseZ | CaseKind::CaseX => {
                                sel_val.case_eq(&pv)
                            }
                        };
                        if eq == LogicVal::ONE {
                            state.frames.push((vec![*body], 0, None));
                            matched = true;
                            break 'outer;
                        }
                    }
                }
                if !matched {
                    if let Some(d) = default {
                        state.frames.push((vec![d], 0, None));
                    }
                }
                StepResult::Continue
            }

            Stmt::BlockingAssign(lval, expr_id) => {
                let signed = self.design.expr_signed[expr_id.0 as usize];
                let val = self.eval_expr(expr_id);
                let old = self.get_lval_val(&lval);
                self.write_lvalue(&lval, val.clone(), signed);
                self.trigger_sensitivity(&lval, old.as_ref(), &val);
                StepResult::Continue
            }

            Stmt::NbaAssign(lval, expr_id) => {
                let signed = self.design.expr_signed[expr_id.0 as usize];
                let val = self.eval_expr(expr_id);
                self.nba_queue.push((lval, val, signed));
                StepResult::Continue
            }

            Stmt::Delay(t, body) => {
                state.frames.push((vec![body], 0, None));
                StepResult::Delay(t)
            }

            Stmt::EventCtl(sens, body) => {
                state.frames.push((vec![body], 0, None));
                StepResult::Wait(sens)
            }

            Stmt::SysCall(task, args) => {
                self.exec_syscall(task, &args);
                if self.finished {
                    return StepResult::Finish;
                }
                StepResult::Continue
            }

            Stmt::ReadMem(task, path_id, mem_id) => {
                self.exec_readmem(task, path_id, mem_id);
                StepResult::Continue
            }

            Stmt::While(cond_id, body_id) => {
                let cond = self.eval_expr(cond_id);
                let nonzero = cond.pad_to_width(cond.width()) != 0;
                if cond.is_known() && nonzero {
                    // Push body followed by this while stmt again
                    state.frames.push((vec![body_id, stmt_id], 0, None));
                }
                StepResult::Continue
            }

            Stmt::NamedBlock(block_id, stmts) => {
                if !stmts.is_empty() {
                    state.frames.push((stmts, 0, Some(block_id)));
                }
                StepResult::Continue
            }

            Stmt::Disable(target) => {
                // 同一プロセスのフレームスタックから対象ブロックを探し、それを含む上の階層を巻き戻す。
                // 他プロセスで実行中のブロック/タスクの中断には対応しない（M2サブセット）。
                if let Some(i) = state.frames.iter().rposition(|(_, _, id)| *id == Some(target)) {
                    state.frames.truncate(i);
                }
                StepResult::Continue
            }

            Stmt::Fork(branches) => {
                if branches.is_empty() {
                    return StepResult::Continue;
                }
                self.fork_seq += 1;
                let fid = self.fork_seq;
                self.fork_remaining.insert(fid, branches.len() as u32);
                for b in branches {
                    self.active.push(ProcState {
                        id: state.id,
                        kind: state.kind,
                        frames: vec![(vec![b], 0, None)],
                        fork_ctx: Some(fid),
                    });
                }
                StepResult::ForkJoin(fid)
            }
        }
    }

    /// fork分岐の完了を記録し、全分岐が終わったらjoin待ちの親プロセスを再開する。
    fn fork_branch_done(&mut self, fid: u32) {
        if let Some(remaining) = self.fork_remaining.get_mut(&fid) {
            *remaining -= 1;
            if *remaining == 0 {
                self.fork_remaining.remove(&fid);
                if let Some(parent) = self.fork_waiters.remove(&fid) {
                    self.active.push(parent);
                }
            }
        }
    }

    fn exec_syscall(&mut self, task: SysTask, args: &[ExprId]) {
        match task {
            SysTask::Display => {
                let s = self.format_args(args);
                println!("{}", s);
                self.output_buf.push_str(&s);
                self.output_buf.push('\n');
            }
            SysTask::Write => {
                let s = self.format_args(args);
                print!("{}", s);
                self.output_buf.push_str(&s);
            }
            SysTask::Monitor => {
                // IEEE 1364: $monitorはシミュレーション全体で1つだけアクティブ。
                // 新しい呼び出しが前の登録を置き換える。ここでは登録のみ行い、
                // 実際の印字はmonitorリージョン（flush_monitor）でリージョン
                // 収束後にまとめて行う。
                self.monitor_args = Some(args.to_vec());
                self.monitor_last = None;
            }
            SysTask::Finish => {
                self.finished = true;
            }
            SysTask::Time => {} // used as expression only
            SysTask::DumpFile => {
                // $dumpfile("path") — store path for $dumpvars to activate
                if let Some(&id) = args.first() {
                    if let Expr::StringLit(path) = self.design.get_expr(id).clone() {
                        self.vcd_path = Some(PathBuf::from(path.as_str()));
                    }
                }
            }
            SysTask::DumpVars => {
                // $dumpvars — activate VCD recording.
                // $dumpfile が未呼び出しの場合、IEEE 1364-2005 §17.2 に従い
                // カレントディレクトリの "dump.vcd" を既定の出力先とする。
                if self.vcd.is_none() {
                    let path = self.vcd_path.take().unwrap_or_else(|| PathBuf::from("dump.vcd"));
                    self.init_vcd_from_path(path);
                }
            }
            // ReadMemH/ReadMemB は elaboration時に Stmt::ReadMem へ変換されるため、
            // ここには到達しない。
            SysTask::ReadMemH | SysTask::ReadMemB => {}
        }
    }

    /// `$readmemh`/`$readmemb` でファイルからメモリ配列を初期化する。
    /// 制約: ファイルパスは文字列リテラルのみ、対象は単純なメモリ識別子のみ。
    fn exec_readmem(&mut self, task: SysTask, path_id: ExprId, mem_id: MemId) {
        let path = match self.design.get_expr(path_id).clone() {
            Expr::StringLit(s) => s,
            _ => {
                eprintln!("$readmem: file path must be a string literal");
                return;
            }
        };
        let content = match std::fs::read_to_string(path.as_str()) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("$readmem: failed to read '{}': {}", path, e);
                return;
            }
        };
        let elem_width = self.design.get_mem(mem_id).elem_width;
        let radix = match task {
            SysTask::ReadMemB => 2,
            _ => 16,
        };
        let mut addr: u32 = 0;
        for token in strip_readmem_comments(&content).split_whitespace() {
            if let Some(addr_text) = token.strip_prefix('@') {
                if let Ok(a) = u32::from_str_radix(addr_text, 16) {
                    addr = a;
                }
                continue;
            }
            let val = parse_readmem_token(token, radix, elem_width);
            self.mem_values.insert((mem_id.0, addr), val);
            addr += 1;
        }
    }

    pub fn eval_expr(&mut self, id: ExprId) -> LogicVal {
        match self.design.get_expr(id).clone() {
            Expr::Const(v) => v,
            Expr::StringLit(_) => LogicVal::ZERO,
            Expr::Net(net_id) => self.read_net(net_id),
            Expr::BitSel(net_id, idx_id) => {
                let net_val = self.read_net(net_id);
                let idx = self.eval_expr(idx_id).pad_to_width(32) as u32;
                net_val.bit_select(idx).unwrap_or(LogicVal::X)
            }
            Expr::PartSel(net_id, hi, lo) => {
                let net_val = self.read_net(net_id);
                net_val.part_select(hi, lo).unwrap_or(LogicVal::X)
            }
            Expr::DynPartSel(net_id, base_id, width, plus_dir) => {
                let net_val = self.read_net(net_id);
                let base = self.eval_expr(base_id).pad_to_width(32) as i64;
                match indexed_part_select_bounds(base, width, plus_dir) {
                    Some((hi, lo)) if (hi as u32) < net_val.width() => {
                        net_val.part_select(hi as u32, lo as u32).unwrap_or_else(|_| LogicVal::x_of_width(width))
                    }
                    _ => LogicVal::x_of_width(width),
                }
            }
            Expr::Concat(parts) => {
                if parts.is_empty() { return LogicVal::ZERO; }
                let vals: Vec<LogicVal> = parts.iter().map(|&id| self.eval_expr(id)).collect();
                // parts[0] = MSB
                vals[1..].iter().fold(vals[0].clone(), |acc, v| acc.concat(v))
            }
            Expr::Repeat(count, inner) => {
                if count == 0 { return LogicVal::ZERO; }
                let val = self.eval_expr(inner);
                (1..count).fold(val.clone(), |acc, _| acc.concat(&val))
            }
            Expr::Bin(op, l, r) => {
                let lv = self.eval_expr(l);
                let rv = self.eval_expr(r);
                let l_signed = self.design.expr_signed[l.0 as usize];
                let r_signed = self.design.expr_signed[r.0 as usize];
                apply_binop(op, &lv, &rv, l_signed, r_signed)
            }
            Expr::Un(op, e) => {
                let v = self.eval_expr(e);
                apply_unop(op, &v, self.now)
            }
            Expr::Cond(c, t, f) => {
                let cv = self.eval_expr(c);
                if cv.is_known() && cv.pad_to_width(cv.width()) != 0 {
                    self.eval_expr(t)
                } else if cv.is_zero() {
                    self.eval_expr(f)
                } else {
                    LogicVal::X
                }
            }
            Expr::MemRead(mem_id, idx_id) => {
                let idx = self.eval_expr(idx_id).pad_to_width(32) as u32;
                let w = self.design.get_mem(mem_id).elem_width;
                self.mem_values.get(&(mem_id.0, idx))
                    .cloned()
                    .unwrap_or_else(|| LogicVal::new(w as u16, 0, 0))
            }
            Expr::CallResult(setup, ret_net) => {
                for sid in setup {
                    self.exec_sync_stmt(sid);
                }
                self.read_net(ret_net)
            }
            Expr::Random(seed) => {
                if let Some(seed_id) = seed {
                    let s = self.eval_expr(seed_id).pad_to_width(64);
                    self.rng_state = s;
                }
                let v = self.next_rand_u32();
                LogicVal::new(32, v as u64, 0)
            }
        }
    }

    /// xorshift64* による軽量PRNG。固定シードのため再現性がある
    /// （iverilog等の `$random` とは数値が一致しない）。
    fn next_rand_u32(&mut self) -> u32 {
        let mut x = self.rng_state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng_state = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
    }

    /// Runs a statement to completion without going through the scheduler.
    /// Used for function-call bodies, which Verilog requires to execute in zero time.
    /// `Delay`/`EventCtl` inside such bodies are non-conformant but tolerated by
    /// running their inner statement immediately.
    fn exec_sync_stmt(&mut self, id: StmtId) {
        match self.design.get_stmt(id).clone() {
            Stmt::Null => {}
            Stmt::Block(stmts) => {
                for s in stmts {
                    self.exec_sync_stmt(s);
                }
            }
            Stmt::If(cond_id, then_id, else_id) => {
                let cond = self.eval_expr(cond_id);
                if cond.is_known() && cond.pad_to_width(cond.width()) != 0 {
                    self.exec_sync_stmt(then_id);
                } else if let Some(e) = else_id {
                    self.exec_sync_stmt(e);
                }
            }
            Stmt::Case { sel, arms, default, .. } => {
                let sel_val = self.eval_expr(sel);
                let mut matched = false;
                'outer: for (pats, body) in &arms {
                    for &pid in pats {
                        let pv = self.eval_expr(pid);
                        if sel_val.case_eq(&pv) == LogicVal::ONE {
                            self.exec_sync_stmt(*body);
                            matched = true;
                            break 'outer;
                        }
                    }
                }
                if !matched {
                    if let Some(d) = default {
                        self.exec_sync_stmt(d);
                    }
                }
            }
            Stmt::BlockingAssign(lval, expr_id) | Stmt::NbaAssign(lval, expr_id) => {
                let signed = self.design.expr_signed[expr_id.0 as usize];
                let val = self.eval_expr(expr_id);
                let old = self.get_lval_val(&lval);
                self.write_lvalue(&lval, val.clone(), signed);
                self.trigger_sensitivity(&lval, old.as_ref(), &val);
            }
            Stmt::Delay(_, body) | Stmt::EventCtl(_, body) => {
                self.exec_sync_stmt(body);
            }
            Stmt::SysCall(task, args) => {
                self.exec_syscall(task, &args);
            }
            Stmt::ReadMem(task, path_id, mem_id) => {
                self.exec_readmem(task, path_id, mem_id);
            }
            Stmt::While(cond_id, body_id) => {
                for _ in 0..1_000_000 {
                    let cond = self.eval_expr(cond_id);
                    if !cond.is_known() || cond.pad_to_width(cond.width()) == 0 { break; }
                    self.exec_sync_stmt(body_id);
                }
            }
            Stmt::NamedBlock(_, stmts) => {
                for s in stmts {
                    self.exec_sync_stmt(s);
                }
            }
            // 関数本体内のdisable/forkは未対応のサブセット（zero-time実行のため意味を持たない）。
            Stmt::Disable(_) => {}
            Stmt::Fork(branches) => {
                for b in branches {
                    self.exec_sync_stmt(b);
                }
            }
        }
    }

    fn read_net(&self, id: NetId) -> LogicVal {
        self.net_values.get(&id).cloned().unwrap_or_else(|| {
            let w = self.design.get_net(id).width;
            let xm = if w >= 64 { u64::MAX } else { (1u64 << w) - 1 };
            LogicVal::new(w as u16, xm, xm)
        })
    }

    /// lvalue が指す値の幅（ビット数）。`Concat` の書き込み分割・センシティビティ分割で使う。
    fn lvalue_width(&self, lval: &LValue) -> u32 {
        match lval {
            LValue::Net(id) => self.design.get_net(*id).width,
            LValue::BitSelect(_, _) | LValue::DynBitSelect(_, _) => 1,
            LValue::PartSelect(_, hi, lo) => hi - lo + 1,
            LValue::DynPartSelect(_, _, width, _) => *width,
            LValue::MemWrite(mem_id, _) => self.design.get_mem(*mem_id).elem_width,
            LValue::Concat(parts) => parts.iter().map(|p| self.lvalue_width(p)).sum(),
        }
    }

    fn get_lval_val(&mut self, lval: &LValue) -> Option<LogicVal> {
        match lval {
            LValue::Net(id) => self.net_values.get(id).cloned(),
            LValue::BitSelect(id, _) | LValue::PartSelect(id, _, _) | LValue::DynBitSelect(id, _)
            | LValue::DynPartSelect(id, _, _, _) => {
                self.net_values.get(id).cloned()
            }
            LValue::MemWrite(mem_id, idx_id) => {
                let idx = self.eval_expr(*idx_id).pad_to_width(32) as u32;
                self.mem_values.get(&(mem_id.0, idx)).cloned()
            }
            LValue::Concat(parts) => {
                let vals: Vec<LogicVal> = parts.iter().map(|p| {
                    self.get_lval_val(p).unwrap_or_else(|| {
                        let w = self.lvalue_width(p);
                        let xm = if w >= 64 { u64::MAX } else { (1u64 << w) - 1 };
                        LogicVal::new(w as u16, xm, xm)
                    })
                }).collect();
                Some(vals[1..].iter().fold(vals[0].clone(), |acc, v| acc.concat(v)))
            }
        }
    }

    fn write_lvalue(&mut self, lval: &LValue, val: LogicVal, signed: bool) {
        match lval {
            LValue::Net(id) => {
                let net_id = *id;
                let w = self.design.get_net(net_id).width;
                let val = if val.width() == w {
                    val
                } else if signed {
                    val.extend_sign(w)
                } else {
                    val.resize(w)
                };
                self.net_values.insert(net_id, val.clone());
                self.vcd_record_net_change(net_id, &val);
            }
            LValue::BitSelect(id, bit) => {
                let net_id = *id;
                let w = self.design.get_net(net_id).width;
                let old = self.net_values.get(&net_id).cloned()
                    .unwrap_or_else(|| LogicVal::new(w as u16, 0, 0));
                let oa = old.pad_to_width(w);
                let ob = old.pad_to_width_b(w);
                let ba = val.pad_to_width(1);
                let bb = val.pad_to_width_b(1);
                let m = 1u64 << bit;
                let na = (oa & !m) | (ba << bit);
                let nb = (ob & !m) | (bb << bit);
                let new_val = LogicVal::new(w as u16, na, nb);
                self.net_values.insert(net_id, new_val.clone());
                self.vcd_record_net_change(net_id, &new_val);
            }
            LValue::DynBitSelect(id, idx_id) => {
                let net_id = *id;
                let bit = self.eval_expr(*idx_id).pad_to_width(32) as u32;
                let w = self.design.get_net(net_id).width;
                let old = self.net_values.get(&net_id).cloned()
                    .unwrap_or_else(|| LogicVal::new(w as u16, 0, 0));
                let oa = old.pad_to_width(w);
                let ob = old.pad_to_width_b(w);
                let ba = val.pad_to_width(1);
                let bb = val.pad_to_width_b(1);
                let m = 1u64 << bit;
                let na = (oa & !m) | (ba << bit);
                let nb = (ob & !m) | (bb << bit);
                let new_val = LogicVal::new(w as u16, na, nb);
                self.net_values.insert(net_id, new_val.clone());
                self.vcd_record_net_change(net_id, &new_val);
            }
            LValue::PartSelect(id, hi, lo) => {
                let net_id = *id;
                let w = self.design.get_net(net_id).width;
                let old = self.net_values.get(&net_id).cloned()
                    .unwrap_or_else(|| LogicVal::new(w as u16, 0, 0));
                let oa = old.pad_to_width(w);
                let ob = old.pad_to_width_b(w);
                let sel_w = hi - lo + 1;
                let m = if sel_w >= 64 { u64::MAX } else { (1u64 << sel_w) - 1 };
                let va = val.pad_to_width(sel_w);
                let vb = val.pad_to_width_b(sel_w);
                let na = (oa & !(m << lo)) | (va << lo);
                let nb = (ob & !(m << lo)) | (vb << lo);
                let new_val = LogicVal::new(w as u16, na, nb);
                self.net_values.insert(net_id, new_val.clone());
                self.vcd_record_net_change(net_id, &new_val);
            }
            LValue::DynPartSelect(id, base_id, width, plus_dir) => {
                let net_id = *id;
                let w = self.design.get_net(net_id).width;
                let base = self.eval_expr(*base_id).pad_to_width(32) as i64;
                // 範囲外（負のbase等）は書き込みを無視する。ネット幅を超える場合も
                // 無視する（PartSelect の書き込みと同水準の割り切り、部分的な
                // ビット単位書き込みまではやらない）
                if let Some((hi, lo)) = indexed_part_select_bounds(base, *width, *plus_dir) {
                    if hi >= 0 && (hi as u32) < w {
                        let (hi, lo) = (hi as u32, lo as u32);
                        let old = self.net_values.get(&net_id).cloned()
                            .unwrap_or_else(|| LogicVal::new(w as u16, 0, 0));
                        let oa = old.pad_to_width(w);
                        let ob = old.pad_to_width_b(w);
                        let sel_w = hi - lo + 1;
                        let m = if sel_w >= 64 { u64::MAX } else { (1u64 << sel_w) - 1 };
                        let va = val.pad_to_width(sel_w);
                        let vb = val.pad_to_width_b(sel_w);
                        let na = (oa & !(m << lo)) | (va << lo);
                        let nb = (ob & !(m << lo)) | (vb << lo);
                        let new_val = LogicVal::new(w as u16, na, nb);
                        self.net_values.insert(net_id, new_val.clone());
                        self.vcd_record_net_change(net_id, &new_val);
                    }
                }
            }
            LValue::MemWrite(mem_id, idx_id) => {
                let idx = self.eval_expr(*idx_id).pad_to_width(32) as u32;
                self.mem_values.insert((mem_id.0, idx), val);
            }
            LValue::Concat(parts) => {
                let total_w: u32 = parts.iter().map(|p| self.lvalue_width(p)).sum();
                let full = if signed { val.extend_sign(total_w) } else { val.resize(total_w) };
                let mut hi = total_w;
                for p in parts {
                    let w = self.lvalue_width(p);
                    let lo = hi - w;
                    let slice = full.part_select(hi - 1, lo).unwrap_or(LogicVal::X);
                    self.write_lvalue(p, slice, false);
                    hi = lo;
                }
            }
        }
    }

    fn trigger_sensitivity(&mut self, lval: &LValue, old: Option<&LogicVal>, new: &LogicVal) {
        if let LValue::Concat(parts) = lval {
            let total_w: u32 = parts.iter().map(|p| self.lvalue_width(p)).sum();
            let new_full = new.resize(total_w);
            let old_full = old.map(|o| o.resize(total_w));
            let mut hi = total_w;
            for p in parts {
                let w = self.lvalue_width(p);
                let lo = hi - w;
                let part_new = new_full.part_select(hi - 1, lo).unwrap_or(LogicVal::X);
                let part_old = old_full.as_ref().and_then(|o| o.part_select(hi - 1, lo).ok());
                self.trigger_sensitivity(p, part_old.as_ref(), &part_new);
                hi = lo;
            }
            return;
        }
        let net_id = match lval {
            LValue::Net(id) | LValue::BitSelect(id, _) | LValue::PartSelect(id, _, _) | LValue::DynBitSelect(id, _)
            | LValue::DynPartSelect(id, _, _, _) => *id,
            LValue::MemWrite(_, _) => return,  // memory writes don't trigger net sensitivity
            LValue::Concat(_) => unreachable!("handled by the early return above"),
        };

        // Determine edge type of the change per IEEE 1364: bit 0 is aval, bit 1 is bval
        // (0,0)=0 (1,0)=1 (0,1)=Z (1,1)=X. posedge: 0->1, 0->X, X->1. negedge: 1->0, 1->X, X->0.
        // Z is treated like X for edge detection (matches iverilog behavior).
        fn edge_bits(v: &LogicVal) -> (u64, u64) {
            (v.pad_to_width(1) & 1, v.pad_to_width_b(1) & 1)
        }
        let (old_a, old_b) = old.map(edge_bits).unwrap_or((0, 1)); // no previous value: treat as X
        let (new_a, new_b) = edge_bits(new);
        let old_is_0 = old_a == 0 && old_b == 0;
        let old_is_1 = old_a == 1 && old_b == 0;
        let old_is_unknown = old_b == 1;
        let new_is_1 = new_a == 1 && new_b == 0;
        let new_is_0 = new_a == 0 && new_b == 0;
        let new_is_unknown = new_b == 1;
        let posedge = (old_is_0 && (new_is_1 || new_is_unknown)) || (old_is_unknown && new_is_1);
        let negedge = (old_is_1 && (new_is_0 || new_is_unknown)) || (old_is_unknown && new_is_0);
        let any_change = old.map(|o| o != new).unwrap_or(true);

        let mut to_wake = Vec::new();
        let mut remaining = Vec::new();

        for (sens, proc) in std::mem::take(&mut self.event_waiters) {
            let wake = match &sens {
                Sensitivity::All => any_change,
                Sensitivity::Items(edges) => edges.iter().any(|e| {
                    if e.net != net_id { return false; }
                    match e.edge {
                        None => any_change,
                        Some(EdgeType::Posedge) => posedge,
                        Some(EdgeType::Negedge) => negedge,
                    }
                }),
            };
            if wake {
                to_wake.push(proc);
            } else {
                remaining.push((sens, proc));
            }
        }

        self.event_waiters = remaining;
        self.active.extend(to_wake);
    }

    fn eval_conts(&mut self) {
        for _ in 0..200 {
            let mut changed = false;
            let conts = self.design.conts.clone();
            for cont in &conts {
                let signed = self.design.expr_signed[cont.expr.0 as usize];
                let val = self.eval_expr(cont.expr);
                let old = self.get_lval_val(&cont.lval);
                if old.as_ref() != Some(&val) {
                    changed = true;
                    let lval = cont.lval.clone();
                    let ov = old;
                    self.write_lvalue(&lval, val.clone(), signed);
                    self.trigger_sensitivity(&lval, ov.as_ref(), &val);
                }
            }
            if !changed { break; }
        }
    }

    /// monitorリージョン: active/inactive/NBAが完全収束した後、時刻を進める前に
    /// 1回だけ呼ぶ。登録済みの$monitor引数を評価し、前回印字時から値が変化して
    /// いた場合（初回登録直後を含む）のみ印字する。
    fn flush_monitor(&mut self) {
        let Some(args) = self.monitor_args.clone() else { return; };
        if args.is_empty() { return; }
        let vals: Vec<LogicVal> = args[1..].iter().map(|&id| self.eval_expr(id)).collect();
        if self.monitor_last.as_ref() != Some(&vals) {
            let s = self.format_args(&args);
            println!("{}", s);
            self.output_buf.push_str(&s);
            self.output_buf.push('\n');
            self.monitor_last = Some(vals);
        }
    }

    fn format_args(&mut self, args: &[ExprId]) -> String {
        if args.is_empty() { return String::new(); }
        let first = self.design.get_expr(args[0]).clone();
        if let Expr::StringLit(fmt) = first {
            let now = self.now;
            format_string(&fmt, &args[1..], self, now)
        } else {
            // No format string: space-separated values
            args.iter().map(|&id| self.eval_expr(id).to_string()).collect::<Vec<_>>().join(" ")
        }
    }

    pub fn set_net(&mut self, net: NetId, val: LogicVal) {
        self.net_values.insert(net, val);
    }

    pub fn get_net(&self, net: NetId) -> Option<&LogicVal> {
        self.net_values.get(&net)
    }

    /// Returns the accumulated $display/$write/$finish output since construction.
    pub fn output(&self) -> &str {
        &self.output_buf
    }

    /// Enable VCD output to the given path.  Call before `run()`.
    pub fn set_vcd_output(&mut self, path: PathBuf) {
        self.vcd_path = Some(path);
    }

    fn init_vcd_from_path(&mut self, path: PathBuf) {
        let scopes_owned: Vec<(u32, u32, String)> = self.design.scopes.iter().enumerate()
            .map(|(i, s)| {
                let parent = s.parent.map(|p| p.0).unwrap_or(u32::MAX);
                (i as u32, parent, s.name.to_string())
            })
            .collect();
        let scopes_ref: Vec<(u32, u32, &str)> = scopes_owned.iter()
            .map(|(id, parent, name)| (*id, *parent, name.as_str()))
            .collect();

        let nets_owned: Vec<(u32, u32, String, u32)> = self.design.nets.iter().enumerate()
            .map(|(i, n)| (i as u32, n.scope.0, n.name.to_string(), n.width))
            .collect();
        let nets_ref: Vec<(u32, u32, &str, u32)> = nets_owned.iter()
            .map(|(id, scope, name, width)| (*id, *scope, name.as_str(), *width))
            .collect();

        let root_ids = vec![self.design.top.0];

        match VcdWriter::new(&path, &scopes_ref, &nets_ref, &root_ids) {
            Ok(mut vcd) => {
                let msg = format!("VCD info: dumpfile {} opened for output.", path.display());
                println!("{}", msg);
                self.output_buf.push_str(&msg);
                self.output_buf.push('\n');
                let initial: HashMap<u32, (u64, u64)> = self.net_values.iter()
                    .map(|(id, val)| (id.0, (
                        val.pad_to_width(val.width()),
                        val.pad_to_width_b(val.width()),
                    )))
                    .collect();
                let _ = vcd.dump_initial(&initial);
                self.vcd = Some(vcd);
                self.vcd_active = true;
            }
            Err(e) => {
                eprintln!("VCD: failed to create {:?}: {}", path, e);
            }
        }
    }

    fn vcd_record_net_change(&mut self, net_id: NetId, val: &LogicVal) {
        if !self.vcd_active { return; }
        if let Some(vcd) = &mut self.vcd {
            let aval = val.pad_to_width(val.width());
            let bval = val.pad_to_width_b(val.width());
            vcd.record_change(net_id.0, aval, bval);
        }
    }

    fn vcd_advance_time(&mut self, old_time: u64, new_time: u64) {
        if !self.vcd_active { return; }
        if let Some(vcd) = &mut self.vcd {
            let _ = vcd.advance_time(old_time, new_time);
        }
    }

    fn vcd_finish(&mut self) {
        if let Some(vcd) = &mut self.vcd {
            let _ = vcd.finish();
        }
    }
}

fn format_string(fmt: &str, args: &[ExprId], interp: &mut Interpreter, now: u64) -> String {
    let mut result = String::new();
    let mut arg_idx = 0;
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            // 数字幅修飾子（例: %0d, %8d, %08h）を捕捉する。
            let mut width_digits = String::new();
            while chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                width_digits.push(chars.next().unwrap());
            }
            let spec = chars.next().unwrap_or('%');
            let val = if arg_idx < args.len() {
                interp.eval_expr(args[arg_idx])
            } else {
                LogicVal::ZERO
            };
            let arg_signed = arg_idx < args.len()
                && interp.design.expr_signed[args[arg_idx].0 as usize];
            arg_idx += 1;
            let width = val.width();
            match spec {
                'd' | 'D' | 'h' | 'H' | 'b' | 'B' | 'o' | 'O' => {
                    // 64bit超（Large値）は既存の生表示のまま対象外。
                    if width > 64 {
                        let n = val.pad_to_width(width);
                        result.push_str(&match spec {
                            'd' | 'D' => n.to_string(),
                            'h' | 'H' => format!("{:x}", n),
                            'b' | 'B' => format!("{:b}", n),
                            _ => format!("{:o}", n),
                        });
                    } else {
                        let a = val.pad_to_width(width);
                        let b = val.pad_to_width_b(width);
                        let natural = natural_repr(spec, a, b, width, arg_signed);
                        result.push_str(&apply_width_modifier(spec, width, &width_digits, &natural, arg_signed));
                    }
                }
                's' | 'S' => result.push_str(&val.to_string()),
                't' | 'T' => { arg_idx -= 1; result.push_str(&now.to_string()); }
                '%' => { arg_idx -= 1; result.push('%'); }
                _ => { result.push('%'); result.push(spec); arg_idx -= 1; }
            }
        } else if c == '\\' {
            match chars.next().unwrap_or('\\') {
                'n' => result.push('\n'),
                't' => result.push('\t'),
                '\\' => result.push('\\'),
                '"' => result.push('"'),
                other => { result.push('\\'); result.push(other); }
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// グループ（%bなら1bit、%hなら4bit、%oなら3bit、%dなら値全体）内のa/bビット
/// （マスク済み）から表示文字を決める。known_digitは全bit既知時の数字変換。
/// 既知/不明混在、またはX型/Z型unknown混在は大文字 `X` にフォールバックする
/// （iverilogの実測挙動: 全bit不明かつ単一種別ならx/z小文字、それ以外は大文字）。
fn group_char(a_grp: u64, b_grp: u64, mask: u64, known_digit: impl Fn(u64) -> char) -> char {
    if b_grp == 0 {
        return known_digit(a_grp);
    }
    let unknown_a = a_grp & b_grp; // X型unknownビット
    let has_x = unknown_a != 0;
    let has_z = (b_grp & !unknown_a) != 0; // Z型unknownビット
    let fully_unknown = b_grp == mask;
    match (fully_unknown, has_x, has_z) {
        (true, true, false) => 'x',
        (true, false, true) => 'z',
        (false, true, false) => 'X',
        (false, false, true) => 'Z',
        _ => 'X',
    }
}

/// 幅修飾子を考慮しない、ビット幅由来の桁数で計算した「自然表示」文字列を返す。
/// `signed` は `%d`/`%D` のみで参照し、trueかつMSBが1なら二の補数を負の10進数として表示する
/// （`%h`/`%o`/`%b`はIEEE準拠でビットパターン表示のみ、signednessの影響を受けない）。
fn natural_repr(spec: char, a: u64, b: u64, width: u32, signed: bool) -> String {
    match spec {
        'b' | 'B' => (0..width).rev()
            .map(|bit| group_char((a >> bit) & 1, (b >> bit) & 1, 1, |v| if v == 0 { '0' } else { '1' }))
            .collect(),
        'h' | 'H' => {
            let digits = (((width + 3) / 4).max(1)) as usize;
            (0..digits as u32).rev()
                .map(|i| {
                    let s = i * 4;
                    let mask = (1u64 << (4.min(width - s))) - 1;
                    group_char((a >> s) & mask, (b >> s) & mask, mask, |v| std::char::from_digit(v as u32, 16).unwrap())
                })
                .collect()
        }
        'o' | 'O' => {
            let digits = (((width + 2) / 3).max(1)) as usize;
            (0..digits as u32).rev()
                .map(|i| {
                    let s = i * 3;
                    let mask = (1u64 << (3.min(width - s))) - 1;
                    group_char((a >> s) & mask, (b >> s) & mask, mask, |v| std::char::from_digit(v as u32, 8).unwrap())
                })
                .collect()
        }
        _ => {
            // 'd' | 'D'（パディングなしの最小表示。既定の幅パディングは
            // apply_width_modifier側で行う）
            if b == 0 {
                if signed && width > 0 {
                    let sign_bit = if width >= 64 { (a >> 63) & 1 } else { (a >> (width - 1)) & 1 };
                    if sign_bit == 1 {
                        let signed_val: i64 = if width >= 64 { a as i64 } else { (a as i64) - (1i64 << width) };
                        return signed_val.to_string();
                    }
                }
                a.to_string()
            } else {
                let mask = if width >= 64 { u64::MAX } else { (1u64 << width) - 1 };
                group_char(a, b, mask, |_| unreachable!()).to_string()
            }
        }
    }
}

/// 数字幅修飾子（空文字列=なし、"0"=最小桁数、それ以外=明示幅N）を自然表示文字列に適用する。
/// `%d`のみ、幅修飾子なしの場合にビット幅由来の桁数を空白パディングで補う
/// （`%h`/`%o`/`%b`は`natural_repr`が既にビット幅由来の桁数を生成済みのため不要）。
/// `signed`（iverilog実測挙動）: signed値は符号1桁分を常に確保するため、最大正値
/// `2^(width-1)-1` の桁数+1をフィールド幅とする（unsignedは`2^width-1`の桁数そのまま）。
fn apply_width_modifier(spec: char, width: u32, width_digits: &str, natural: &str, signed: bool) -> String {
    if width_digits.is_empty() {
        if spec == 'd' || spec == 'D' {
            let digits = if signed && width > 0 {
                let max_pos = if width - 1 >= 64 { u64::MAX } else { (1u64 << (width - 1)) - 1 };
                max_pos.to_string().len() + 1
            } else {
                let max = if width >= 64 { u64::MAX } else { (1u64 << width) - 1 };
                max.to_string().len()
            };
            return format!("{:>width$}", natural, width = digits);
        }
        return natural.to_string();
    }
    if width_digits == "0" {
        let trimmed = natural.trim_start_matches('0');
        return if trimmed.is_empty() { "0".to_string() } else { trimmed.to_string() };
    }
    let n: usize = width_digits.parse().unwrap_or(0);
    if natural.len() >= n {
        return natural.to_string();
    }
    if width_digits.starts_with('0') {
        format!("{:0>width$}", natural, width = n)
    } else {
        format!("{:>width$}", natural, width = n)
    }
}

/// `l_signed`/`r_signed` は各オペランドが signed 文脈で評価されたか
/// （`ElaboratedDesign::expr_signed` 由来）。比較/除算/剰余は両辺が signed の
/// ときのみ signed 演算を、`>>>` は左辺の signedness のみを見て演算子を選択する
/// （IEEE 1364-2001 4.5.1: shift 量の符号は結果に影響しない）。
fn apply_binop(op: BinOp, l: &LogicVal, r: &LogicVal, l_signed: bool, r_signed: bool) -> LogicVal {
    let both_signed = l_signed && r_signed;
    // 幅の異なる signed オペランド同士は、演算前に共通の最大幅へ符号拡張して bit pattern を
    // 揃える（例: 4bit -2 + 8bit 3 を zero-extend のまま加算すると値が壊れる）。
    // shift 量（右辺）は self-determined unsigned のため対象外、===/!== は IEEE 上
    // 暗黙のコンテキスト拡張を行わない厳密ビット比較のため対象外。
    let is_shift = matches!(op, BinOp::Shl | BinOp::Shr | BinOp::Ashl | BinOp::Ashr);
    let is_case = matches!(op, BinOp::CaseEq | BinOp::CaseNe);
    let (l_ext, r_ext);
    let (l, r) = if both_signed && !is_shift && !is_case && l.width() != r.width() {
        let w = l.width().max(r.width());
        l_ext = l.extend_sign(w);
        r_ext = r.extend_sign(w);
        (&l_ext, &r_ext)
    } else {
        (l, r)
    };
    match op {
        BinOp::Add => l.add(r),
        BinOp::Sub => l.sub(r),
        BinOp::Mul => l.mul(r),
        BinOp::Div => if both_signed { l.div_signed(r) } else { l.div(r) },
        BinOp::Mod => if both_signed { l.mod_signed(r) } else { l.mod_(r) },
        BinOp::LogAnd => l.log_and(r),
        BinOp::LogOr => l.log_or(r),
        BinOp::BitAnd => l.clone() & r.clone(),
        BinOp::BitOr => l.clone() | r.clone(),
        BinOp::BitXor => l.clone() ^ r.clone(),
        BinOp::BitNand => !(l.clone() & r.clone()),
        BinOp::BitNor => !(l.clone() | r.clone()),
        BinOp::BitXnor => !(l.clone() ^ r.clone()),
        BinOp::Eq => l.eq(r),
        BinOp::Ne => l.ne(r),
        BinOp::CaseEq => l.case_eq(r),
        BinOp::CaseNe => l.case_ne(r),
        BinOp::Lt => if both_signed { l.lt_signed(r) } else { l.lt(r) },
        BinOp::Gt => if both_signed { l.gt_signed(r) } else { l.gt(r) },
        BinOp::Le => if both_signed { l.le_signed(r) } else { l.le(r) },
        BinOp::Ge => if both_signed { l.ge_signed(r) } else { l.ge(r) },
        BinOp::Shl => l.shl(r),
        BinOp::Shr => l.shr(r),
        BinOp::Ashl => l.ashl(r),
        BinOp::Ashr => if l_signed { l.ashr(r) } else { l.shr(r) },
    }
}

fn apply_unop(op: UnOp, v: &LogicVal, _now: u64) -> LogicVal {
    match op {
        UnOp::Pos => v.clone(),
        UnOp::Neg => {
            if v.is_known() {
                let a = v.pad_to_width(v.width());
                LogicVal::new(v.width() as u16, (-(a as i64)) as u64, 0)
            } else {
                LogicVal::X
            }
        }
        UnOp::LogNot => v.log_not(),
        UnOp::BitNot => !v.clone(),
        UnOp::RedAnd => v.reduce_and(),
        UnOp::RedNand => v.reduce_nand(),
        UnOp::RedOr => v.reduce_or(),
        UnOp::RedNor => v.reduce_nor(),
        UnOp::RedXor => v.reduce_xor(),
        UnOp::RedXnor => v.reduce_xnor(),
    }
}

/// `//` 行コメントと `/* */` ブロックコメントを取り除く（`$readmemh`/`$readmemb` ファイル用）。
fn strip_readmem_comments(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '/' && chars.peek() == Some(&'/') {
            while let Some(&n) = chars.peek() {
                if n == '\n' { break; }
                chars.next();
            }
        } else if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            while let Some(n) = chars.next() {
                if n == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// `$readmemh`/`$readmemb` の値トークンを `elem_width` ビットの `LogicVal` にパースする。
/// `radix` は 16（hex）または 2（bin）。`x`/`X`/`z`/`Z` はその桁の全ビットをX/Zにし、
/// `_` は読み飛ばす。
fn parse_readmem_token(token: &str, radix: u32, elem_width: u32) -> LogicVal {
    let bits_per_digit = if radix == 2 { 1 } else { 4 };
    let n_chunks = ((elem_width as usize) + 63) / 64;
    let mut a = vec![0u64; n_chunks];
    let mut b = vec![0u64; n_chunks];
    let mut bitpos: usize = 0;
    for ch in token.chars().filter(|&c| c != '_').rev() {
        if bitpos >= elem_width as usize {
            break;
        }
        // digit_bit(bit) returns (aval, bval) for the given bit position within this digit.
        let digit_bit: Box<dyn Fn(u32) -> (u64, u64)> = match ch {
            'x' | 'X' => Box::new(|_| (1, 1)),
            'z' | 'Z' => Box::new(|_| (0, 1)),
            _ => match ch.to_digit(radix) {
                Some(d) => Box::new(move |bit| (((d as u64) >> bit) & 1, 0)),
                None => continue,
            },
        };
        for bit in 0..bits_per_digit {
            if bitpos >= elem_width as usize {
                break;
            }
            let (av, bv) = digit_bit(bit);
            let chunk = bitpos / 64;
            let off = bitpos % 64;
            if av != 0 {
                a[chunk] |= 1u64 << off;
            }
            if bv != 0 {
                b[chunk] |= 1u64 << off;
            }
            bitpos += 1;
        }
    }
    LogicVal::from_chunks(elem_width, &a, &b)
}

#[cfg(test)]
mod readmem_tests {
    use super::*;

    #[test]
    fn strips_line_and_block_comments() {
        let s = strip_readmem_comments("aa // line\nbb /* block */ cc");
        assert_eq!(s, "aa \nbb  cc");
    }

    #[test]
    fn parses_hex_digits() {
        let v = parse_readmem_token("a5", 16, 8);
        assert_eq!(v.pad_to_width(8), 0xa5);
        assert!(v.is_known());
    }

    #[test]
    fn parses_x_digit_marks_whole_nibble_unknown() {
        let v = parse_readmem_token("x", 16, 4);
        assert!(!v.is_known());
        for bit in 0..4 {
            assert_eq!(v.bit_select(bit).unwrap(), LogicVal::X);
        }
    }

    #[test]
    fn parses_z_digit_marks_whole_nibble_z() {
        let v = parse_readmem_token("z", 16, 4);
        for bit in 0..4 {
            assert_eq!(v.bit_select(bit).unwrap(), LogicVal::Z);
        }
    }

    #[test]
    fn parses_binary_digits() {
        let v = parse_readmem_token("101", 2, 3);
        assert_eq!(v.pad_to_width(3), 0b101);
    }
}

#[cfg(test)]
mod format_tests {
    use super::*;

    const A: u64 = 0b1010_1100; // 8'b1010_1100 = 0xAC = 172
    const FULL: u64 = 0xFF;
    const TOP_NIBBLE_X: u64 = 0xF0; // 上位4bit known(1010)、下位4bit X
    const LOW_NIBBLE: u64 = 0x0F;

    fn repr(spec: char, a: u64, b: u64) -> String {
        apply_width_modifier(spec, 8, "", &natural_repr(spec, a, b, 8, false), false)
    }

    // 既知値（幅修飾子なし）: /tmp/xz.v の iverilog実測"known: d=172 h=ac o=254 b=10101100"と一致。
    #[test]
    fn known_value_default_padding() {
        assert_eq!(repr('d', A, 0), "172");
        assert_eq!(repr('h', A, 0), "ac");
        assert_eq!(repr('o', A, 0), "254");
        assert_eq!(repr('b', A, 0), "10101100");
    }

    // 全bit X: iverilog実測 "allx: d=  x h=xx o=xxx b=xxxxxxxx"
    #[test]
    fn all_x_value() {
        assert_eq!(repr('d', FULL, FULL), "  x");
        assert_eq!(repr('h', FULL, FULL), "xx");
        assert_eq!(repr('o', FULL, FULL), "xxx");
        assert_eq!(repr('b', FULL, FULL), "xxxxxxxx");
    }

    // 全bit Z: iverilog実測 "allz: d=  z h=zz o=zzz b=zzzzzzzz"
    #[test]
    fn all_z_value() {
        assert_eq!(repr('d', 0, FULL), "  z");
        assert_eq!(repr('h', 0, FULL), "zz");
        assert_eq!(repr('o', 0, FULL), "zzz");
        assert_eq!(repr('b', 0, FULL), "zzzzzzzz");
    }

    // 上位nibble既知(1010)・下位nibble全X: iverilog実測 "mixx: d=  X h=ax o=2Xx b=1010xxxx"
    #[test]
    fn partial_x_value() {
        let a = (TOP_NIBBLE_X & A) | LOW_NIBBLE; // 上位4bit=known(1010)、下位4bitはX型(a=b=1)
        let b = LOW_NIBBLE;       // 下位4bitがX
        assert_eq!(repr('d', a, b), "  X");
        assert_eq!(repr('h', a, b), "ax");
        assert_eq!(repr('o', a, b), "2Xx");
        assert_eq!(repr('b', a, b), "1010xxxx");
    }

    // 上位nibble既知(1010)・下位nibble全Z: iverilog実測 "mixz: d=  Z h=az o=2Zz b=1010zzzz"
    #[test]
    fn partial_z_value() {
        let a = TOP_NIBBLE_X & A; // 下位4bitはZなのでa側は0のまま
        let b = LOW_NIBBLE;
        assert_eq!(repr('d', a, b), "  Z");
        assert_eq!(repr('h', a, b), "az");
        assert_eq!(repr('o', a, b), "2Zz");
        assert_eq!(repr('b', a, b), "1010zzzz");
    }

    #[test]
    fn explicit_width_and_zero_modifier() {
        // iverilog実測: "width: d=  172 h=  ac o= 254 b=  10101100"
        assert_eq!(apply_width_modifier('d', 8, "5", &natural_repr('d', A, 0, 8, false), false), "  172");
        assert_eq!(apply_width_modifier('h', 8, "4", &natural_repr('h', A, 0, 8, false), false), "  ac");
        assert_eq!(apply_width_modifier('o', 8, "4", &natural_repr('o', A, 0, 8, false), false), " 254");
        assert_eq!(apply_width_modifier('b', 8, "10", &natural_repr('b', A, 0, 8, false), false), "  10101100");
        // iverilog実測: "zpad: d=00172 h=00ac"
        assert_eq!(apply_width_modifier('d', 8, "05", &natural_repr('d', A, 0, 8, false), false), "00172");
        assert_eq!(apply_width_modifier('h', 8, "04", &natural_repr('h', A, 0, 8, false), false), "00ac");
        // iverilog実測: "zero: d=172 h=ac"（%0d/%0h は最小桁数）
        assert_eq!(apply_width_modifier('d', 8, "0", &natural_repr('d', A, 0, 8, false), false), "172");
        assert_eq!(apply_width_modifier('h', 8, "0", &natural_repr('h', A, 0, 8, false), false), "ac");
    }

    #[test]
    fn signed_decimal_negative_value() {
        // 8bit 0b1010_1100 = 172 (unsigned) / -84 (signed, MSB=1)
        assert_eq!(natural_repr('d', A, 0, 8, true), "-84");
        assert_eq!(natural_repr('d', A, 0, 8, false), "172");
        // %h/%o/%b はsignedでもビットパターン表示のまま変化しない
        assert_eq!(natural_repr('h', A, 0, 8, true), "ac");
    }

    #[test]
    fn signed_decimal_positive_value_unaffected() {
        let positive: u64 = 0b0101_0000; // MSB=0
        assert_eq!(natural_repr('d', positive, 0, 8, true), "80");
    }
}
