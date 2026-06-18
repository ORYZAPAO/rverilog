use rverilog_hir::{
    Design, HirModule, Stmt as HirStmt, Expr as HirExpr, LValue as HirLValue,
    Sensitivity as HirSensitivity, SysTask as HirSysTask, CaseKind as HirCaseKind,
    BinOp as HirBinOp, UnOp as HirUnOp, EdgeType as HirEdgeType,
    NetKind as HirNetKind, PortDirection, SysFuncKind,
};
use rverilog_mir::{
    ElaboratedDesign, NetId, ProcessId, ScopeId, StmtId, ExprId, MemId,
    Process, ProcessKind, NetInfo, MemInfo, NetKind, Scope,
    LValue, Stmt, CaseKind, BinOp, UnOp, SysTask,
    Sensitivity, SensitivityEdge, EdgeType,
    ContAssign, Expr, LogicVal,
};
use indexmap::IndexMap;
use smol_str::SmolStr;
use crate::ElabError;

struct ElabCtx<'a> {
    modules: &'a IndexMap<SmolStr, HirModule>,
    nets: Vec<NetInfo>,
    memories: Vec<MemInfo>,
    stmts: Vec<Stmt>,
    exprs: Vec<Expr>,
    processes: Vec<Process>,
    conts: Vec<ContAssign>,
    scopes: Vec<Scope>,
    sensitivity_table: IndexMap<u32, Vec<u32>>,
    scope_nets: IndexMap<u32, IndexMap<SmolStr, NetId>>,
    scope_mems: IndexMap<u32, IndexMap<SmolStr, MemId>>,
    scope_params: IndexMap<u32, IndexMap<SmolStr, u64>>,
}

impl<'a> ElabCtx<'a> {
    fn new(modules: &'a IndexMap<SmolStr, HirModule>) -> Self {
        ElabCtx {
            modules,
            nets: Vec::new(),
            memories: Vec::new(),
            stmts: Vec::new(),
            exprs: Vec::new(),
            processes: Vec::new(),
            conts: Vec::new(),
            scopes: Vec::new(),
            sensitivity_table: IndexMap::new(),
            scope_nets: IndexMap::new(),
            scope_mems: IndexMap::new(),
            scope_params: IndexMap::new(),
        }
    }

    fn alloc_scope(&mut self, scope: Scope) -> ScopeId {
        let id = ScopeId(self.scopes.len() as u32);
        self.scopes.push(scope);
        id
    }

    fn alloc_net(&mut self, net: NetInfo) -> NetId {
        let id = NetId(self.nets.len() as u32);
        self.nets.push(net);
        id
    }

    fn alloc_stmt(&mut self, stmt: Stmt) -> StmtId {
        let id = StmtId(self.stmts.len() as u32);
        self.stmts.push(stmt);
        id
    }

    fn alloc_expr(&mut self, expr: Expr) -> ExprId {
        let id = ExprId(self.exprs.len() as u32);
        self.exprs.push(expr);
        id
    }

    fn register_net(&mut self, scope: ScopeId, name: SmolStr, net_id: NetId) {
        self.scope_nets
            .entry(scope.0)
            .or_default()
            .insert(name, net_id);
    }

    fn resolve_net(&self, scope: ScopeId, name: &str) -> Option<NetId> {
        let mut cur = Some(scope);
        while let Some(s) = cur {
            if let Some(map) = self.scope_nets.get(&s.0) {
                if let Some(&id) = map.get(name) {
                    return Some(id);
                }
            }
            cur = self.scopes.get(s.0 as usize).and_then(|sc| sc.parent);
        }
        None
    }

    fn register_param(&mut self, scope: ScopeId, name: SmolStr, val: u64) {
        self.scope_params
            .entry(scope.0)
            .or_default()
            .insert(name, val);
    }

    fn resolve_param(&self, scope: ScopeId, name: &str) -> Option<u64> {
        let mut cur = Some(scope);
        while let Some(s) = cur {
            if let Some(map) = self.scope_params.get(&s.0) {
                if let Some(&v) = map.get(name) {
                    return Some(v);
                }
            }
            cur = self.scopes.get(s.0 as usize).and_then(|sc| sc.parent);
        }
        None
    }

