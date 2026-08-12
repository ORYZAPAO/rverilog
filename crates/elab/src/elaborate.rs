use rverilog_hir::{
    Design, HirModule, Stmt as HirStmt, Expr as HirExpr, LValue as HirLValue,
    Sensitivity as HirSensitivity, SysTask as HirSysTask, CaseKind as HirCaseKind,
    BinOp as HirBinOp, UnOp as HirUnOp, EdgeType as HirEdgeType,
    NetKind as HirNetKind, PortDirection, SysFuncKind,
    GenerateItems, GenerateConstruct,
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

#[derive(Clone)]
struct FunctionInfo {
    args: Vec<NetId>,
    ret: NetId,
    body: StmtId,
}

#[derive(Clone)]
struct TaskInfo {
    params: Vec<(NetId, PortDirection)>,
    body: StmtId,
}

struct ElabCtx<'a> {
    modules: &'a IndexMap<SmolStr, HirModule>,
    nets: Vec<NetInfo>,
    memories: Vec<MemInfo>,
    stmts: Vec<Stmt>,
    exprs: Vec<Expr>,
    /// exprs[i] と対応するsignedness（比較/除算/剰余/算術シフトの符号選択に使用）
    expr_signed: Vec<bool>,
    processes: Vec<Process>,
    conts: Vec<ContAssign>,
    scopes: Vec<Scope>,
    sensitivity_table: IndexMap<u32, Vec<u32>>,
    scope_nets: IndexMap<u32, IndexMap<SmolStr, NetId>>,
    scope_mems: IndexMap<u32, IndexMap<SmolStr, MemId>>,
    scope_params: IndexMap<u32, IndexMap<SmolStr, (u64, u32)>>,
    scope_funcs: IndexMap<u32, IndexMap<SmolStr, FunctionInfo>>,
    scope_tasks: IndexMap<u32, IndexMap<SmolStr, TaskInfo>>,
    scope_blocks: IndexMap<u32, IndexMap<SmolStr, u32>>,
    next_block_id: u32,
}

