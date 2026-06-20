use std::collections::{HashMap, BinaryHeap};
use std::cmp::Reverse;
use std::path::PathBuf;
use rverilog_mir::*;
use rverilog_vcd::VcdWriter;

// Execution frame: (stmt_list, next_index, block_id of NamedBlock this frame represents, for `disable`)
type Frame = (Vec<StmtId>, usize, Option<u32>);

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
    nba_queue: Vec<(LValue, LogicVal)>,
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

            // Run all active events + cont-assign propagation until quiescent
            loop {
                // Drain active processes and NBA
                loop {
                    let procs: Vec<ProcState> = std::mem::take(&mut self.active);
                    if procs.is_empty() && self.nba_queue.is_empty() { break; }

                    for proc in procs {
                        self.exec_proc(proc);
                        if self.finished { break 'outer; }
                    }

                    // Apply NBA at end of active cycle
                    if !self.nba_queue.is_empty() {
                        let nba: Vec<_> = std::mem::take(&mut self.nba_queue);
                        for (lval, val) in nba {
                            let old = self.get_lval_val(&lval);
                            self.write_lvalue(&lval, val.clone());
                            self.trigger_sensitivity(&lval, old.as_ref(), &val);
                        }
                    }
                }

                // Re-evaluate continuous assigns (may trigger sensitivity and add to active)
                self.eval_conts();

                // If cont assigns triggered new active processes, loop again
                if self.active.is_empty() { break; }
            }

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
                let val = self.eval_expr(expr_id);
                let old = self.get_lval_val(&lval);
                self.write_lvalue(&lval, val.clone());
                self.trigger_sensitivity(&lval, old.as_ref(), &val);
                StepResult::Continue
            }

            Stmt::NbaAssign(lval, expr_id) => {
                let val = self.eval_expr(expr_id);
                self.nba_queue.push((lval, val));
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
                let s = self.format_args(args);
                println!("{}", s);
                self.output_buf.push_str(&s);
                self.output_buf.push('\n');
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
                // $dumpvars — activate VCD recording
                if let Some(path) = self.vcd_path.take() {
                    self.init_vcd_from_path(path);
                }
            }
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
                apply_binop(op, &lv, &rv)
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
        }
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
                let val = self.eval_expr(expr_id);
                let old = self.get_lval_val(&lval);
                self.write_lvalue(&lval, val.clone());
                self.trigger_sensitivity(&lval, old.as_ref(), &val);
            }
            Stmt::Delay(_, body) | Stmt::EventCtl(_, body) => {
                self.exec_sync_stmt(body);
            }
            Stmt::SysCall(task, args) => {
                self.exec_syscall(task, &args);
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

    fn get_lval_val(&mut self, lval: &LValue) -> Option<LogicVal> {
        match lval {
            LValue::Net(id) => self.net_values.get(id).cloned(),
            LValue::BitSelect(id, _) | LValue::PartSelect(id, _, _) | LValue::DynBitSelect(id, _) => {
                self.net_values.get(id).cloned()
            }
            LValue::MemWrite(mem_id, idx_id) => {
                let idx = self.eval_expr(*idx_id).pad_to_width(32) as u32;
                self.mem_values.get(&(mem_id.0, idx)).cloned()
            }
        }
    }

    fn write_lvalue(&mut self, lval: &LValue, val: LogicVal) {
        match lval {
            LValue::Net(id) => {
                let net_id = *id;
                let w = self.design.get_net(net_id).width;
                let val = if val.width() == w { val } else { val.resize(w) };
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
            LValue::MemWrite(mem_id, idx_id) => {
                let idx = self.eval_expr(*idx_id).pad_to_width(32) as u32;
                self.mem_values.insert((mem_id.0, idx), val);
            }
        }
    }

    fn trigger_sensitivity(&mut self, lval: &LValue, old: Option<&LogicVal>, new: &LogicVal) {
        let net_id = match lval {
            LValue::Net(id) | LValue::BitSelect(id, _) | LValue::PartSelect(id, _, _) | LValue::DynBitSelect(id, _) => *id,
            LValue::MemWrite(_, _) => return,  // memory writes don't trigger net sensitivity
        };

        // Determine edge type of the change
        let old_bit = old.map(|v| v.pad_to_width(1) & 1).unwrap_or(0);
        let new_bit = new.pad_to_width(1) & 1;
        let posedge = old_bit == 0 && new_bit == 1;
        let negedge = old_bit == 1 && new_bit == 0;
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
                let val = self.eval_expr(cont.expr);
                let old = self.get_lval_val(&cont.lval);
                if old.as_ref() != Some(&val) {
                    changed = true;
                    let lval = cont.lval.clone();
                    let ov = old;
                    self.write_lvalue(&lval, val.clone());
                    self.trigger_sensitivity(&lval, ov.as_ref(), &val);
                }
            }
            if !changed { break; }
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
            // Skip numeric width/precision modifiers (e.g. %0d, %8d)
            while chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                chars.next();
            }
            let spec = chars.next().unwrap_or('%');
            let val = if arg_idx < args.len() {
                interp.eval_expr(args[arg_idx])
            } else {
                LogicVal::ZERO
            };
            arg_idx += 1;
            match spec {
                'd' | 'D' => result.push_str(&val.pad_to_width(val.width()).to_string()),
                'h' | 'H' => result.push_str(&format!("{:x}", val.pad_to_width(val.width()))),
                'b' | 'B' => result.push_str(&format!("{:b}", val.pad_to_width(val.width()))),
                'o' | 'O' => result.push_str(&format!("{:o}", val.pad_to_width(val.width()))),
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

fn apply_binop(op: BinOp, l: &LogicVal, r: &LogicVal) -> LogicVal {
    match op {
        BinOp::Add => l.add(r),
        BinOp::Sub => l.sub(r),
        BinOp::Mul => l.mul(r),
        BinOp::Div => l.div(r),
        BinOp::Mod => l.mod_(r),
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
        BinOp::Lt => l.lt(r),
        BinOp::Gt => l.gt(r),
        BinOp::Le => l.le(r),
        BinOp::Ge => l.ge(r),
        BinOp::Shl => l.shl(r),
        BinOp::Shr => l.shr(r),
        BinOp::Ashl => l.ashl(r),
        BinOp::Ashr => l.ashr(r),
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