    fn alloc_mem(&mut self, mem: MemInfo) -> MemId {
        let id = MemId(self.memories.len() as u32);
        self.memories.push(mem);
        id
    }

    fn register_mem(&mut self, scope: ScopeId, name: SmolStr, mem_id: MemId) {
        self.scope_mems
            .entry(scope.0)
            .or_default()
            .insert(name, mem_id);
    }

    fn resolve_mem(&self, scope: ScopeId, name: &str) -> Option<MemId> {
        let mut cur = Some(scope);
        while let Some(s) = cur {
            if let Some(map) = self.scope_mems.get(&s.0) {
                if let Some(&id) = map.get(name) {
                    return Some(id);
                }
            }
            cur = self.scopes.get(s.0 as usize).and_then(|sc| sc.parent);
        }
        None
    }
}

pub fn elaborate(
    design: &Design,
    top_name: &str,
    _params: &[(String, String)],
) -> Result<ElaboratedDesign, ElabError> {
    let top = design.modules.get(top_name)
        .ok_or_else(|| ElabError::ModuleNotFound(top_name.to_string()))?;

    let mut ctx = ElabCtx::new(&design.modules);

    elab_module(&mut ctx, top, None, SmolStr::from(top_name), &[])?;

    let top_scope = ScopeId(0);
    Ok(ElaboratedDesign {
        nets: ctx.nets,
        memories: ctx.memories,
        stmts: ctx.stmts,
        exprs: ctx.exprs,
        processes: ctx.processes,
        conts: ctx.conts,
        scopes: ctx.scopes,
        top: top_scope,
        sensitivity_table: ctx.sensitivity_table,
    })
}