impl<'a> ElabCtx<'a> {
    fn new(modules: &'a IndexMap<SmolStr, HirModule>) -> Self {
        ElabCtx {
            modules,
            nets: Vec::new(),
            memories: Vec::new(),
            stmts: Vec::new(),
            exprs: Vec::new(),
            expr_signed: Vec::new(),
            processes: Vec::new(),
            conts: Vec::new(),
            scopes: Vec::new(),
            sensitivity_table: IndexMap::new(),
            scope_nets: IndexMap::new(),
            scope_mems: IndexMap::new(),
            scope_params: IndexMap::new(),
            scope_funcs: IndexMap::new(),
            scope_tasks: IndexMap::new(),
            scope_blocks: IndexMap::new(),
            next_block_id: 0,
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
        let signed = self.compute_expr_signed(&expr);
        self.alloc_expr_signed(expr, signed)
    }

    /// signedness を自動算出せず明示的に指定する版。符号無し10進即値や `'s` 基数の
    /// リテラル（`HirExpr::SignedConst`）は MIR 上では通常の `Expr::Const` に落ちる
    /// ため `compute_expr_signed` からは判別できず、ここで明示指定する。
    fn alloc_expr_signed(&mut self, expr: Expr, signed: bool) -> ExprId {
        let id = ExprId(self.exprs.len() as u32);
        self.exprs.push(expr);
        self.expr_signed.push(signed);
        id
    }

    /// 式のsignedness（IEEE 1364-2001 4.5.1 準拠の簡易版）を子の signedness から算出する。
    /// 子 ExprId は必ず自分より先に alloc_expr 済みのため `expr_signed` に既に値がある。
    fn compute_expr_signed(&self, expr: &Expr) -> bool {
        match expr {
            // signed リテラル（8'sh..）は未対応のため常に unsigned 扱い（既知の割り切り）
            Expr::Const(_) => false,
            Expr::Net(id) => self.nets[id.0 as usize].is_signed,
            // ビット選択/部分選択/連結/リピート/メモリ読み出しは self-determined unsigned（IEEE準拠）
            Expr::BitSel(..) | Expr::PartSel(..) | Expr::DynPartSel(..)
            | Expr::Concat(..) | Expr::Repeat(..) | Expr::MemRead(..) => false,
            Expr::StringLit(_) => false,
            Expr::Random(_) => false,
            Expr::CallResult(_, ret_net) => self.nets[ret_net.0 as usize].is_signed,
            Expr::Bin(op, l, r) => {
                let ls = self.expr_signed[l.0 as usize];
                let rs = self.expr_signed[r.0 as usize];
                match op {
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod
                    | BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor
                    | BinOp::BitNand | BinOp::BitNor | BinOp::BitXnor => ls && rs,
                    // シフト量(right)の符号は無視し、左辺のsignednessを結果に伝播する
                    BinOp::Shl | BinOp::Shr | BinOp::Ashl | BinOp::Ashr => ls,
                    // 比較/論理演算の結果は常に1bit unsigned（signed比較の要否は評価時に
                    // 各オペランドの expr_signed を個別参照して判定する）
                    BinOp::LogAnd | BinOp::LogOr | BinOp::Eq | BinOp::Ne
                    | BinOp::CaseEq | BinOp::CaseNe | BinOp::Lt | BinOp::Gt
                    | BinOp::Le | BinOp::Ge => false,
                }
            }
            Expr::Un(op, e) => {
                let es = self.expr_signed[e.0 as usize];
                match op {
                    UnOp::Pos | UnOp::Neg | UnOp::BitNot => es,
                    UnOp::LogNot | UnOp::RedAnd | UnOp::RedNand | UnOp::RedOr
                    | UnOp::RedNor | UnOp::RedXor | UnOp::RedXnor => false,
                }
            }
            Expr::Cond(_, t, f) => {
                self.expr_signed[t.0 as usize] && self.expr_signed[f.0 as usize]
            }
        }
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

    fn register_param(&mut self, scope: ScopeId, name: SmolStr, val: u64, width: u32) {
        self.scope_params
            .entry(scope.0)
            .or_default()
            .insert(name, (val, width));
    }

    /// パラメータ／localparamの値と幅を返す。幅は宣言側のリテラル幅から推定した
    /// もので、複雑な式（三項演算等）の場合は既定の32ビットにフォールバックする
    /// （IEEE context-determined幅推論の完全実装はPLAN.md D節の残タスク）。
    fn resolve_param(&self, scope: ScopeId, name: &str) -> Option<(u64, u32)> {
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

    fn register_func(&mut self, scope: ScopeId, name: SmolStr, info: FunctionInfo) {
        self.scope_funcs.entry(scope.0).or_default().insert(name, info);
    }

    fn resolve_func(&self, scope: ScopeId, name: &str) -> Option<FunctionInfo> {
        let mut cur = Some(scope);
        while let Some(s) = cur {
            if let Some(map) = self.scope_funcs.get(&s.0) {
                if let Some(f) = map.get(name) {
                    return Some(f.clone());
                }
            }
            cur = self.scopes.get(s.0 as usize).and_then(|sc| sc.parent);
        }
        None
    }

    fn register_task(&mut self, scope: ScopeId, name: SmolStr, info: TaskInfo) {
        self.scope_tasks.entry(scope.0).or_default().insert(name, info);
    }

    fn resolve_task(&self, scope: ScopeId, name: &str) -> Option<TaskInfo> {
        let mut cur = Some(scope);
        while let Some(s) = cur {
            if let Some(map) = self.scope_tasks.get(&s.0) {
                if let Some(t) = map.get(name) {
                    return Some(t.clone());
                }
            }
            cur = self.scopes.get(s.0 as usize).and_then(|sc| sc.parent);
        }
        None
    }

    fn register_block(&mut self, scope: ScopeId, name: SmolStr) -> u32 {
        let id = self.next_block_id;
        self.next_block_id += 1;
        self.scope_blocks.entry(scope.0).or_default().insert(name, id);
        id
    }

    fn resolve_block(&self, scope: ScopeId, name: &str) -> Option<u32> {
        let mut cur = Some(scope);
        while let Some(s) = cur {
            if let Some(map) = self.scope_blocks.get(&s.0) {
                if let Some(&id) = map.get(name) {
                    return Some(id);
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

    let mut cont_sensitivity: IndexMap<u32, Vec<u32>> = IndexMap::new();
    for (i, cont) in ctx.conts.iter().enumerate() {
        let mut nets = std::collections::HashSet::new();
        collect_sensitivity_expr(&ctx, cont.expr, &mut nets);
        for net in nets {
            cont_sensitivity.entry(net.0).or_default().push(i as u32);
        }
    }

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
        cont_sensitivity,
        expr_signed: ctx.expr_signed,
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
    let mut param_widths: IndexMap<SmolStr, u32> = IndexMap::new();
    for p in &hir.params {
        param_widths.insert(p.name.clone(), hir_const_width(&p.value));
        match eval_const_hir(ctx, scope, &p.value) {
            Ok(v) => { param_vals.insert(p.name.clone(), v); }
            Err(_) => { param_vals.insert(p.name.clone(), 0); }
        }
    }
    for (name, val) in param_overrides {
        param_vals.insert(name.clone(), *val);
    }
    for lp in &hir.locals {
        param_widths.insert(lp.name.clone(), hir_const_width(&lp.value));
        match eval_const_hir_with(ctx, scope, &lp.value, &param_vals) {
            Ok(v) => { param_vals.insert(lp.name.clone(), v); }
            Err(_) => {}
        }
    }
    for (name, val) in &param_vals {
        let width = param_widths.get(name).copied().unwrap_or(32);
        ctx.register_param(scope, name.clone(), *val, width);
    }

    // Register ports
    for port in &hir.ports {
        let kind = match port.direction {
            PortDirection::Output => NetKind::Reg,
            _ => NetKind::Wire,
        };
        let width = eval_const_hir(ctx, scope, &port.width_expr)
            .map(|v| v as u32).unwrap_or(port.width).max(1);
        let net_id = ctx.alloc_net(NetInfo { width, kind, scope, name: port.name.clone(), is_signed: port.signed });
        ctx.register_net(scope, port.name.clone(), net_id);
    }

    // Register wire nets
    for net in &hir.nets {
        let kind = lower_netkind(net.kind);
        let width = eval_const_hir(ctx, scope, &net.width_expr)
            .map(|v| v as u32).unwrap_or(net.width).max(1);
        let net_id = ctx.alloc_net(NetInfo { width, kind, scope, name: net.name.clone(), is_signed: net.signed });
        ctx.register_net(scope, net.name.clone(), net_id);
    }

    // Register reg declarations
    for reg in &hir.regs {
        let width = eval_const_hir(ctx, scope, &reg.width_expr)
            .map(|v| v as u32).unwrap_or(reg.width).max(1);
        let net_id = ctx.alloc_net(NetInfo { width, kind: NetKind::Reg, scope, name: reg.name.clone(), is_signed: reg.signed });
        ctx.register_net(scope, reg.name.clone(), net_id);
    }

    // Register memory declarations
    for mem in &hir.mems {
        let elem_width = eval_const_hir(ctx, scope, &mem.elem_width).unwrap_or(8) as u32;
        let depth = eval_const_hir(ctx, scope, &mem.depth).unwrap_or(1) as u32;
        let mem_id = ctx.alloc_mem(MemInfo { depth, elem_width, scope, name: mem.name.clone() });
        ctx.register_mem(scope, mem.name.clone(), mem_id);
    }

    // Function declarations
    for func in &hir.functions {
        let func_scope = ctx.alloc_scope(Scope {
            parent: Some(scope),
            name: SmolStr::from(format!("$func_{}", func.name)),
            module_name: hir.name.clone(),
        });
        let ret_w = eval_const_hir(ctx, scope, &func.width_expr)
            .map(|v| v as u32).unwrap_or(func.width).max(1);
        let ret_net = ctx.alloc_net(NetInfo { width: ret_w, kind: NetKind::Reg, scope: func_scope, name: func.name.clone(), is_signed: func.signed });
        ctx.register_net(func_scope, func.name.clone(), ret_net);

        let mut arg_nets = Vec::new();
        for arg in &func.args {
            let w = eval_const_hir(ctx, scope, &arg.width_expr).unwrap_or(1).max(1) as u32;
            let net_id = ctx.alloc_net(NetInfo { width: w, kind: NetKind::Reg, scope: func_scope, name: arg.name.clone(), is_signed: arg.signed });
            ctx.register_net(func_scope, arg.name.clone(), net_id);
            arg_nets.push(net_id);
        }
        for local in &func.locals {
            let w = eval_const_hir(ctx, func_scope, &local.width_expr)
                .map(|v| v as u32).unwrap_or(local.width).max(1);
            let net_id = ctx.alloc_net(NetInfo { width: w, kind: NetKind::Reg, scope: func_scope, name: local.name.clone(), is_signed: local.signed });
            ctx.register_net(func_scope, local.name.clone(), net_id);
        }

        let body_id = lower_stmt(ctx, func_scope, &func.body)?;
        ctx.register_func(scope, func.name.clone(), FunctionInfo { args: arg_nets, ret: ret_net, body: body_id });
    }

    // Task declarations
    for task in &hir.tasks {
        let task_scope = ctx.alloc_scope(Scope {
            parent: Some(scope),
            name: SmolStr::from(format!("$task_{}", task.name)),
            module_name: hir.name.clone(),
        });
        let mut params = Vec::new();
        for arg in &task.args {
            let w = eval_const_hir(ctx, scope, &arg.width_expr).unwrap_or(1).max(1) as u32;
            let net_id = ctx.alloc_net(NetInfo { width: w, kind: NetKind::Reg, scope: task_scope, name: arg.name.clone(), is_signed: arg.signed });
            ctx.register_net(task_scope, arg.name.clone(), net_id);
            params.push((net_id, arg.direction));
        }
        for local in &task.locals {
            let w = eval_const_hir(ctx, task_scope, &local.width_expr)
                .map(|v| v as u32).unwrap_or(local.width).max(1);
            let net_id = ctx.alloc_net(NetInfo { width: w, kind: NetKind::Reg, scope: task_scope, name: local.name.clone(), is_signed: local.signed });
            ctx.register_net(task_scope, local.name.clone(), net_id);
        }

        let body_id = lower_stmt(ctx, task_scope, &task.body)?;
        ctx.register_task(scope, task.name.clone(), TaskInfo { params, body: body_id });
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
            lower_sensitivity(ctx, scope, s, Some(body_id))?
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
                        // child drives parent: parent_lvalue = child_port
                        // (ビット選択 `.o(out[i])` も含め、ポート接続式を lvalue として解決する)
                        if let Some(parent_lv) = expr_as_lvalue(&conn.expr) {
                            let parent_lval = lower_lvalue(ctx, scope, &parent_lv)?;
                            let ce = ctx.alloc_expr(Expr::Net(child_id));
                            ctx.conts.push(ContAssign { lval: parent_lval, expr: ce });
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

    // generate / genvar constructs
    elab_generate_items(ctx, scope, &hir.name, &hir.generates)?;

    Ok(scope)
}

/// `generate`/`genvar` 構築物の展開。`items` 内の宣言・assign・instance 等を
/// 現在の `scope` へ直接登録し、`nested`（if/case/for）は再帰的に展開する。
/// for ループは genvar の値ごとに専用の子スコープを割り当てる（名前衝突回避のため）。
fn elab_generate_items(
    ctx: &mut ElabCtx,
    scope: ScopeId,
    module_name: &SmolStr,
    items: &GenerateItems,
) -> Result<(), ElabError> {
    for lp in &items.locals {
        if let Ok(v) = eval_const_hir(ctx, scope, &lp.value) {
            ctx.register_param(scope, lp.name.clone(), v, hir_const_width(&lp.value));
        }
    }

    for net in &items.nets {
        let kind = lower_netkind(net.kind);
        let width = eval_const_hir(ctx, scope, &net.width_expr)
            .map(|v| v as u32).unwrap_or(net.width).max(1);
        let net_id = ctx.alloc_net(NetInfo { width, kind, scope, name: net.name.clone(), is_signed: net.signed });
        ctx.register_net(scope, net.name.clone(), net_id);
    }

    for reg in &items.regs {
        let width = eval_const_hir(ctx, scope, &reg.width_expr)
            .map(|v| v as u32).unwrap_or(reg.width).max(1);
        let net_id = ctx.alloc_net(NetInfo { width, kind: NetKind::Reg, scope, name: reg.name.clone(), is_signed: reg.signed });
        ctx.register_net(scope, reg.name.clone(), net_id);
    }

    for mem in &items.mems {
        let elem_width = eval_const_hir(ctx, scope, &mem.elem_width).unwrap_or(8) as u32;
        let depth = eval_const_hir(ctx, scope, &mem.depth).unwrap_or(1) as u32;
        let mem_id = ctx.alloc_mem(MemInfo { depth, elem_width, scope, name: mem.name.clone() });
        ctx.register_mem(scope, mem.name.clone(), mem_id);
    }

    for assign in &items.assigns {
        let lval = lower_lvalue(ctx, scope, &assign.lval)?;
        let expr_id = lower_expr(ctx, scope, &assign.expr)?;
        ctx.conts.push(ContAssign { lval, expr: expr_id });
    }

    for init in &items.initials {
        let body_id = lower_stmt(ctx, scope, &init.body)?;
        ctx.processes.push(Process {
            scope,
            body: body_id,
            kind: ProcessKind::Initial,
            sensitivity: Sensitivity::Items(Vec::new()),
        });
    }

    for always in &items.alwayses {
        let body_id = lower_stmt(ctx, scope, &always.body)?;
        let sens = if let Some(s) = &always.sensitivity {
            lower_sensitivity(ctx, scope, s, Some(body_id))?
        } else {
            Sensitivity::Items(Vec::new())
        };

        let proc_id = ProcessId(ctx.processes.len() as u32);
        if let Sensitivity::Items(ref edges) = sens {
            for edge in edges {
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

    for inst in &items.instances {
        let sub = ctx.modules.get(&inst.module)
            .ok_or_else(|| ElabError::ModuleNotFound(inst.module.to_string()))?
            .clone();

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
                        if let Some(parent_lv) = expr_as_lvalue(&conn.expr) {
                            let parent_lval = lower_lvalue(ctx, scope, &parent_lv)?;
                            let ce = ctx.alloc_expr(Expr::Net(child_id));
                            ctx.conts.push(ContAssign { lval: parent_lval, expr: ce });
                        }
                    }
                    _ => {
                        ctx.conts.push(ContAssign { lval: LValue::Net(child_id), expr: parent_expr_id });
                    }
                }
            }
        }
    }

    for construct in &items.nested {
        match construct {
            GenerateConstruct::If(gi) => {
                let cond = eval_const_hir(ctx, scope, &gi.cond).unwrap_or(0);
                let chosen = if cond != 0 { &gi.then_items } else { &gi.else_items };
                elab_generate_items(ctx, scope, module_name, chosen)?;
            }
            GenerateConstruct::Case(gc) => {
                let sel = eval_const_hir(ctx, scope, &gc.sel).unwrap_or(0);
                let mut chosen = None;
                'arms: for (pats, arm_items) in &gc.arms {
                    for p in pats {
                        if eval_const_hir(ctx, scope, p).unwrap_or(0) == sel {
                            chosen = Some(arm_items);
                            break 'arms;
                        }
                    }
                }
                let chosen = chosen.or(gc.default.as_ref());
                if let Some(arm_items) = chosen {
                    elab_generate_items(ctx, scope, module_name, arm_items)?;
                }
            }
            GenerateConstruct::For(gf) => {
                // genvar の値ごとに無限ループ防止の上限を設けつつ展開する
                const MAX_GENERATE_ITERS: u32 = 4096;
                let mut i = eval_const_hir(ctx, scope, &gf.init).unwrap_or(0);
                let mut count = 0u32;
                loop {
                    let iter_scope = ctx.alloc_scope(Scope {
                        parent: Some(scope),
                        name: SmolStr::from(format!("$gen_{}_{}", gf.var, i)),
                        module_name: module_name.clone(),
                    });
                    ctx.register_param(iter_scope, gf.var.clone(), i, 32);
                    if eval_const_hir(ctx, iter_scope, &gf.cond).unwrap_or(0) == 0 {
                        break;
                    }
                    elab_generate_items(ctx, iter_scope, module_name, &gf.body)?;
                    i = eval_const_hir(ctx, iter_scope, &gf.step).unwrap_or(i);
                    count += 1;
                    if count > MAX_GENERATE_ITERS {
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

// ── HIR → MIR lowering ───────────────────────────────────────────────────────

fn lower_expr(ctx: &mut ElabCtx, scope: ScopeId, e: &HirExpr) -> Result<ExprId, ElabError> {
    // signed リテラル（符号無し10進即値 / `'s` 基数）はMIR上ではExpr::Constと同じ
    // 表現になるため、compute_expr_signed による自動判定を経由せずここで明示登録する。
    if let HirExpr::SignedConst(v) = e {
        return Ok(ctx.alloc_expr_signed(Expr::Const(v.clone()), true));
    }
    let mir = match e {
        HirExpr::Const(v) => Expr::Const(v.clone()),
        HirExpr::SignedConst(_) => unreachable!(),
        HirExpr::Net(name) => {
            if let Some(id) = ctx.resolve_net(scope, name.as_str()) {
                Expr::Net(id)
            } else if let Some((val, width)) = ctx.resolve_param(scope, name.as_str()) {
                Expr::Const(LogicVal::new(width as u16, val, 0))
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
        HirExpr::IndexedPartSel(base, idx, width_e, plus_dir) => {
            let net_id = extract_net_id(ctx, scope, base)?;
            let idx_id = lower_expr(ctx, scope, idx)?;
            let width = eval_const_hir(ctx, scope, width_e).unwrap_or(1) as u32;
            Expr::DynPartSel(net_id, idx_id, width, *plus_dir)
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
        HirExpr::SysFunc(SysFuncKind::Random, args) => {
            let seed_id = args.first().map(|a| lower_expr(ctx, scope, a)).transpose()?;
            Expr::Random(seed_id)
        }
        // $signed/$unsigned: inner のトップ Expr を複製し signedness を明示して再登録する
        // （MIR に cast ノードは追加しない。幅は inner と同じ = self-determined）
        HirExpr::SysFunc(kind @ (SysFuncKind::Signed | SysFuncKind::Unsigned), args) => {
            let arg = args.first().ok_or_else(|| ElabError::UnsupportedConstruct(
                "$signed/$unsigned requires exactly 1 argument".into()))?;
            let inner_id = lower_expr(ctx, scope, arg)?;
            let expr = ctx.exprs[inner_id.0 as usize].clone();
            return Ok(ctx.alloc_expr_signed(expr, *kind == SysFuncKind::Signed));
        }
        HirExpr::Call(name, args) => {
            let info = ctx.resolve_func(scope, name.as_str())
                .ok_or_else(|| ElabError::UnresolvedName(name.to_string()))?;
            let mut setup = Vec::new();
            for (&arg_net, arg_e) in info.args.iter().zip(args.iter()) {
                let val_id = lower_expr(ctx, scope, arg_e)?;
                setup.push(ctx.alloc_stmt(Stmt::BlockingAssign(LValue::Net(arg_net), val_id)));
            }
            setup.push(info.body);
            Expr::CallResult(setup, info.ret)
        }
    };
    Ok(ctx.alloc_expr(mir))
}

fn clog2(n: u64) -> u64 {
    if n <= 1 { 0 } else { (64 - (n - 1).leading_zeros()) as u64 }
}

/// インスタンスの出力ポート接続式（`.o(out)` / `.o(out[i])` 等）を代入先 lvalue に変換する。
/// 任意式（連結・演算結果など）は出力ポート接続として不正なので非対応のまま `None` を返す。
fn expr_as_lvalue(e: &HirExpr) -> Option<HirLValue> {
    match e {
        HirExpr::Net(name) => Some(HirLValue::Net(name.clone())),
        HirExpr::IndexSel(name, idx) => Some(HirLValue::IndexSel(name.clone(), idx.clone())),
        HirExpr::PartSel(base, range) => {
            if let HirExpr::Net(name) = base.as_ref() {
                Some(HirLValue::PartSelect(Box::new(HirLValue::Net(name.clone())), (**range).clone()))
            } else {
                None
            }
        }
        _ => None,
    }
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
        HirLValue::IndexedPartSelect(base, idx, width_e, plus_dir) => {
            let base_id = match base.as_ref() {
                HirLValue::Net(n) => ctx.resolve_net(scope, n.as_str())
                    .ok_or_else(|| ElabError::UnresolvedName(n.to_string()))?,
                _ => return Err(ElabError::UnsupportedConstruct("nested lvalue".into())),
            };
            let idx_id = lower_expr(ctx, scope, idx)?;
            let width = eval_const_hir(ctx, scope, width_e).unwrap_or(1) as u32;
            Ok(LValue::DynPartSelect(base_id, idx_id, width, *plus_dir))
        }
        HirLValue::IndexSel(name, idx) => {
            let idx_id = lower_expr(ctx, scope, idx)?;
            if let Some(mem_id) = ctx.resolve_mem(scope, name.as_str()) {
                Ok(LValue::MemWrite(mem_id, idx_id))
            } else if let Some(net_id) = ctx.resolve_net(scope, name.as_str()) {
                // 実行時に決まる動的インデックスでのビット選択（genvar 由来の定数を含む）
                Ok(LValue::DynBitSelect(net_id, idx_id))
            } else {
                Err(ElabError::UnresolvedName(name.to_string()))
            }
        }
        HirLValue::Concat(parts) => {
            let lowered = parts.iter()
                .map(|p| lower_lvalue(ctx, scope, p))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(LValue::Concat(lowered))
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
            let body_id = lower_stmt(ctx, scope, body)?;
            let ms = lower_sensitivity(ctx, scope, sens, Some(body_id))?;
            Stmt::EventCtl(ms, body_id)
        }
        HirStmt::SysCall(task @ (HirSysTask::ReadMemH | HirSysTask::ReadMemB), args) => {
            let path_id = lower_expr(ctx, scope, &args[0])?;
            let mem_id = match args.get(1) {
                Some(HirExpr::Net(name)) => ctx.resolve_mem(scope, name.as_str())
                    .ok_or_else(|| ElabError::UnresolvedName(name.to_string()))?,
                _ => return Err(ElabError::UnsupportedConstruct("readmem target must be a plain memory identifier".into())),
            };
            Stmt::ReadMem(lower_systask(task), path_id, mem_id)
        }
        HirStmt::SysCall(task, args) => {
            let arg_ids: Result<Vec<_>, _> = args.iter().map(|a| lower_expr(ctx, scope, a)).collect();
            Stmt::SysCall(lower_systask(task), arg_ids?)
        }
        HirStmt::For { var, init, cond, step, body } => {
            // Ensure loop variable is declared as a net in this scope
            if ctx.resolve_net(scope, var.as_str()).is_none() {
                let net_id = ctx.alloc_net(NetInfo { width: 32, kind: NetKind::Integer, scope, name: var.clone(), is_signed: true });
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
        HirStmt::TaskCall(name, args) => {
            let info = ctx.resolve_task(scope, name.as_str())
                .ok_or_else(|| ElabError::UnresolvedName(name.to_string()))?;
            let mut block = Vec::new();
            for (i, (param_net, dir)) in info.params.iter().enumerate() {
                if *dir != PortDirection::Output {
                    if let Some(arg_e) = args.get(i) {
                        let val_id = lower_expr(ctx, scope, arg_e)?;
                        block.push(ctx.alloc_stmt(Stmt::BlockingAssign(LValue::Net(*param_net), val_id)));
                    }
                }
            }
            block.push(info.body);
            for (i, (param_net, dir)) in info.params.iter().enumerate() {
                if *dir != PortDirection::Input {
                    if let Some(HirExpr::Net(n)) = args.get(i) {
                        if let Some(target_net) = ctx.resolve_net(scope, n.as_str()) {
                            let ret_expr = ctx.alloc_expr(Expr::Net(*param_net));
                            block.push(ctx.alloc_stmt(Stmt::BlockingAssign(LValue::Net(target_net), ret_expr)));
                        }
                    }
                }
            }
            Stmt::Block(block)
        }
        HirStmt::NamedBlock(name, stmts) => {
            let block_id = ctx.register_block(scope, name.clone());
            let ids: Result<Vec<_>, _> = stmts.iter().map(|s| lower_stmt(ctx, scope, s)).collect();
            Stmt::NamedBlock(block_id, ids?)
        }
        HirStmt::Disable(name) => {
            let block_id = ctx.resolve_block(scope, name.as_str())
                .ok_or_else(|| ElabError::UnresolvedName(name.to_string()))?;
            Stmt::Disable(block_id)
        }
        HirStmt::Fork(branches) => {
            let ids: Result<Vec<_>, _> = branches.iter().map(|s| lower_stmt(ctx, scope, s)).collect();
            Stmt::Fork(ids?)
        }
    };
    Ok(ctx.alloc_stmt(mir))
}

fn lower_sensitivity(
    ctx: &mut ElabCtx,
    scope: ScopeId,
    hs: &HirSensitivity,
    auto_body: Option<StmtId>,
) -> Result<Sensitivity, ElabError> {
    match hs {
        HirSensitivity::All => {
            let Some(body) = auto_body else {
                return Ok(Sensitivity::All);
            };
            let mut nets = std::collections::HashSet::new();
            if !collect_sensitivity_stmt(ctx, body, &mut nets) || nets.is_empty() {
                return Ok(Sensitivity::All);
            }
            Ok(Sensitivity::Items(nets.into_iter().map(|net| SensitivityEdge {
                edge: None,
                net,
            }).collect()))
        }
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

/// `always @*` の本体から、値として参照されるネットを収集する。
/// 依存先を静的に確定できない式では false を返し、呼び出し側で
/// `Sensitivity::All` にフォールバックする。
fn collect_sensitivity_stmt(ctx: &ElabCtx, stmt_id: StmtId, nets: &mut std::collections::HashSet<NetId>) -> bool {
    match &ctx.stmts[stmt_id.0 as usize] {
        Stmt::Block(stmts) | Stmt::NamedBlock(_, stmts) | Stmt::Fork(stmts) => stmts.iter().all(|&stmt| collect_sensitivity_stmt(ctx, stmt, nets)),
        Stmt::If(cond, then_stmt, else_stmt) => collect_sensitivity_expr(ctx, *cond, nets)
            && collect_sensitivity_stmt(ctx, *then_stmt, nets)
            && else_stmt.map_or(true, |stmt| collect_sensitivity_stmt(ctx, stmt, nets)),
        Stmt::Case { sel, arms, default, .. } => collect_sensitivity_expr(ctx, *sel, nets)
            && arms.iter().all(|(patterns, body)| patterns.iter().all(|&expr| collect_sensitivity_expr(ctx, expr, nets))
                && collect_sensitivity_stmt(ctx, *body, nets))
            && default.map_or(true, |stmt| collect_sensitivity_stmt(ctx, stmt, nets)),
        Stmt::BlockingAssign(lval, expr) | Stmt::NbaAssign(lval, expr) => collect_sensitivity_lvalue(ctx, lval, nets)
            && collect_sensitivity_expr(ctx, *expr, nets),
        Stmt::Delay(_, body) | Stmt::EventCtl(_, body) => collect_sensitivity_stmt(ctx, *body, nets),
        Stmt::SysCall(_, args) => args.iter().all(|&expr| collect_sensitivity_expr(ctx, expr, nets)),
        Stmt::While(cond, body) => collect_sensitivity_expr(ctx, *cond, nets) && collect_sensitivity_stmt(ctx, *body, nets),
        Stmt::ReadMem(_, path, _) => collect_sensitivity_expr(ctx, *path, nets),
        Stmt::Null | Stmt::Disable(_) => true,
    }
}

/// 左辺は代入先そのものを感度に含めず、動的な添字式だけを読み出しとして扱う。
fn collect_sensitivity_lvalue(ctx: &ElabCtx, lval: &LValue, nets: &mut std::collections::HashSet<NetId>) -> bool {
    match lval {
        LValue::Net(_) | LValue::BitSelect(_, _) | LValue::PartSelect(_, _, _) => true,
        LValue::DynBitSelect(_, index) | LValue::DynPartSelect(_, index, _, _) | LValue::MemWrite(_, index) => collect_sensitivity_expr(ctx, *index, nets),
        LValue::Concat(parts) => parts.iter().all(|part| collect_sensitivity_lvalue(ctx, part, nets)),
    }
}

fn collect_sensitivity_expr(ctx: &ElabCtx, expr_id: ExprId, nets: &mut std::collections::HashSet<NetId>) -> bool {
    match &ctx.exprs[expr_id.0 as usize] {
        Expr::Const(_) | Expr::StringLit(_) => true,
        Expr::Net(net) | Expr::BitSel(net, _) | Expr::PartSel(net, _, _) | Expr::DynPartSel(net, _, _, _) => {
            nets.insert(*net);
            match &ctx.exprs[expr_id.0 as usize] {
                Expr::BitSel(_, index) | Expr::DynPartSel(_, index, _, _) => collect_sensitivity_expr(ctx, *index, nets),
                _ => true,
            }
        }
        Expr::Concat(parts) => parts.iter().all(|&expr| collect_sensitivity_expr(ctx, expr, nets)),
        Expr::Repeat(_, expr) | Expr::Un(_, expr) => collect_sensitivity_expr(ctx, *expr, nets),
        Expr::Bin(_, lhs, rhs) => collect_sensitivity_expr(ctx, *lhs, nets) && collect_sensitivity_expr(ctx, *rhs, nets),
        Expr::Cond(cond, then_expr, else_expr) => collect_sensitivity_expr(ctx, *cond, nets)
            && collect_sensitivity_expr(ctx, *then_expr, nets)
            && collect_sensitivity_expr(ctx, *else_expr, nets),
        Expr::MemRead(_, index) | Expr::Random(Some(index)) => collect_sensitivity_expr(ctx, *index, nets),
        Expr::Random(None) => true,
        // 関数呼び出しは、lowering済みの引数セットアップ文と関数本体を辿る。
        // 返り値ネット自体は呼び出し内で書かれる左辺なので感度に含めない。
        Expr::CallResult(stmts, _) => stmts.iter().all(|&stmt| collect_sensitivity_stmt(ctx, stmt, nets)),
    }
}

// ── const evaluation ──────────────────────────────────────────────────────────

/// parameter/localparamの宣言側HIR式から幅を推定する。リテラル直書き
/// （`8'b...`等）は自身の幅をそのまま使う。IEEE context-determined幅推論の
/// 完全実装は見送り、それ以外（三項演算・関数呼び出し等）は既定の32ビットに
/// フォールバックする（PLAN.md 実装課題D節「width.rsスタブ化」参照）。
fn hir_const_width(e: &HirExpr) -> u32 {
    match e {
        HirExpr::Const(v) | HirExpr::SignedConst(v) => v.width(),
        _ => 32,
    }
}

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
        HirExpr::Const(v) | HirExpr::SignedConst(v) => {
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
                .map(|(v, _)| v)
                .ok_or_else(|| ElabError::UnresolvedName(name.to_string()))
        }
        HirExpr::Bin(op, lhs, rhs) => {
            let l = eval_const_hir_with(ctx, scope, lhs, extra)?;
            let r = eval_const_hir_with(ctx, scope, rhs, extra)?;
            #[allow(unreachable_patterns)]
            Ok(match op {
                HirBinOp::Add => l.wrapping_add(r),
                HirBinOp::Sub => l.wrapping_sub(r),
                HirBinOp::Mul => l.wrapping_mul(r),
                HirBinOp::Div => if r == 0 { 0 } else { l / r },
                HirBinOp::Mod => if r == 0 { 0 } else { l % r },
                HirBinOp::LogAnd => ((l != 0) && (r != 0)) as u64,
                HirBinOp::LogOr => ((l != 0) || (r != 0)) as u64,
                HirBinOp::Shl | HirBinOp::Ashl => l << (r & 63),
                HirBinOp::Shr | HirBinOp::Ashr => l >> (r & 63),
                HirBinOp::BitAnd => l & r,
                HirBinOp::BitOr => l | r,
                HirBinOp::BitXor => l ^ r,
                HirBinOp::BitNand => !(l & r),
                HirBinOp::BitNor => !(l | r),
                HirBinOp::BitXnor => !(l ^ r),
                HirBinOp::Eq => (l == r) as u64,
                HirBinOp::Ne => (l != r) as u64,
                HirBinOp::CaseEq => (l == r) as u64,
                HirBinOp::CaseNe => (l != r) as u64,
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
        // 定数文脈の $signed/$unsigned は u64 値としては素通し（幅情報を持たないため）
        HirExpr::SysFunc(SysFuncKind::Signed | SysFuncKind::Unsigned, args) => {
            let a = args.first().ok_or_else(|| ElabError::UnsupportedConstruct(
                "$signed/$unsigned requires exactly 1 argument".into()))?;
            eval_const_hir_with(ctx, scope, a, extra)
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
        HirSysTask::ReadMemH => SysTask::ReadMemH,
        HirSysTask::ReadMemB => SysTask::ReadMemB,
    }
}