fn elab_module(
    ctx: &mut ElabCtx,
    hir: &HirModule,
    parent: Option<ScopeId>,
    inst_name: SmolStr,
    param_overrides: &[(SmolStr, u64)],
) -> Result<ScopeId, ElabError> {
    let scope = ctx.alloc_scope(Scope {
        parent,
        name: inst_name,
        module_name: hir.name.clone(),
    });

    // Evaluate parameters (defaults, then overrides)
    let mut param_vals: IndexMap<SmolStr, u64> = IndexMap::new();
    for p in &hir.params {
        match eval_const_hir(ctx, scope, &p.value) {
            Ok(v) => { param_vals.insert(p.name.clone(), v); }
            Err(_) => { param_vals.insert(p.name.clone(), 0); }
        }
    }
    for (name, val) in param_overrides {
        param_vals.insert(name.clone(), *val);
    }
    for lp in &hir.locals {
        match eval_const_hir_with(ctx, scope, &lp.value, &param_vals) {
            Ok(v) => { param_vals.insert(lp.name.clone(), v); }
            Err(_) => {}
        }
    }
    for (name, val) in &param_vals {
        ctx.register_param(scope, name.clone(), *val);
    }

    // Register ports
    for port in &hir.ports {
        let kind = match port.direction {
            PortDirection::Output => NetKind::Reg,
            _ => NetKind::Wire,
        };
        let width = eval_const_hir(ctx, scope, &port.width_expr)
            .map(|v| v as u32).unwrap_or(port.width).max(1);
        let net_id = ctx.alloc_net(NetInfo { width, kind, scope, name: port.name.clone() });
        ctx.register_net(scope, port.name.clone(), net_id);
    }

    // Register wire nets
    for net in &hir.nets {
        let kind = lower_netkind(net.kind);
        let width = eval_const_hir(ctx, scope, &net.width_expr)
            .map(|v| v as u32).unwrap_or(net.width).max(1);
        let net_id = ctx.alloc_net(NetInfo { width, kind, scope, name: net.name.clone() });
        ctx.register_net(scope, net.name.clone(), net_id);
    }

    // Register reg declarations
    for reg in &hir.regs {
        let width = eval_const_hir(ctx, scope, &reg.width_expr)
            .map(|v| v as u32).unwrap_or(reg.width).max(1);
        let net_id = ctx.alloc_net(NetInfo { width, kind: NetKind::Reg, scope, name: reg.name.clone() });
        ctx.register_net(scope, reg.name.clone(), net_id);
    }

    // Register memory declarations
    for mem in &hir.mems {
        let elem_width = eval_const_hir(ctx, scope, &mem.elem_width).unwrap_or(8) as u32;
        let depth = eval_const_hir(ctx, scope, &mem.depth).unwrap_or(1) as u32;
        let mem_id = ctx.alloc_mem(MemInfo { depth, elem_width, scope, name: mem.name.clone() });
        ctx.register_mem(scope, mem.name.clone(), mem_id);
    }

    // Continuous assigns
    for assign in &hir.assigns {
        let lval = lower_lvalue(ctx, scope, &assign.lval)?;
        let expr_id = lower_expr(ctx, scope, &assign.expr)?;
        ctx.conts.push(ContAssign { lval, expr: expr_id });
    }

    // Initial constructs
    for init in &hir.initials {
        let body_id = lower_stmt(ctx, scope, &init.body)?;
        ctx.processes.push(Process {
            scope,
            body: body_id,
            kind: ProcessKind::Initial,
            sensitivity: Sensitivity::Items(Vec::new()),
        });
    }

    // Always constructs
    for always in &hir.alwayses {
        let body_id = lower_stmt(ctx, scope, &always.body)?;
        let sens = if let Some(s) = &always.sensitivity {
            lower_sensitivity(ctx, scope, s)?
        } else {
            Sensitivity::Items(Vec::new())
        };

        let proc_id = ProcessId(ctx.processes.len() as u32);
        if let Sensitivity::Items(ref items) = sens {
            for edge in items {
                ctx.sensitivity_table
                    .entry(edge.net.0)
                    .or_default()
                    .push(proc_id.0);
            }
        }

        ctx.processes.push(Process {
            scope,
            body: body_id,
            kind: ProcessKind::Always,
            sensitivity: sens,
        });
    }

    // Module instances (recursive)
    for inst in &hir.instances {
        let sub = ctx.modules.get(&inst.module)
            .ok_or_else(|| ElabError::ModuleNotFound(inst.module.to_string()))?
            .clone();

        // Resolve parameter overrides for the sub-instance
        let mut sub_params: Vec<(SmolStr, u64)> = Vec::new();
        for (i, po) in inst.params.iter().enumerate() {
            let val = eval_const_hir(ctx, scope, &po.value).unwrap_or(0);
            let name = if let Some(n) = &po.name {
                n.clone()
            } else {
                sub.params.get(i).map(|p| p.name.clone())
                    .unwrap_or_else(|| SmolStr::from(format!("__p{}", i)))
            };
            sub_params.push((name, val));
        }

        let child_scope = elab_module(ctx, &sub, Some(scope), inst.name.clone(), &sub_params)?;

        // Connect ports: build cont assigns between parent nets and child port nets
        for (port_idx, conn) in inst.ports.iter().enumerate() {
            let port_name = if let Some(n) = &conn.name {
                n.clone()
            } else {
                sub.ports.get(port_idx)
                    .map(|p| p.name.clone())
                    .ok_or_else(|| ElabError::UnsupportedConstruct("port index out of range".into()))?
            };

            let port_dir = sub.ports.iter().find(|p| p.name == port_name)
                .or_else(|| sub.ports.get(port_idx))
                .map(|p| p.direction);

            let child_net = ctx.scope_nets.get(&child_scope.0)
                .and_then(|m| m.get(&port_name))
                .copied();

            if let Some(child_id) = child_net {
                let parent_expr_id = lower_expr(ctx, scope, &conn.expr)?;
                match port_dir {
                    Some(PortDirection::Output) => {
                        // child drives parent: parent_net = child_port
                        if let HirExpr::Net(pname) = &conn.expr {
                            if let Some(parent_id) = ctx.resolve_net(scope, pname.as_str()) {
                                let ce = ctx.alloc_expr(Expr::Net(child_id));
                                ctx.conts.push(ContAssign { lval: LValue::Net(parent_id), expr: ce });
                            }
                        }
                    }
                    _ => {
                        // input/inout: parent drives child port
                        ctx.conts.push(ContAssign { lval: LValue::Net(child_id), expr: parent_expr_id });
                    }
                }
            }
        }
    }

    Ok(scope)
}

// ── HIR → MIR lowering ───────────────────────────────────────────────────────

fn lower_expr(ctx: &mut ElabCtx, scope: ScopeId, e: &HirExpr) -> Result<ExprId, ElabError> {
    let mir = match e {
        HirExpr::Const(v) => Expr::Const(v.clone()),
        HirExpr::Net(name) => {
            if let Some(id) = ctx.resolve_net(scope, name.as_str()) {
                Expr::Net(id)
            } else if let Some(val) = ctx.resolve_param(scope, name.as_str()) {
                Expr::Const(LogicVal::new(32, val, 0))
            } else {
                eprintln!("elab warning: unresolved net/param '{}'", name);
                Expr::Const(LogicVal::X)
            }
        }
        HirExpr::BitSel(base, idx) => {
            let net_id = extract_net_id(ctx, scope, base)?;
            let idx_id = lower_expr(ctx, scope, idx)?;
            Expr::BitSel(net_id, idx_id)
        }
        HirExpr::PartSel(base, range) => {
            let net_id = extract_net_id(ctx, scope, base)?;
            let hi = eval_const_hir(ctx, scope, &range.left).unwrap_or(0) as u32;
            let lo = eval_const_hir(ctx, scope, &range.right).unwrap_or(0) as u32;
            Expr::PartSel(net_id, hi, lo)
        }
        HirExpr::Concat(parts) => {
            let ids: Result<Vec<_>, _> = parts.iter().map(|p| lower_expr(ctx, scope, p)).collect();
            Expr::Concat(ids?)
        }
        HirExpr::Repeat(count_e, parts) => {
            let count = eval_const_hir(ctx, scope, count_e).unwrap_or(1) as u32;
            let part_ids: Result<Vec<_>, _> = parts.iter().map(|p| lower_expr(ctx, scope, p)).collect();
            let part_ids = part_ids?;
            let inner = if part_ids.len() == 1 {
                part_ids[0]
            } else {
                ctx.alloc_expr(Expr::Concat(part_ids))
            };
            Expr::Repeat(count, inner)
        }
        HirExpr::Bin(op, lhs, rhs) => {
            let l = lower_expr(ctx, scope, lhs)?;
            let r = lower_expr(ctx, scope, rhs)?;
            Expr::Bin(lower_binop(*op), l, r)
        }
        HirExpr::Un(op, e2) => {
            let e2 = lower_expr(ctx, scope, e2)?;
            Expr::Un(lower_unop(*op), e2)
        }
        HirExpr::StringLit(s) => Expr::StringLit(s.clone()),
        HirExpr::Cond(c, t, f) => {
            let c = lower_expr(ctx, scope, c)?;
            let t = lower_expr(ctx, scope, t)?;
            let f = lower_expr(ctx, scope, f)?;
            Expr::Cond(c, t, f)
        }
        HirExpr::IndexSel(name, idx) => {
            let idx_id = lower_expr(ctx, scope, idx)?;
            if let Some(mem_id) = ctx.resolve_mem(scope, name.as_str()) {
                Expr::MemRead(mem_id, idx_id)
            } else if let Some(net_id) = ctx.resolve_net(scope, name.as_str()) {
                Expr::BitSel(net_id, idx_id)
            } else {
                eprintln!("elab warning: unresolved index access '{}'", name);
                Expr::Const(LogicVal::X)
            }
        }
        HirExpr::SysFunc(SysFuncKind::Clog2, args) => {
            let v = if args.is_empty() { 0u64 } else {
                eval_const_hir(ctx, scope, &args[0]).unwrap_or(0)
            };
            Expr::Const(LogicVal::new(32, clog2(v), 0))
        }
    };
    Ok(ctx.alloc_expr(mir))
}

fn clog2(n: u64) -> u64 {
    if n <= 1 { 0 } else { (64 - (n - 1).leading_zeros()) as u64 }
}

fn extract_net_id(ctx: &ElabCtx, scope: ScopeId, e: &HirExpr) -> Result<NetId, ElabError> {
    match e {
        HirExpr::Net(name) => ctx.resolve_net(scope, name.as_str())
            .ok_or_else(|| ElabError::UnresolvedName(name.to_string())),
        _ => Err(ElabError::UnsupportedConstruct("non-net as bit/part-select base".into())),
    }
}

fn lower_lvalue(ctx: &mut ElabCtx, scope: ScopeId, lval: &HirLValue) -> Result<LValue, ElabError> {
    match lval {
        HirLValue::Net(name) => {
            let id = ctx.resolve_net(scope, name.as_str())
                .ok_or_else(|| ElabError::UnresolvedName(name.to_string()))?;
            Ok(LValue::Net(id))
        }
        HirLValue::BitSelect(base, bit) => {
            let base_id = match base.as_ref() {
                HirLValue::Net(n) => ctx.resolve_net(scope, n.as_str())
                    .ok_or_else(|| ElabError::UnresolvedName(n.to_string()))?,
                _ => return Err(ElabError::UnsupportedConstruct("nested lvalue".into())),
            };
            Ok(LValue::BitSelect(base_id, *bit))
        }
        HirLValue::PartSelect(base, range) => {
            let base_id = match base.as_ref() {
                HirLValue::Net(n) => ctx.resolve_net(scope, n.as_str())
                    .ok_or_else(|| ElabError::UnresolvedName(n.to_string()))?,
                _ => return Err(ElabError::UnsupportedConstruct("nested lvalue".into())),
            };
            let hi = eval_const_hir(ctx, scope, &range.left).unwrap_or(0) as u32;
            let lo = eval_const_hir(ctx, scope, &range.right).unwrap_or(0) as u32;
            Ok(LValue::PartSelect(base_id, hi, lo))
        }
        HirLValue::IndexSel(name, idx) => {
            let idx_id = lower_expr(ctx, scope, idx)?;
            if let Some(mem_id) = ctx.resolve_mem(scope, name.as_str()) {
                Ok(LValue::MemWrite(mem_id, idx_id))
            } else if let Some(net_id) = ctx.resolve_net(scope, name.as_str()) {
                Ok(LValue::BitSelect(net_id, 0))  // fallback: static bit 0
            } else {
                Err(ElabError::UnresolvedName(name.to_string()))
            }
        }
    }
}

fn lower_stmt(ctx: &mut ElabCtx, scope: ScopeId, s: &HirStmt) -> Result<StmtId, ElabError> {
    let mir = match s {
        HirStmt::Block(stmts) => {
            let ids: Result<Vec<_>, _> = stmts.iter().map(|s| lower_stmt(ctx, scope, s)).collect();
            Stmt::Block(ids?)
        }
        HirStmt::If(cond, then_s, else_s) => {
            let c = lower_expr(ctx, scope, cond)?;
            let t = lower_stmt(ctx, scope, then_s)?;
            let e = match else_s {
                Some(e) => Some(lower_stmt(ctx, scope, e)?),
                None => None,
            };
            Stmt::If(c, t, e)
        }
        HirStmt::Case { sel, arms, default, kind } => {
            let sel_id = lower_expr(ctx, scope, sel)?;
            let mut mir_arms = Vec::new();
            for (patterns, body) in arms {
                let pats: Result<Vec<_>, _> = patterns.iter().map(|p| lower_expr(ctx, scope, p)).collect();
                let body_id = lower_stmt(ctx, scope, body)?;
                mir_arms.push((pats?, body_id));
            }
            let default_id = match default {
                Some(d) => Some(lower_stmt(ctx, scope, d)?),
                None => None,
            };
            Stmt::Case { sel: sel_id, arms: mir_arms, default: default_id, kind: lower_casekind(*kind) }
        }
        HirStmt::BlockingAssign(lval, expr) => {
            let lv = lower_lvalue(ctx, scope, lval)?;
            let ex = lower_expr(ctx, scope, expr)?;
            Stmt::BlockingAssign(lv, ex)
        }
        HirStmt::NbaAssign(lval, expr) => {
            let lv = lower_lvalue(ctx, scope, lval)?;
            let ex = lower_expr(ctx, scope, expr)?;
            Stmt::NbaAssign(lv, ex)
        }
        HirStmt::Delay(time, body) => {
            let body_id = lower_stmt(ctx, scope, body)?;
            Stmt::Delay(*time, body_id)
        }
        HirStmt::EventCtl(sens, body) => {
            let ms = lower_sensitivity(ctx, scope, sens)?;
            let body_id = lower_stmt(ctx, scope, body)?;
            Stmt::EventCtl(ms, body_id)
        }
        HirStmt::SysCall(task, args) => {
            let arg_ids: Result<Vec<_>, _> = args.iter().map(|a| lower_expr(ctx, scope, a)).collect();
            Stmt::SysCall(lower_systask(task), arg_ids?)
        }
        HirStmt::For { var, init, cond, step, body } => {
            // Ensure loop variable is declared as a net in this scope
            if ctx.resolve_net(scope, var.as_str()).is_none() {
                let net_id = ctx.alloc_net(NetInfo { width: 32, kind: NetKind::Integer, scope, name: var.clone() });
                ctx.register_net(scope, var.clone(), net_id);
            }
            let init_id = lower_stmt(ctx, scope, init)?;
            let cond_id = lower_expr(ctx, scope, cond)?;
            let body_id = lower_stmt(ctx, scope, body)?;
            let step_id = lower_stmt(ctx, scope, step)?;
            let while_body = ctx.alloc_stmt(Stmt::Block(vec![body_id, step_id]));
            let while_stmt = ctx.alloc_stmt(Stmt::While(cond_id, while_body));
            Stmt::Block(vec![init_id, while_stmt])
        }
    };
    Ok(ctx.alloc_stmt(mir))
}

fn lower_sensitivity(
    ctx: &mut ElabCtx,
    scope: ScopeId,
    hs: &HirSensitivity,
) -> Result<Sensitivity, ElabError> {
    match hs {
        HirSensitivity::All => Ok(Sensitivity::All),
        HirSensitivity::Items(items) => {
            let mut edges = Vec::new();
            for item in items {
                let net = ctx.resolve_net(scope, item.signal.as_str())
                    .ok_or_else(|| ElabError::UnresolvedName(item.signal.to_string()))?;
                let edge = item.edge.map(|e| match e {
                    HirEdgeType::Posedge => EdgeType::Posedge,
                    HirEdgeType::Negedge => EdgeType::Negedge,
                });
                edges.push(SensitivityEdge { edge, net });
            }
            Ok(Sensitivity::Items(edges))
        }
    }
}

// ── const evaluation ──────────────────────────────────────────────────────────

fn eval_const_hir(ctx: &ElabCtx, scope: ScopeId, e: &HirExpr) -> Result<u64, ElabError> {
    eval_const_hir_with(ctx, scope, e, &IndexMap::new())
}

fn eval_const_hir_with(
    ctx: &ElabCtx,
    scope: ScopeId,
    e: &HirExpr,
    extra: &IndexMap<SmolStr, u64>,
) -> Result<u64, ElabError> {
    match e {
        HirExpr::Const(v) => {
            if v.is_known() {
                Ok(v.pad_to_width(v.width()))
            } else {
                Err(ElabError::ParamError("X/Z in const expr".into()))
            }
        }
        HirExpr::Net(name) => {
            if let Some(&v) = extra.get(name.as_str()) {
                return Ok(v);
            }
            ctx.resolve_param(scope, name.as_str())
                .ok_or_else(|| ElabError::UnresolvedName(name.to_string()))
        }
        HirExpr::Bin(op, lhs, rhs) => {
            let l = eval_const_hir_with(ctx, scope, lhs, extra)?;
            let r = eval_const_hir_with(ctx, scope, rhs, extra)?;
            Ok(match op {
                HirBinOp::Add => l.wrapping_add(r),
                HirBinOp::Sub => l.wrapping_sub(r),
                HirBinOp::Mul => l.wrapping_mul(r),
                HirBinOp::Div => if r == 0 { 0 } else { l / r },
                HirBinOp::Mod => if r == 0 { 0 } else { l % r },
                HirBinOp::Shl | HirBinOp::Ashl => l << (r & 63),
                HirBinOp::Shr | HirBinOp::Ashr => l >> (r & 63),
                HirBinOp::BitAnd => l & r,
                HirBinOp::BitOr => l | r,
                HirBinOp::BitXor => l ^ r,
                HirBinOp::Eq => (l == r) as u64,
                HirBinOp::Ne => (l != r) as u64,
                HirBinOp::Lt => (l < r) as u64,
                HirBinOp::Gt => (l > r) as u64,
                HirBinOp::Le => (l <= r) as u64,
                HirBinOp::Ge => (l >= r) as u64,
                _ => return Err(ElabError::UnsupportedConstruct("unsupported op in const expr".into())),
            })
        }
        HirExpr::Un(op, inner) => {
            let v = eval_const_hir_with(ctx, scope, inner, extra)?;
            Ok(match op {
                HirUnOp::Pos => v,
                HirUnOp::Neg => (-(v as i64)) as u64,
                HirUnOp::LogNot => (v == 0) as u64,
                HirUnOp::BitNot => !v,
            })
        }
        HirExpr::Cond(c, t, f) => {
            let cv = eval_const_hir_with(ctx, scope, c, extra)?;
            if cv != 0 {
                eval_const_hir_with(ctx, scope, t, extra)
            } else {
                eval_const_hir_with(ctx, scope, f, extra)
            }
        }
        HirExpr::SysFunc(SysFuncKind::Clog2, args) => {
            let v = if args.is_empty() { 0u64 } else {
                eval_const_hir_with(ctx, scope, &args[0], extra).unwrap_or(0)
            };
            Ok(clog2(v))
        }
        _ => Err(ElabError::UnsupportedConstruct("non-const expression in param context".into())),
    }
}

// ── small converters ──────────────────────────────────────────────────────────

fn lower_netkind(k: HirNetKind) -> NetKind {
    match k {
        HirNetKind::Wire => NetKind::Wire,
        HirNetKind::Reg => NetKind::Reg,
        HirNetKind::Integer => NetKind::Integer,
    }
}

fn lower_binop(op: HirBinOp) -> BinOp {
    match op {
        HirBinOp::Add => BinOp::Add,
        HirBinOp::Sub => BinOp::Sub,
        HirBinOp::Mul => BinOp::Mul,
        HirBinOp::Div => BinOp::Div,
        HirBinOp::Mod => BinOp::Mod,
        HirBinOp::LogAnd => BinOp::LogAnd,
        HirBinOp::LogOr => BinOp::LogOr,
        HirBinOp::BitAnd => BinOp::BitAnd,
        HirBinOp::BitOr => BinOp::BitOr,
        HirBinOp::BitXor => BinOp::BitXor,
        HirBinOp::BitNand => BinOp::BitNand,
        HirBinOp::BitNor => BinOp::BitNor,
        HirBinOp::BitXnor => BinOp::BitXnor,
        HirBinOp::Eq => BinOp::Eq,
        HirBinOp::Ne => BinOp::Ne,
        HirBinOp::CaseEq => BinOp::CaseEq,
        HirBinOp::CaseNe => BinOp::CaseNe,
        HirBinOp::Lt => BinOp::Lt,
        HirBinOp::Gt => BinOp::Gt,
        HirBinOp::Le => BinOp::Le,
        HirBinOp::Ge => BinOp::Ge,
        HirBinOp::Shl => BinOp::Shl,
        HirBinOp::Shr => BinOp::Shr,
        HirBinOp::Ashl => BinOp::Ashl,
        HirBinOp::Ashr => BinOp::Ashr,
    }
}

fn lower_unop(op: HirUnOp) -> UnOp {
    match op {
        HirUnOp::Pos => UnOp::Pos,
        HirUnOp::Neg => UnOp::Neg,
        HirUnOp::LogNot => UnOp::LogNot,
        HirUnOp::BitNot => UnOp::BitNot,
    }
}

fn lower_casekind(k: HirCaseKind) -> CaseKind {
    match k {
        HirCaseKind::Case => CaseKind::Case,
        HirCaseKind::CaseZ => CaseKind::CaseZ,
        HirCaseKind::CaseX => CaseKind::CaseX,
    }
}

fn lower_systask(t: &HirSysTask) -> SysTask {
    match t {
        HirSysTask::Display => SysTask::Display,
        HirSysTask::Write => SysTask::Write,
        HirSysTask::Monitor => SysTask::Monitor,
        HirSysTask::Finish => SysTask::Finish,
        HirSysTask::Time => SysTask::Time,
        HirSysTask::DumpFile => SysTask::DumpFile,
        HirSysTask::DumpVars => SysTask::DumpVars,
    }
}
