use crate::error::FrontendError;
use indexmap::IndexMap;
use rverilog_hir::{
    AlwaysConstruct, BinOp, CaseKind, ContinuousAssign, Design, EdgeType, Expr, FunctionDecl,
    GenerateCase, GenerateConstruct, GenerateFor, GenerateIf, GenerateItems, HirModule,
    InitialConstruct, LValue, LocalParamDecl, MemDecl, ModuleInstance, NetDecl, NetKind, ParamDecl,
    ParamOverride, PortConnection, PortDecl, PortDirection, Range, RegDecl, Sensitivity,
    SensitivityItem, Stmt, SysFuncKind, SysTask, TaskDecl, TfArg, UnOp,
};
use rverilog_mir::LogicVal;
use smol_str::SmolStr;
use std::collections::HashMap;
use std::path::PathBuf;
use sv_parser::{parse_sv, unwrap_node, Define, DefineText, RefNode, SyntaxTree};

// ── helpers ───────────────────────────────────────────────────────────────────

fn lv(v: u64, w: u16) -> LogicVal {
    LogicVal::new(w, v, 0)
}

fn get_id(tree: &SyntaxTree, node: RefNode) -> Option<SmolStr> {
    match unwrap_node!(node, SimpleIdentifier, EscapedIdentifier) {
        Some(RefNode::SimpleIdentifier(x)) => Some(SmolStr::new(tree.get_str(&x.nodes.0)?)),
        Some(RefNode::EscapedIdentifier(x)) => Some(SmolStr::new(tree.get_str(&x.nodes.0)?)),
        _ => None,
    }
}

fn unsupported(what: &str) -> FrontendError {
    FrontendError::UnsupportedConstruct(what.into())
}

// ── public entry point ────────────────────────────────────────────────────────

pub fn parse_files(
    paths: &[PathBuf],
    includes: &[PathBuf],
    defines: &[(String, String)],
) -> Result<Design, FrontendError> {
    let mut design = Design {
        modules: IndexMap::new(),
    };

    let mut defs: HashMap<String, Option<Define>> = HashMap::new();
    for (k, v) in defines {
        defs.insert(
            k.clone(),
            Some(Define::new(
                k.clone(),
                vec![],
                Some(DefineText::new(v.clone(), None)),
            )),
        );
    }
    let inc: Vec<PathBuf> = includes.to_vec();

    for path in paths {
        let (tree, _) = parse_sv(path, &defs, &inc, false, false)
            .map_err(|e| FrontendError::ParseError(format!("{}: {}", path.display(), e)))?;

        // Iterate top-level nodes only (depth-0 module declarations)
        for node in &tree {
            match node {
                RefNode::ModuleDeclarationAnsi(x) => {
                    let m = lower_module_ansi(&tree, x)?;
                    design.modules.insert(m.name.clone(), m);
                }
                RefNode::ModuleDeclarationNonansi(x) => {
                    let m = lower_module_nonansi(&tree, x)?;
                    design.modules.insert(m.name.clone(), m);
                }
                _ => {}
            }
        }
    }
    Ok(design)
}

// ── module lowering ───────────────────────────────────────────────────────────

fn lower_module_ansi(
    tree: &SyntaxTree,
    x: &sv_parser::ModuleDeclarationAnsi,
) -> Result<HirModule, FrontendError> {
    let header = &x.nodes.0;

    let name = get_id(tree, RefNode::ModuleIdentifier(&header.nodes.3))
        .ok_or_else(|| FrontendError::ParseError("missing module name".into()))?;

    let ports = lower_ansi_ports(tree, &header.nodes.6)?;
    let params = lower_param_port_list(tree, &header.nodes.5)?;
    let (
        nets,
        regs,
        mems,
        locals,
        assigns,
        initials,
        alwayses,
        instances,
        functions,
        tasks,
        generates,
    ) = lower_nonport_items(tree, &x.nodes.2)?;

    Ok(HirModule {
        name,
        ports,
        params,
        locals,
        nets,
        regs,
        mems,
        assigns,
        initials,
        alwayses,
        instances,
        functions,
        tasks,
        generates,
    })
}

fn lower_module_nonansi(
    tree: &SyntaxTree,
    x: &sv_parser::ModuleDeclarationNonansi,
) -> Result<HirModule, FrontendError> {
    let header = &x.nodes.0;
    let name = get_id(tree, RefNode::ModuleIdentifier(&header.nodes.3))
        .ok_or_else(|| FrontendError::ParseError("missing module name".into()))?;

    let (
        nets,
        regs,
        mems,
        locals,
        assigns,
        initials,
        alwayses,
        instances,
        functions,
        tasks,
        generates,
    ) = lower_module_items(tree, &x.nodes.2)?;

    Ok(HirModule {
        name,
        ports: vec![],
        params: vec![],
        locals,
        nets,
        regs,
        mems,
        assigns,
        initials,
        alwayses,
        instances,
        functions,
        tasks,
        generates,
    })
}

// ── ports ─────────────────────────────────────────────────────────────────────

fn lower_ansi_ports(
    tree: &SyntaxTree,
    opt: &Option<sv_parser::ListOfPortDeclarations>,
) -> Result<Vec<PortDecl>, FrontendError> {
    let mut ports = Vec::new();
    if let Some(lpd) = opt {
        // Paren<Option<List<Symbol, (Vec<AttributeInstance>, AnsiPortDeclaration)>>>
        // lpd.nodes.0 is Paren, .nodes.1 is the inner Option<List<...>>
        if let Some(list) = &lpd.nodes.0.nodes.1 {
            // list.nodes.0 is first item, list.nodes.1 is rest
            let first = &list.nodes.0;
            // ANSI ポートでヘッダを省略した場合は、直前のポート方向を継承する。
            // 先頭ポートには IEEE 1364-2001 の既定値である input を適用する。
            let mut prev_dir = PortDirection::Input;
            let port = lower_ansi_port(tree, &first.1, prev_dir)?;
            prev_dir = port.direction;
            ports.push(port);
            for (_, item) in &list.nodes.1 {
                let port = lower_ansi_port(tree, &item.1, prev_dir)?;
                prev_dir = port.direction;
                ports.push(port);
            }
        }
    }
    Ok(ports)
}

fn lower_ansi_port(
    tree: &SyntaxTree,
    port: &sv_parser::AnsiPortDeclaration,
    prev_dir: PortDirection,
) -> Result<PortDecl, FrontendError> {
    match port {
        sv_parser::AnsiPortDeclaration::Net(n) => lower_ansi_port_net(tree, n, prev_dir),
        sv_parser::AnsiPortDeclaration::Variable(v) => lower_ansi_port_variable(tree, v, prev_dir),
        sv_parser::AnsiPortDeclaration::Paren(p) => {
            let dir = match &p.nodes.0 {
                Some(d) => lower_port_direction(d),
                None => prev_dir,
            };
            let name = get_id(tree, RefNode::PortIdentifier(&p.nodes.2))
                .ok_or_else(|| FrontendError::ParseError("port name missing".into()))?;
            Ok(PortDecl {
                name,
                direction: dir,
                width: 1,
                width_expr: Expr::Const(lv(1, 32)),
                signed: false,
            })
        }
    }
}

fn lower_ansi_port_net(
    tree: &SyntaxTree,
    x: &sv_parser::AnsiPortDeclarationNet,
    prev_dir: PortDirection,
) -> Result<PortDecl, FrontendError> {
    let dir = if let Some(hdr) = &x.nodes.0 {
        match hdr {
            sv_parser::NetPortHeaderOrInterfacePortHeader::NetPortHeader(h) => h
                .nodes
                .0
                .as_ref()
                .map(lower_port_direction)
                .unwrap_or(prev_dir),
            _ => prev_dir,
        }
    } else {
        prev_dir
    };
    let (width, width_expr) = packed_width_expr(tree, RefNode::AnsiPortDeclarationNet(x));
    let signed = has_signed(RefNode::AnsiPortDeclarationNet(x));
    let name = get_id(tree, RefNode::PortIdentifier(&x.nodes.1))
        .ok_or_else(|| FrontendError::ParseError("port name missing".into()))?;
    Ok(PortDecl {
        name,
        direction: dir,
        width,
        width_expr,
        signed,
    })
}

fn lower_ansi_port_variable(
    tree: &SyntaxTree,
    x: &sv_parser::AnsiPortDeclarationVariable,
    prev_dir: PortDirection,
) -> Result<PortDecl, FrontendError> {
    let dir = if let Some(hdr) = &x.nodes.0 {
        hdr.nodes
            .0
            .as_ref()
            .map(lower_port_direction)
            .unwrap_or(prev_dir)
    } else {
        prev_dir
    };
    let (width, width_expr) = packed_width_expr(tree, RefNode::AnsiPortDeclarationVariable(x));
    let signed = has_signed(RefNode::AnsiPortDeclarationVariable(x));
    let name = get_id(tree, RefNode::PortIdentifier(&x.nodes.1))
        .ok_or_else(|| FrontendError::ParseError("port name missing".into()))?;
    Ok(PortDecl {
        name,
        direction: dir,
        width,
        width_expr,
        signed,
    })
}

fn lower_port_direction(d: &sv_parser::PortDirection) -> PortDirection {
    match d {
        sv_parser::PortDirection::Input(_) => PortDirection::Input,
        sv_parser::PortDirection::Output(_) => PortDirection::Output,
        sv_parser::PortDirection::Inout(_) => PortDirection::Inout,
        sv_parser::PortDirection::Ref(_) => PortDirection::Inout,
    }
}

// ── parameters ────────────────────────────────────────────────────────────────

fn lower_param_port_list(
    tree: &SyntaxTree,
    opt: &Option<sv_parser::ParameterPortList>,
) -> Result<Vec<ParamDecl>, FrontendError> {
    let mut params = Vec::new();
    if let Some(ppl) = opt {
        match ppl {
            sv_parser::ParameterPortList::Assignment(a) => {
                for pa in a.nodes.1.nodes.1 .0.nodes.0.contents() {
                    if let Some(name) = get_id(tree, RefNode::ParameterIdentifier(&pa.nodes.0)) {
                        let value = if let Some((_, cpe)) = &pa.nodes.2 {
                            lower_constant_param_expr(tree, cpe)?
                        } else {
                            Expr::Const(lv(0, 32))
                        };
                        params.push(ParamDecl { name, value });
                    }
                }
            }
            sv_parser::ParameterPortList::Declaration(d) => {
                // `#(parameter WIDTH = 8, parameter DEPTH = 16)` 形式：各項目が個別の
                // `parameter` キーワードを持つため、デフォルト値の式もここで取り出す。
                for item in d.nodes.1.nodes.1.contents() {
                    if let sv_parser::ParameterPortDeclaration::ParameterDeclaration(pd) = item {
                        if let sv_parser::ParameterDeclaration::Param(pp) = pd.as_ref() {
                            for pa in pp.nodes.2.nodes.0.contents() {
                                if let Some(name) =
                                    get_id(tree, RefNode::ParameterIdentifier(&pa.nodes.0))
                                {
                                    let value = if let Some((_, cpe)) = &pa.nodes.2 {
                                        lower_constant_param_expr(tree, cpe)?
                                    } else {
                                        Expr::Const(lv(0, 32))
                                    };
                                    params.push(ParamDecl { name, value });
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(params)
}

// ── module items ──────────────────────────────────────────────────────────────

type Items = (
    Vec<NetDecl>,
    Vec<RegDecl>,
    Vec<MemDecl>,
    Vec<LocalParamDecl>,
    Vec<ContinuousAssign>,
    Vec<InitialConstruct>,
    Vec<AlwaysConstruct>,
    Vec<ModuleInstance>,
    Vec<FunctionDecl>,
    Vec<TaskDecl>,
    GenerateItems,
);

fn lower_nonport_items(
    tree: &SyntaxTree,
    items: &[sv_parser::NonPortModuleItem],
) -> Result<Items, FrontendError> {
    let mut nets = Vec::new();
    let mut regs = Vec::new();
    let mut mems = Vec::new();
    let mut locals = Vec::new();
    let mut assigns = Vec::new();
    let mut initials = Vec::new();
    let mut alwayses = Vec::new();
    let mut instances = Vec::new();
    let mut functions = Vec::new();
    let mut tasks = Vec::new();
    let mut generates = GenerateItems::default();

    for item in items {
        match item {
            sv_parser::NonPortModuleItem::ModuleOrGenerateItem(mogi) => {
                process_mogi(
                    tree,
                    mogi,
                    &mut nets,
                    &mut regs,
                    &mut mems,
                    &mut locals,
                    &mut assigns,
                    &mut initials,
                    &mut alwayses,
                    &mut instances,
                    &mut functions,
                    &mut tasks,
                    &mut generates,
                )?;
            }
            sv_parser::NonPortModuleItem::GenerateRegion(gr) => {
                let g = lower_generate_items(tree, &gr.nodes.1);
                merge_generate_items(&mut generates, g);
            }
            sv_parser::NonPortModuleItem::SpecifyBlock(_) => {
                return Err(unsupported("specify block"));
            }
            _ => {}
        }
    }
    Ok((
        nets, regs, mems, locals, assigns, initials, alwayses, instances, functions, tasks,
        generates,
    ))
}

fn lower_module_items(
    tree: &SyntaxTree,
    items: &[sv_parser::ModuleItem],
) -> Result<Items, FrontendError> {
    let mut nets = Vec::new();
    let mut regs = Vec::new();
    let mut mems = Vec::new();
    let mut locals = Vec::new();
    let mut assigns = Vec::new();
    let mut initials = Vec::new();
    let mut alwayses = Vec::new();
    let mut instances = Vec::new();
    let mut functions = Vec::new();
    let mut tasks = Vec::new();
    let mut generates = GenerateItems::default();

    for item in items {
        if let sv_parser::ModuleItem::NonPortModuleItem(npmi) = item {
            match npmi.as_ref() {
                sv_parser::NonPortModuleItem::ModuleOrGenerateItem(mogi) => {
                    process_mogi(
                        tree,
                        mogi,
                        &mut nets,
                        &mut regs,
                        &mut mems,
                        &mut locals,
                        &mut assigns,
                        &mut initials,
                        &mut alwayses,
                        &mut instances,
                        &mut functions,
                        &mut tasks,
                        &mut generates,
                    )?;
                }
                sv_parser::NonPortModuleItem::GenerateRegion(gr) => {
                    let g = lower_generate_items(tree, &gr.nodes.1);
                    merge_generate_items(&mut generates, g);
                }
                sv_parser::NonPortModuleItem::SpecifyBlock(_) => {
                    return Err(unsupported("specify block"));
                }
                _ => {}
            }
        }
    }
    Ok((
        nets, regs, mems, locals, assigns, initials, alwayses, instances, functions, tasks,
        generates,
    ))
}

fn merge_generate_items(dst: &mut GenerateItems, src: GenerateItems) {
    dst.nets.extend(src.nets);
    dst.regs.extend(src.regs);
    dst.mems.extend(src.mems);
    dst.locals.extend(src.locals);
    dst.assigns.extend(src.assigns);
    dst.initials.extend(src.initials);
    dst.alwayses.extend(src.alwayses);
    dst.instances.extend(src.instances);
    dst.nested.extend(src.nested);
}

#[allow(clippy::too_many_arguments)]
fn process_mogi(
    tree: &SyntaxTree,
    mogi: &sv_parser::ModuleOrGenerateItem,
    nets: &mut Vec<NetDecl>,
    regs: &mut Vec<RegDecl>,
    mems: &mut Vec<MemDecl>,
    locals: &mut Vec<LocalParamDecl>,
    assigns: &mut Vec<ContinuousAssign>,
    initials: &mut Vec<InitialConstruct>,
    alwayses: &mut Vec<AlwaysConstruct>,
    instances: &mut Vec<ModuleInstance>,
    functions: &mut Vec<FunctionDecl>,
    tasks: &mut Vec<TaskDecl>,
    generates: &mut GenerateItems,
) -> Result<(), FrontendError> {
    use sv_parser::ModuleOrGenerateItem as MOGI;
    match mogi {
        MOGI::Module(m) => {
            instances.push(lower_module_inst(tree, &m.nodes.1)?);
        }
        MOGI::Gate(g) => {
            assigns.extend(lower_gate_inst(tree, &g.nodes.1));
        }
        MOGI::ModuleItem(mi) => {
            use sv_parser::ModuleCommonItem as MCI;
            match &mi.nodes.1 {
                MCI::AlwaysConstruct(ac) => alwayses.push(lower_always(tree, ac)?),
                MCI::InitialConstruct(ic) => initials.push(lower_initial(tree, ic)?),
                MCI::ContinuousAssign(ca) => assigns.extend(lower_continuous_assign(tree, ca)?),
                MCI::ModuleOrGenerateItemDeclaration(d) => {
                    lower_decl(tree, d, nets, regs, mems, locals, assigns, functions, tasks)?
                }
                MCI::LoopGenerateConstruct(lgc) => {
                    if let Some(c) = lower_loop_generate(tree, lgc) {
                        generates.nested.push(GenerateConstruct::For(c));
                    }
                }
                MCI::ConditionalGenerateConstruct(cgc) => {
                    if let Some(c) = lower_conditional_generate(tree, cgc) {
                        generates.nested.push(c);
                    }
                }
                _ => {}
            }
        }
        MOGI::Parameter(_) => return Err(unsupported("defparam")),
        MOGI::Udp(_) => return Err(unsupported("UDP instantiation")),
    }
    Ok(())
}

// ── generate / genvar ─────────────────────────────────────────────────────────

fn lower_generate_items(tree: &SyntaxTree, items: &[sv_parser::GenerateItem]) -> GenerateItems {
    let mut g = GenerateItems::default();
    for item in items {
        process_generate_item(tree, item, &mut g);
    }
    g
}

fn process_generate_item(tree: &SyntaxTree, item: &sv_parser::GenerateItem, g: &mut GenerateItems) {
    if let sv_parser::GenerateItem::ModuleOrGenerateItem(mogi) = item {
        process_generate_mogi(tree, mogi, g);
    }
}

fn process_generate_mogi(
    tree: &SyntaxTree,
    mogi: &sv_parser::ModuleOrGenerateItem,
    g: &mut GenerateItems,
) {
    use sv_parser::ModuleOrGenerateItem as MOGI;
    match mogi {
        MOGI::Module(m) => {
            if let Ok(inst) = lower_module_inst(tree, &m.nodes.1) {
                g.instances.push(inst);
            }
        }
        MOGI::Gate(gt) => {
            g.assigns.extend(lower_gate_inst(tree, &gt.nodes.1));
        }
        MOGI::ModuleItem(mi) => {
            use sv_parser::ModuleCommonItem as MCI;
            match &mi.nodes.1 {
                MCI::AlwaysConstruct(ac) => {
                    if let Ok(a) = lower_always(tree, ac) {
                        g.alwayses.push(a);
                    }
                }
                MCI::InitialConstruct(ic) => {
                    if let Ok(i) = lower_initial(tree, ic) {
                        g.initials.push(i);
                    }
                }
                MCI::ContinuousAssign(ca) => {
                    if let Ok(v) = lower_continuous_assign(tree, ca) {
                        g.assigns.extend(v);
                    }
                }
                MCI::ModuleOrGenerateItemDeclaration(d) => lower_generate_decl(tree, d, g),
                MCI::LoopGenerateConstruct(lgc) => {
                    if let Some(c) = lower_loop_generate(tree, lgc) {
                        g.nested.push(GenerateConstruct::For(c));
                    }
                }
                MCI::ConditionalGenerateConstruct(cgc) => {
                    if let Some(c) = lower_conditional_generate(tree, cgc) {
                        g.nested.push(c);
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
}

fn lower_generate_decl(
    tree: &SyntaxTree,
    d: &sv_parser::ModuleOrGenerateItemDeclaration,
    g: &mut GenerateItems,
) {
    use sv_parser::ModuleOrGenerateItemDeclaration as D;
    if let D::PackageOrGenerateItemDeclaration(p) = d {
        use sv_parser::PackageOrGenerateItemDeclaration as PD;
        match p.as_ref() {
            PD::NetDeclaration(nd) => {
                if let Ok((nets, assigns)) = lower_net_decl(tree, nd) {
                    g.nets.extend(nets);
                    g.assigns.extend(assigns);
                }
            }
            PD::DataDeclaration(dd) => {
                if let Ok((regs, mems)) = lower_data_decl(tree, dd) {
                    g.regs.extend(regs);
                    g.mems.extend(mems);
                }
            }
            PD::LocalParameterDeclaration(lp) => {
                if let Ok(v) = lower_localparam(tree, lp) {
                    g.locals.extend(v);
                }
            }
            _ => {}
        }
    }
}

fn lower_generate_block(tree: &SyntaxTree, gb: &sv_parser::GenerateBlock) -> GenerateItems {
    use sv_parser::GenerateBlock as GB;
    match gb {
        GB::GenerateItem(gi) => {
            let mut g = GenerateItems::default();
            process_generate_item(tree, gi, &mut g);
            g
        }
        GB::Multiple(m) => lower_generate_items(tree, &m.nodes.3),
    }
}

fn lower_loop_generate(
    tree: &SyntaxTree,
    lgc: &sv_parser::LoopGenerateConstruct,
) -> Option<GenerateFor> {
    let (init, _, cond, _, iter) = &lgc.nodes.1.nodes.1;
    let var = get_id(tree, RefNode::GenvarIdentifier(&init.nodes.1))?;
    let init_expr = lower_constant_expr(tree, &init.nodes.3).ok()?;
    let cond_expr = lower_constant_expr(tree, &cond.nodes.0).ok()?;
    let step_expr = lower_genvar_iteration(tree, &var, iter)?;
    let body = lower_generate_block(tree, &lgc.nodes.2);
    Some(GenerateFor {
        var,
        init: init_expr,
        cond: cond_expr,
        step: step_expr,
        body,
    })
}

fn lower_genvar_iteration(
    tree: &SyntaxTree,
    var: &SmolStr,
    iter: &sv_parser::GenvarIteration,
) -> Option<Expr> {
    use sv_parser::GenvarIteration as GI;
    match iter {
        // ++i / i++ は ++/-- の区別をテキストで判定する
        GI::Prefix(p) => Some(inc_or_dec_expr(tree, var, &p.nodes.0)),
        GI::Suffix(s) => Some(inc_or_dec_expr(tree, var, &s.nodes.1)),
        GI::Assignment(a) => {
            // GenvarIterationAssignment.nodes = (GenvarIdentifier, AssignmentOperator, GenvarExpression)
            lower_constant_expr(tree, &a.nodes.2.nodes.0).ok()
        }
    }
}

fn inc_or_dec_expr(tree: &SyntaxTree, var: &SmolStr, op: &sv_parser::IncOrDecOperator) -> Expr {
    let text = tree.get_str(op).unwrap_or("++").trim();
    let bin_op = if text == "--" { BinOp::Sub } else { BinOp::Add };
    Expr::Bin(
        bin_op,
        Box::new(Expr::Net(var.clone())),
        Box::new(Expr::Const(lv(1, 32))),
    )
}

fn lower_conditional_generate(
    tree: &SyntaxTree,
    cgc: &sv_parser::ConditionalGenerateConstruct,
) -> Option<GenerateConstruct> {
    use sv_parser::ConditionalGenerateConstruct as CGC;
    match cgc {
        CGC::If(ifc) => {
            let cond = lower_constant_expr(tree, &ifc.nodes.1.nodes.1).ok()?;
            let then_items = lower_generate_block(tree, &ifc.nodes.2);
            let else_items = match &ifc.nodes.3 {
                Some((_, gb)) => lower_generate_block(tree, gb),
                None => GenerateItems::default(),
            };
            Some(GenerateConstruct::If(GenerateIf {
                cond,
                then_items,
                else_items,
            }))
        }
        CGC::Case(casec) => {
            let sel = lower_constant_expr(tree, &casec.nodes.1.nodes.1).ok()?;
            let mut arms = Vec::new();
            let mut default = None;
            for item in &casec.nodes.2 {
                match item {
                    sv_parser::CaseGenerateItem::Nondefault(nd) => {
                        let pats: Vec<Expr> = nd
                            .nodes
                            .0
                            .contents()
                            .into_iter()
                            .filter_map(|ce| lower_constant_expr(tree, ce).ok())
                            .collect();
                        let body = lower_generate_block(tree, &nd.nodes.2);
                        arms.push((pats, body));
                    }
                    sv_parser::CaseGenerateItem::Default(d) => {
                        default = Some(lower_generate_block(tree, &d.nodes.2));
                    }
                }
            }
            Some(GenerateConstruct::Case(GenerateCase { sel, arms, default }))
        }
    }
}

// ── constant expression lowering (genvar/parameter aware) ─────────────────────

fn lower_constant_expr(
    tree: &SyntaxTree,
    ce: &sv_parser::ConstantExpression,
) -> Result<Expr, FrontendError> {
    use sv_parser::ConstantExpression as CE;
    match ce {
        CE::ConstantPrimary(p) => lower_constant_primary(tree, p),
        CE::Unary(u) => {
            let op = lower_unary_op(tree, &u.nodes.0)?;
            let inner = lower_constant_primary(tree, &u.nodes.2)?;
            Ok(Expr::Un(op, Box::new(inner)))
        }
        CE::Binary(_) => {
            let mut operands = Vec::new();
            let mut ops = Vec::new();
            let tail = flatten_constant_binary_chain(tree, ce, &mut operands, &mut ops)?;
            let combined = build_binop_tree(operands, ops);
            match tail {
                Some((then_e, else_e)) => Ok(Expr::Cond(
                    Box::new(combined),
                    Box::new(then_e),
                    Box::new(else_e),
                )),
                None => Ok(combined),
            }
        }
        CE::Ternary(t) => {
            let cond = lower_constant_expr(tree, &t.nodes.0)?;
            let then_e = lower_constant_expr(tree, &t.nodes.3)?;
            let else_e = lower_constant_expr(tree, &t.nodes.5)?;
            Ok(Expr::Cond(
                Box::new(cond),
                Box::new(then_e),
                Box::new(else_e),
            ))
        }
        CE::Inside(_) => Err(unsupported("constant inside expression")),
    }
}

fn lower_constant_primary(
    tree: &SyntaxTree,
    p: &sv_parser::ConstantPrimary,
) -> Result<Expr, FrontendError> {
    use sv_parser::ConstantPrimary as CP;
    match p {
        CP::PrimaryLiteral(lit) => lower_primary_literal(tree, lit),
        CP::GenvarIdentifier(g) => {
            let name = get_id(tree, RefNode::GenvarIdentifier(g))
                .ok_or_else(|| FrontendError::ParseError("genvar identifier missing".into()))?;
            Ok(Expr::Net(name))
        }
        CP::PsParameter(pp) => {
            let text = tree.get_str(&pp.nodes.0).unwrap_or("?").trim();
            Ok(Expr::Net(SmolStr::from(text)))
        }
        CP::MintypmaxExpression(m) => lower_constant_mintypmax(tree, &m.nodes.0.nodes.1),
        _ => {
            if let Some(t) = tree.get_str(p) {
                Ok(parse_simple_const_expr(t.trim()))
            } else {
                Err(unsupported("constant primary kind"))
            }
        }
    }
}

fn lower_constant_mintypmax(
    tree: &SyntaxTree,
    m: &sv_parser::ConstantMintypmaxExpression,
) -> Result<Expr, FrontendError> {
    use sv_parser::ConstantMintypmaxExpression as CM;
    match m {
        CM::Unary(ce) => lower_constant_expr(tree, ce),
        CM::Ternary(t) => {
            let cond = lower_constant_expr(tree, &t.nodes.0)?;
            let then_e = lower_constant_expr(tree, &t.nodes.2)?;
            let else_e = lower_constant_expr(tree, &t.nodes.4)?;
            Ok(Expr::Cond(
                Box::new(cond),
                Box::new(then_e),
                Box::new(else_e),
            ))
        }
    }
}

fn lower_constant_param_expr(
    tree: &SyntaxTree,
    cpe: &sv_parser::ConstantParamExpression,
) -> Result<Expr, FrontendError> {
    match cpe {
        sv_parser::ConstantParamExpression::ConstantMintypmaxExpression(m) => {
            lower_constant_mintypmax(tree, m)
        }
        sv_parser::ConstantParamExpression::DataType(_) => {
            Err(unsupported("constant param expression: data type"))
        }
        sv_parser::ConstantParamExpression::Dollar(_) => {
            Err(unsupported("constant param expression: dollar"))
        }
    }
}

fn lower_mintypmax(
    tree: &SyntaxTree,
    m: &sv_parser::MintypmaxExpression,
) -> Result<Expr, FrontendError> {
    match m {
        sv_parser::MintypmaxExpression::Expression(e) => lower_expression(tree, e),
        sv_parser::MintypmaxExpression::Ternary(t) => {
            let cond = lower_expression(tree, &t.nodes.0)?;
            let then_e = lower_expression(tree, &t.nodes.2)?;
            let else_e = lower_expression(tree, &t.nodes.4)?;
            Ok(Expr::Cond(
                Box::new(cond),
                Box::new(then_e),
                Box::new(else_e),
            ))
        }
    }
}

fn lower_param_expr(
    tree: &SyntaxTree,
    pe: &sv_parser::ParamExpression,
) -> Result<Expr, FrontendError> {
    match pe {
        sv_parser::ParamExpression::MintypmaxExpression(m) => lower_mintypmax(tree, m),
        sv_parser::ParamExpression::DataType(_) => Err(unsupported("param expression: data type")),
        sv_parser::ParamExpression::Dollar(_) => Err(unsupported("param expression: dollar")),
    }
}

// ── declarations ──────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn lower_decl(
    tree: &SyntaxTree,
    d: &sv_parser::ModuleOrGenerateItemDeclaration,
    nets: &mut Vec<NetDecl>,
    regs: &mut Vec<RegDecl>,
    mems: &mut Vec<MemDecl>,
    locals: &mut Vec<LocalParamDecl>,
    assigns: &mut Vec<ContinuousAssign>,
    functions: &mut Vec<FunctionDecl>,
    tasks: &mut Vec<TaskDecl>,
) -> Result<(), FrontendError> {
    use sv_parser::ModuleOrGenerateItemDeclaration as D;
    if let D::PackageOrGenerateItemDeclaration(p) = d {
        use sv_parser::PackageOrGenerateItemDeclaration as PD;
        match p.as_ref() {
            PD::NetDeclaration(nd) => {
                let (new_nets, new_assigns) = lower_net_decl(tree, nd)?;
                nets.extend(new_nets);
                assigns.extend(new_assigns);
            }
            PD::DataDeclaration(dd) => {
                let (new_regs, new_mems) = lower_data_decl(tree, dd)?;
                regs.extend(new_regs);
                mems.extend(new_mems);
            }
            PD::LocalParameterDeclaration(lp) => locals.extend(lower_localparam(tree, lp)?),
            PD::FunctionDeclaration(fd) => {
                if let Some(f) = lower_function_decl(tree, fd)? {
                    functions.push(f);
                }
            }
            PD::TaskDeclaration(td) => {
                if let Some(t) = lower_task_decl(tree, td)? {
                    tasks.push(t);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

// ── function / task declarations ──────────────────────────────────────────────

fn lower_tf_port_item(
    tree: &SyntaxTree,
    item: &sv_parser::TfPortItem,
    default_dir: PortDirection,
) -> Option<TfArg> {
    use sv_parser::TfPortDirection as TD;
    let dir = match &item.nodes.1 {
        Some(TD::PortDirection(d)) => lower_port_direction(d),
        Some(TD::ConstRef(_)) => PortDirection::Input,
        None => default_dir,
    };
    let (_, width_expr) = packed_width_expr(tree, RefNode::TfPortItem(item));
    let signed = has_signed(RefNode::TfPortItem(item));
    let (name, _, _) = item.nodes.4.as_ref()?;
    let name = get_id(tree, RefNode::PortIdentifier(name))?;
    Some(TfArg {
        name,
        width_expr,
        direction: dir,
        signed,
    })
}

/// `function/task ...; input a; output b; ...` style (no parenthesized port list):
/// each `TfItemDeclaration::TfPortDeclaration` declares one or more args.
fn lower_tf_item_decls(
    tree: &SyntaxTree,
    items: &[sv_parser::TfItemDeclaration],
) -> (Vec<TfArg>, Vec<RegDecl>) {
    let mut args = Vec::new();
    let mut locals = Vec::new();
    for item in items {
        match item {
            sv_parser::TfItemDeclaration::TfPortDeclaration(d) => {
                use sv_parser::TfPortDirection as TD;
                let dir = match &d.nodes.1 {
                    TD::PortDirection(pd) => lower_port_direction(pd),
                    TD::ConstRef(_) => PortDirection::Input,
                };
                let (_, width_expr) = packed_width_expr(tree, RefNode::TfPortDeclaration(d));
                let signed = has_signed(RefNode::TfPortDeclaration(d));
                for (name_node, _, _) in d.nodes.4.nodes.0.contents() {
                    if let Some(name) = get_id(tree, RefNode::PortIdentifier(name_node)) {
                        args.push(TfArg {
                            name,
                            width_expr: width_expr.clone(),
                            direction: dir,
                            signed,
                        });
                    }
                }
            }
            sv_parser::TfItemDeclaration::BlockItemDeclaration(bid) => {
                if let sv_parser::BlockItemDeclaration::Data(d) = bid.as_ref() {
                    if let Ok((new_regs, _)) = lower_data_decl(tree, &d.nodes.1) {
                        locals.extend(new_regs);
                    }
                }
            }
        }
    }
    (args, locals)
}

fn lower_function_decl(
    tree: &SyntaxTree,
    fd: &sv_parser::FunctionDeclaration,
) -> Result<Option<FunctionDecl>, FrontendError> {
    use sv_parser::FunctionBodyDeclaration as FB;
    match &fd.nodes.2 {
        FB::WithPort(p) => {
            let name = get_id(tree, RefNode::FunctionIdentifier(&p.nodes.2))
                .ok_or_else(|| FrontendError::ParseError("function name missing".into()))?;
            let (width, width_expr) =
                packed_width_expr(tree, RefNode::FunctionDataTypeOrImplicit(&p.nodes.0));
            let signed = has_signed(RefNode::FunctionDataTypeOrImplicit(&p.nodes.0));
            let mut args = Vec::new();
            if let Some(list) = &p.nodes.3.nodes.1 {
                for item in list.nodes.0.contents() {
                    if let Some(a) = lower_tf_port_item(tree, item, PortDirection::Input) {
                        args.push(a);
                    }
                }
            }
            let mut locals = Vec::new();
            for bid in &p.nodes.5 {
                if let sv_parser::BlockItemDeclaration::Data(d) = bid {
                    let (new_regs, _) = lower_data_decl(tree, &d.nodes.1)?;
                    locals.extend(new_regs);
                }
            }
            let mut stmts = Vec::new();
            for fs in &p.nodes.6 {
                if let sv_parser::FunctionStatementOrNull::Statement(s) = fs {
                    stmts.push(lower_statement(tree, &s.nodes.0)?);
                }
            }
            Ok(Some(FunctionDecl {
                name,
                width,
                width_expr,
                signed,
                args,
                locals,
                body: Stmt::Block(stmts),
            }))
        }
        FB::WithoutPort(p) => {
            let name = get_id(tree, RefNode::FunctionIdentifier(&p.nodes.2))
                .ok_or_else(|| FrontendError::ParseError("function name missing".into()))?;
            let (width, width_expr) =
                packed_width_expr(tree, RefNode::FunctionDataTypeOrImplicit(&p.nodes.0));
            let signed = has_signed(RefNode::FunctionDataTypeOrImplicit(&p.nodes.0));
            let (args, locals) = lower_tf_item_decls(tree, &p.nodes.4);
            let mut stmts = Vec::new();
            for fs in &p.nodes.5 {
                if let sv_parser::FunctionStatementOrNull::Statement(s) = fs {
                    stmts.push(lower_statement(tree, &s.nodes.0)?);
                }
            }
            Ok(Some(FunctionDecl {
                name,
                width,
                width_expr,
                signed,
                args,
                locals,
                body: Stmt::Block(stmts),
            }))
        }
    }
}

fn lower_task_decl(
    tree: &SyntaxTree,
    td: &sv_parser::TaskDeclaration,
) -> Result<Option<TaskDecl>, FrontendError> {
    use sv_parser::TaskBodyDeclaration as TB;
    match &td.nodes.2 {
        TB::WithPort(p) => {
            let name = get_id(tree, RefNode::TaskIdentifier(&p.nodes.1))
                .ok_or_else(|| FrontendError::ParseError("task name missing".into()))?;
            let mut args = Vec::new();
            if let Some(list) = &p.nodes.2.nodes.1 {
                for item in list.nodes.0.contents() {
                    if let Some(a) = lower_tf_port_item(tree, item, PortDirection::Input) {
                        args.push(a);
                    }
                }
            }
            let mut locals = Vec::new();
            for bid in &p.nodes.4 {
                if let sv_parser::BlockItemDeclaration::Data(d) = bid {
                    let (new_regs, _) = lower_data_decl(tree, &d.nodes.1)?;
                    locals.extend(new_regs);
                }
            }
            let mut stmts = Vec::new();
            for s in &p.nodes.5 {
                stmts.push(lower_stmt_or_null(tree, s)?);
            }
            Ok(Some(TaskDecl {
                name,
                args,
                locals,
                body: Stmt::Block(stmts),
            }))
        }
        TB::WithoutPort(p) => {
            let name = get_id(tree, RefNode::TaskIdentifier(&p.nodes.1))
                .ok_or_else(|| FrontendError::ParseError("task name missing".into()))?;
            let (args, locals) = lower_tf_item_decls(tree, &p.nodes.3);
            let mut stmts = Vec::new();
            for s in &p.nodes.4 {
                stmts.push(lower_stmt_or_null(tree, s)?);
            }
            Ok(Some(TaskDecl {
                name,
                args,
                locals,
                body: Stmt::Block(stmts),
            }))
        }
    }
}

fn lower_net_decl(
    tree: &SyntaxTree,
    nd: &sv_parser::NetDeclaration,
) -> Result<(Vec<NetDecl>, Vec<ContinuousAssign>), FrontendError> {
    let mut nets = Vec::new();
    let mut assigns = Vec::new();
    if let sv_parser::NetDeclaration::NetType(nt) = nd {
        let (width, width_expr) = packed_width_expr(tree, RefNode::NetDeclarationNetType(nt));
        let signed = has_signed(RefNode::NetDeclarationNetType(nt));
        for assignment in nt.nodes.5.nodes.0.contents() {
            if let Some(name) = get_id(tree, RefNode::NetIdentifier(&assignment.nodes.0)) {
                nets.push(NetDecl {
                    name: name.clone(),
                    width,
                    kind: NetKind::Wire,
                    width_expr: width_expr.clone(),
                    signed,
                });
                // ネット宣言時の初期化式は連続代入として扱う。
                if let Some((_, expr)) = &assignment.nodes.2 {
                    assigns.push(ContinuousAssign {
                        lval: LValue::Net(name),
                        expr: lower_expression(tree, expr)?,
                    });
                }
            }
        }
    }
    Ok((nets, assigns))
}

fn lower_data_decl(
    tree: &SyntaxTree,
    dd: &sv_parser::DataDeclaration,
) -> Result<(Vec<RegDecl>, Vec<MemDecl>), FrontendError> {
    let mut regs = Vec::new();
    let mut mems = Vec::new();
    if let sv_parser::DataDeclaration::Variable(dv) = dd {
        // Check if this is an integer type (keyword "integer")
        let is_integer = has_integer_type(tree, RefNode::DataDeclarationVariable(dv));
        let (width, width_expr) = if is_integer {
            (32u32, Expr::Const(lv(32, 32)))
        } else {
            packed_width_expr(tree, RefNode::DataDeclarationVariable(dv))
        };
        // `integer` は IEEE 1364 上 32bit signed 型として定義される
        let signed = is_integer || has_signed(RefNode::DataDeclarationVariable(dv));

        // Check for unpacked dimension (indicates memory array)
        for vda_node in unwrap_all_var_decl_assignments(RefNode::DataDeclarationVariable(dv)) {
            let name = match get_id(tree, vda_node.clone()) {
                Some(n) => n,
                None => continue,
            };
            let depth_expr = extract_unpacked_dim_depth(tree, vda_node);
            if let Some(depth) = depth_expr {
                mems.push(MemDecl {
                    name,
                    elem_width: width_expr.clone(),
                    depth,
                });
            } else {
                regs.push(RegDecl {
                    name,
                    width,
                    width_expr: width_expr.clone(),
                    signed,
                });
            }
        }
    }
    Ok((regs, mems))
}

fn has_integer_type(tree: &SyntaxTree, node: RefNode) -> bool {
    for inner in node {
        if let RefNode::IntegerAtomType(iat) = inner {
            if let Some(t) = tree.get_str(iat) {
                return t.trim() == "integer";
            }
        }
    }
    false
}

/// `signed` キーワード（`Signing::Signed`）がノード内に存在するかを走査する。
/// `wire signed [7:0] x`、`reg signed [7:0] y`、ポート宣言の `signed` に対応。
fn has_signed(node: RefNode) -> bool {
    for inner in node {
        if let RefNode::Signing(s) = inner {
            if matches!(s, sv_parser::Signing::Signed(_)) {
                return true;
            }
        }
    }
    false
}

fn packed_width_expr(tree: &SyntaxTree, node: RefNode) -> (u32, Expr) {
    if let Some(RefNode::PackedDimension(pd)) = unwrap_node!(node, PackedDimension) {
        if let Some(t) = tree.get_str(pd) {
            let s = t.trim().trim_start_matches('[').trim_end_matches(']');
            let mut it = s.splitn(2, ':');
            let msb_str = it.next().unwrap_or("0").trim();
            let lsb_str = it.next().unwrap_or("0").trim();
            let msb_e = parse_simple_const_expr(msb_str);
            let lsb_e = parse_simple_const_expr(lsb_str);
            // width = msb - lsb + 1
            let width_e = Expr::Bin(
                BinOp::Add,
                Box::new(Expr::Bin(
                    BinOp::Sub,
                    Box::new(msb_e.clone()),
                    Box::new(lsb_e),
                )),
                Box::new(Expr::Const(lv(1, 32))),
            );
            // 静的宣言の場合はリテラル幅の算出を試みる
            let literal_width = if let Expr::Const(v) = &msb_e {
                v.pad_to_width(32) as u32 + 1
            } else {
                0 // param依存の動的幅
            };
            return (literal_width, width_e);
        }
    }
    (1, Expr::Const(lv(1, 32)))
}

fn parse_simple_const_expr(s: &str) -> Expr {
    let s = s.trim();
    if let Ok(n) = s.parse::<i64>() {
        return Expr::Const(lv(n as u64, 32));
    }
    // Handle IDENT-1, IDENT+1 etc.
    if let Some(pos) = s.rfind('-') {
        let lhs = s[..pos].trim();
        let rhs = s[pos + 1..].trim();
        if !lhs.is_empty() && !rhs.is_empty() {
            return Expr::Bin(
                BinOp::Sub,
                Box::new(parse_simple_const_expr(lhs)),
                Box::new(parse_simple_const_expr(rhs)),
            );
        }
    }
    if let Some(pos) = s.find('+') {
        let lhs = s[..pos].trim();
        let rhs = s[pos + 1..].trim();
        if !lhs.is_empty() {
            return Expr::Bin(
                BinOp::Add,
                Box::new(parse_simple_const_expr(lhs)),
                Box::new(parse_simple_const_expr(rhs)),
            );
        }
    }
    // Check for $clog2(...)
    if s.starts_with("$clog2(") && s.ends_with(')') {
        let inner = &s[7..s.len() - 1];
        return Expr::SysFunc(SysFuncKind::Clog2, vec![parse_simple_const_expr(inner)]);
    }
    // サイズ付き基数リテラル（`8'b10000000`等）。上の `+`/`-` 分割で拾われなかった
    // 残りはここでのみ判定する（`'` を含む複合式を誤って丸ごと数値パースしない
    // ため、識別子フォールバックの直前に置く）。
    if s.contains('\'') {
        let (v, signed) = parse_number_text(s);
        return if signed {
            Expr::SignedConst(v)
        } else {
            Expr::Const(v)
        };
    }
    Expr::Net(SmolStr::from(s))
}

fn unwrap_all_var_decl_assignments(node: RefNode) -> Vec<RefNode> {
    let mut out = Vec::new();
    for inner in node {
        if let RefNode::VariableDeclAssignment(_) = &inner {
            out.push(inner);
        }
    }
    out
}

fn extract_unpacked_dim_depth(tree: &SyntaxTree, node: RefNode) -> Option<Expr> {
    for inner in node {
        if let RefNode::UnpackedDimensionRange(r) = inner {
            if let Some(t) = tree.get_str(r) {
                let s = t.trim().trim_start_matches('[').trim_end_matches(']');
                let mut it = s.splitn(2, ':');
                let lo_str = it.next().unwrap_or("0").trim();
                let hi_str = it.next().unwrap_or("0").trim();
                let lo_e = parse_simple_const_expr(lo_str);
                let hi_e = parse_simple_const_expr(hi_str);
                // depth = hi - lo + 1
                let d = Expr::Bin(
                    BinOp::Add,
                    Box::new(Expr::Bin(BinOp::Sub, Box::new(hi_e), Box::new(lo_e))),
                    Box::new(Expr::Const(lv(1, 32))),
                );
                return Some(d);
            }
        }
    }
    None
}

fn lower_localparam(
    tree: &SyntaxTree,
    lp: &(sv_parser::LocalParameterDeclaration, sv_parser::Symbol),
) -> Result<Vec<LocalParamDecl>, FrontendError> {
    let mut out = Vec::new();
    if let sv_parser::LocalParameterDeclaration::Param(lpdp) = &lp.0 {
        for pa in lpdp.nodes.2.nodes.0.contents() {
            if let Some(name) = get_id(tree, RefNode::ParameterIdentifier(&pa.nodes.0)) {
                let value = if let Some((_, cpe)) = &pa.nodes.2 {
                    lower_constant_param_expr(tree, cpe)?
                } else {
                    Expr::Const(lv(0, 32))
                };
                out.push(LocalParamDecl { name, value });
            }
        }
    }
    Ok(out)
}

// ── continuous assign ─────────────────────────────────────────────────────────

fn lower_continuous_assign(
    tree: &SyntaxTree,
    ca: &sv_parser::ContinuousAssign,
) -> Result<Vec<ContinuousAssign>, FrontendError> {
    let mut out = Vec::new();
    if let sv_parser::ContinuousAssign::Net(n) = ca {
        // ListOfNetAssignments: List<Symbol, NetAssignment>
        // NetAssignment: (NetLvalue, Symbol, Expression)
        for na_node in unwrap_all_net_assignments(RefNode::ContinuousAssignNet(n)) {
            if let RefNode::NetAssignment(na) = na_node {
                let lval = lower_net_lvalue(tree, RefNode::NetLvalue(&na.nodes.0))?;
                let expr = lower_expression(tree, &na.nodes.2)?;
                out.push(ContinuousAssign { lval, expr });
            }
        }
    }
    Ok(out)
}

// ── gate primitives (and/or/nand/nor/xor/xnor/buf/not) ─────────────────────────
// Lowered directly into continuous assigns: a gate is just combinational logic.

fn gate_keyword_text<'a>(tree: &'a SyntaxTree, kw: &sv_parser::Keyword) -> &'a str {
    tree.get_str(kw).unwrap_or("").trim()
}

fn lower_gate_inst(tree: &SyntaxTree, gi: &sv_parser::GateInstantiation) -> Vec<ContinuousAssign> {
    use sv_parser::GateInstantiation as GI;
    let mut out = Vec::new();
    match gi {
        GI::NInput(n) => {
            let (op, negate) = match gate_keyword_text(tree, &n.nodes.0.nodes.0) {
                "and" => (BinOp::BitAnd, false),
                "nand" => (BinOp::BitAnd, true),
                "or" => (BinOp::BitOr, false),
                "nor" => (BinOp::BitOr, true),
                "xor" => (BinOp::BitXor, false),
                "xnor" => (BinOp::BitXor, true),
                _ => return out,
            };
            for inst in n.nodes.3.contents() {
                let (out_term, _, in_terms) = &inst.nodes.1.nodes.1;
                let lval = match lower_net_lvalue(tree, RefNode::NetLvalue(&out_term.nodes.0)) {
                    Ok(lv) => lv,
                    Err(_) => continue,
                };
                let inputs: Vec<Expr> = in_terms
                    .contents()
                    .into_iter()
                    .filter_map(|it| lower_expression(tree, &it.nodes.0).ok())
                    .collect();
                if inputs.is_empty() {
                    continue;
                }
                let mut expr = inputs[0].clone();
                for rhs in &inputs[1..] {
                    expr = Expr::Bin(op, Box::new(expr), Box::new(rhs.clone()));
                }
                if negate {
                    expr = Expr::Un(UnOp::BitNot, Box::new(expr));
                }
                out.push(ContinuousAssign { lval, expr });
            }
        }
        GI::NOutput(n) => {
            let negate = match gate_keyword_text(tree, &n.nodes.0.nodes.0) {
                "buf" => false,
                "not" => true,
                _ => return out,
            };
            for inst in n.nodes.3.contents() {
                let (out_terms, _, in_term) = &inst.nodes.1.nodes.1;
                let in_expr = match lower_expression(tree, &in_term.nodes.0) {
                    Ok(e) => e,
                    Err(_) => continue,
                };
                let in_expr = if negate {
                    Expr::Un(UnOp::BitNot, Box::new(in_expr))
                } else {
                    in_expr
                };
                for ot in out_terms.contents() {
                    if let Ok(lval) = lower_net_lvalue(tree, RefNode::NetLvalue(&ot.nodes.0)) {
                        out.push(ContinuousAssign {
                            lval,
                            expr: in_expr.clone(),
                        });
                    }
                }
            }
        }
        _ => {} // switch/cmos/pass/pullup/pulldown gates: unsupported subset
    }
    out
}

fn unwrap_all_net_assignments(node: RefNode) -> Vec<RefNode> {
    let mut out = Vec::new();
    for inner in node {
        if let RefNode::NetAssignment(_) = &inner {
            out.push(inner);
        }
    }
    out
}

fn lower_net_lvalue(tree: &SyntaxTree, node: RefNode) -> Result<LValue, FrontendError> {
    if let Some(name_node) = unwrap_node!(node, NetIdentifier) {
        if let Some(name) = get_id(tree, name_node) {
            return Ok(LValue::Net(name));
        }
    }
    Err(FrontendError::ParseError("unsupported net lvalue".into()))
}

// ── always / initial ──────────────────────────────────────────────────────────

fn lower_always(
    tree: &SyntaxTree,
    ac: &sv_parser::AlwaysConstruct,
) -> Result<AlwaysConstruct, FrontendError> {
    let (sensitivity, body) = split_timing_from_stmt(tree, &ac.nodes.1)?;
    Ok(AlwaysConstruct { sensitivity, body })
}

fn lower_initial(
    tree: &SyntaxTree,
    ic: &sv_parser::InitialConstruct,
) -> Result<InitialConstruct, FrontendError> {
    let body = lower_stmt_or_null(tree, &ic.nodes.1)?;
    Ok(InitialConstruct { body })
}

fn split_timing_from_stmt(
    tree: &SyntaxTree,
    stmt: &sv_parser::Statement,
) -> Result<(Option<Sensitivity>, Stmt), FrontendError> {
    use sv_parser::StatementItem as SI;
    if let SI::ProceduralTimingControlStatement(ptcs) = &stmt.nodes.2 {
        use sv_parser::ProceduralTimingControl as PTC;
        if let PTC::EventControl(ec) = &ptcs.nodes.0 {
            let sens = lower_event_control(tree, ec)?;
            let body = lower_stmt_or_null(tree, &ptcs.nodes.1)?;
            return Ok((Some(sens), body));
        }
    }
    Ok((None, lower_statement(tree, stmt)?))
}

fn lower_event_control(
    tree: &SyntaxTree,
    ec: &sv_parser::EventControl,
) -> Result<Sensitivity, FrontendError> {
    use sv_parser::EventControl as EC;
    match ec {
        EC::Asterisk(_) | EC::ParenAsterisk(_) => Ok(Sensitivity::All),
        EC::EventExpression(e) => {
            // EventControlEventExpression.nodes = (Symbol "@", Paren<EventExpression>)
            // Paren.nodes = (Symbol "(", EventExpression, Symbol ")")
            let items = collect_sens_items(tree, &e.nodes.1.nodes.1)?;
            Ok(Sensitivity::Items(items))
        }
        _ => Ok(Sensitivity::All),
    }
}

fn collect_sens_items(
    tree: &SyntaxTree,
    expr: &sv_parser::EventExpression,
) -> Result<Vec<SensitivityItem>, FrontendError> {
    use sv_parser::EventExpression as EE;
    match expr {
        EE::Or(o) => {
            // EventExpressionOr.nodes = (EventExpression, Keyword "or", EventExpression)
            let mut v = collect_sens_items(tree, &o.nodes.0)?;
            v.extend(collect_sens_items(tree, &o.nodes.2)?);
            Ok(v)
        }
        EE::Comma(c) => {
            // EventExpressionComma.nodes = (EventExpression, Symbol, EventExpression)
            let mut v = collect_sens_items(tree, &c.nodes.0)?;
            v.extend(collect_sens_items(tree, &c.nodes.2)?);
            Ok(v)
        }
        EE::Paren(p) => {
            // EventExpressionParen.nodes = (Paren<EventExpression>,)
            // Paren.nodes = (Symbol, EventExpression, Symbol)
            collect_sens_items(tree, &p.nodes.0.nodes.1)
        }
        EE::Expression(e) => {
            // EventExpressionExpression.nodes = (Option<EdgeIdentifier>, Expression, Option<...>)
            let edge = match &e.nodes.0 {
                Some(sv_parser::EdgeIdentifier::Posedge(_)) => Some(EdgeType::Posedge),
                Some(sv_parser::EdgeIdentifier::Negedge(_)) => Some(EdgeType::Negedge),
                _ => None,
            };
            let signal =
                get_id(tree, RefNode::Expression(&e.nodes.1)).unwrap_or_else(|| SmolStr::new("?"));
            Ok(vec![SensitivityItem { edge, signal }])
        }
        _ => Ok(vec![]),
    }
}

// ── statement lowering ────────────────────────────────────────────────────────

fn lower_stmt_or_null(
    tree: &SyntaxTree,
    s: &sv_parser::StatementOrNull,
) -> Result<Stmt, FrontendError> {
    match s {
        sv_parser::StatementOrNull::Statement(stmt) => lower_statement(tree, stmt),
        sv_parser::StatementOrNull::Attribute(_) => Ok(Stmt::Block(vec![])),
    }
}

fn lower_statement(tree: &SyntaxTree, stmt: &sv_parser::Statement) -> Result<Stmt, FrontendError> {
    lower_stmt_item(tree, &stmt.nodes.2)
}

fn lower_stmt_item(
    tree: &SyntaxTree,
    item: &sv_parser::StatementItem,
) -> Result<Stmt, FrontendError> {
    use sv_parser::StatementItem as SI;
    match item {
        SI::SeqBlock(sb) => lower_seq_block(tree, sb),
        SI::ConditionalStatement(cs) => lower_conditional(tree, cs),
        SI::BlockingAssignment(ba) => lower_blocking(tree, &ba.0),
        SI::NonblockingAssignment(na) => lower_nonblocking(tree, &na.0),
        SI::ProceduralTimingControlStatement(ptcs) => lower_timing_ctrl(tree, ptcs),
        SI::SubroutineCallStatement(sc) => lower_syscall_stmt(tree, sc),
        SI::CaseStatement(cs) => lower_case(tree, cs),
        SI::LoopStatement(ls) => lower_loop_stmt(tree, ls),
        SI::DisableStatement(ds) => lower_disable(tree, ds),
        SI::ParBlock(pb) => lower_par_block(tree, pb),
        _ => Err(unsupported("statement kind")),
    }
}

fn lower_disable(
    tree: &SyntaxTree,
    ds: &sv_parser::DisableStatement,
) -> Result<Stmt, FrontendError> {
    use sv_parser::DisableStatement as DS;
    match ds {
        DS::Block(b) => {
            let ident = &b.nodes.1.nodes.0.nodes.2;
            let name = get_id(tree, RefNode::Identifier(ident))
                .ok_or_else(|| unsupported("disable target identifier"))?;
            Ok(Stmt::Disable(name))
        }
        // `disable task;` も同じブロック中断機構で扱う（タスク本体はNamedBlockとして展開されていない場合は no-op）。
        DS::Task(t) => {
            let ident = &t.nodes.1.nodes.0.nodes.2;
            let name = get_id(tree, RefNode::Identifier(ident))
                .ok_or_else(|| unsupported("disable target identifier"))?;
            Ok(Stmt::Disable(name))
        }
        DS::Fork(_) => Err(unsupported("disable fork")),
    }
}

fn lower_par_block(tree: &SyntaxTree, pb: &sv_parser::ParBlock) -> Result<Stmt, FrontendError> {
    // ParBlock.nodes = (fork, Option<label>, Vec<BlockItemDecl>, Vec<StatementOrNull>, JoinKeyword, Option<end label>)
    let mut branches = Vec::new();
    for s in &pb.nodes.3 {
        branches.push(lower_stmt_or_null(tree, s)?);
    }
    Ok(Stmt::Fork(branches))
}

fn lower_loop_stmt(
    tree: &SyntaxTree,
    ls: &sv_parser::LoopStatement,
) -> Result<Stmt, FrontendError> {
    use sv_parser::LoopStatement as LS;
    match ls {
        LS::For(f) => {
            let inner = &f.nodes.1.nodes.1;
            let (var_name, init_stmt) = lower_for_init(tree, inner.0.as_ref())?;
            let cond = if let Some(ce) = &inner.2 {
                lower_expression(tree, ce)?
            } else {
                Expr::Const(lv(1, 1))
            };
            let step_stmt = if let Some(step) = &inner.4 {
                lower_for_step(tree, step)?
            } else {
                Stmt::Block(vec![])
            };
            let body = lower_stmt_or_null(tree, &f.nodes.2)?;
            let var = var_name.unwrap_or_else(|| SmolStr::from("__for_i"));
            Ok(Stmt::For {
                var,
                init: Box::new(init_stmt),
                cond,
                step: Box::new(step_stmt),
                body: Box::new(body),
            })
        }
        _ => Err(unsupported("loop statement variant")),
    }
}

fn lower_for_init(
    tree: &SyntaxTree,
    init: Option<&sv_parser::ForInitialization>,
) -> Result<(Option<SmolStr>, Stmt), FrontendError> {
    use sv_parser::ForInitialization as FI;
    match init {
        None => Ok((None, Stmt::Block(vec![]))),
        Some(FI::ListOfVariableAssignments(lva)) => {
            let mut stmts = Vec::new();
            let mut first_var: Option<SmolStr> = None;
            for ass in lva.nodes.0.contents() {
                let lval = lower_var_lvalue(tree, &ass.nodes.0)?;
                if first_var.is_none() {
                    if let LValue::Net(ref name) = lval {
                        first_var = Some(name.clone());
                    }
                }
                let expr = lower_expression(tree, &ass.nodes.2)?;
                stmts.push(Stmt::BlockingAssign(lval, expr));
            }
            Ok((first_var, Stmt::Block(stmts)))
        }
        Some(FI::Declaration(decl)) => {
            let mut var_name = None;
            let mut stmts = Vec::new();
            for fvd in decl.nodes.0.contents() {
                for (vi, _, init_expr) in fvd.nodes.2.contents() {
                    let name = get_id(tree, RefNode::VariableIdentifier(vi))
                        .unwrap_or_else(|| SmolStr::from("__for_i"));
                    if var_name.is_none() {
                        var_name = Some(name.clone());
                    }
                    let expr = lower_expression(tree, init_expr)?;
                    stmts.push(Stmt::BlockingAssign(LValue::Net(name), expr));
                }
            }
            Ok((var_name, Stmt::Block(stmts)))
        }
    }
}

fn lower_for_step(tree: &SyntaxTree, step: &sv_parser::ForStep) -> Result<Stmt, FrontendError> {
    use sv_parser::ForStepAssignment as FSA;
    let mut stmts = Vec::new();
    for fsa in step.nodes.0.contents() {
        if let FSA::OperatorAssignment(oa) = fsa {
            let lval = lower_var_lvalue(tree, &oa.nodes.0)?;
            let expr = lower_expression(tree, &oa.nodes.2)?;
            stmts.push(Stmt::BlockingAssign(lval, expr));
        }
    }
    Ok(Stmt::Block(stmts))
}

fn lower_seq_block(tree: &SyntaxTree, sb: &sv_parser::SeqBlock) -> Result<Stmt, FrontendError> {
    // SeqBlock.nodes = (begin, Option<(":", label)>, Vec<BlockItemDecl>, Vec<StatementOrNull>, end, ...)
    let mut stmts = Vec::new();
    for s in &sb.nodes.3 {
        stmts.push(lower_stmt_or_null(tree, s)?);
    }
    if let Some((_, label)) = &sb.nodes.1 {
        let name = get_id(tree, RefNode::BlockIdentifier(label))
            .unwrap_or_else(|| SmolStr::from("__block"));
        Ok(Stmt::NamedBlock(name, stmts))
    } else {
        Ok(Stmt::Block(stmts))
    }
}

fn lower_conditional(
    tree: &SyntaxTree,
    cs: &sv_parser::ConditionalStatement,
) -> Result<Stmt, FrontendError> {
    // nodes = (Option<UniquePriority>, "if", Paren<CondPredicate>, StatementOrNull,
    //          Vec<(else_kw, if_kw, Paren<CondPredicate>, StatementOrNull)>, Option<(else_kw, StatementOrNull)>)
    let cond = lower_cond_pred(tree, &cs.nodes.2.nodes.1)?;
    let then_b = lower_stmt_or_null(tree, &cs.nodes.3)?;
    let mut else_b = if let Some((_, else_s)) = &cs.nodes.5 {
        Some(Box::new(lower_stmt_or_null(tree, else_s)?))
    } else {
        None
    };

    for (_, _, else_if_cond, else_if_stmt) in cs.nodes.4.iter().rev() {
        let cond = lower_cond_pred(tree, &else_if_cond.nodes.1)?;
        let then_b = lower_stmt_or_null(tree, else_if_stmt)?;
        else_b = Some(Box::new(Stmt::If(cond, Box::new(then_b), else_b)));
    }

    Ok(Stmt::If(cond, Box::new(then_b), else_b))
}

fn lower_cond_pred(
    tree: &SyntaxTree,
    cp: &sv_parser::CondPredicate,
) -> Result<Expr, FrontendError> {
    // CondPredicate.nodes.0 is List<Symbol, ExpressionOrCondPattern>
    // List.nodes.0 is first item
    let first = &cp.nodes.0.nodes.0;
    match first {
        sv_parser::ExpressionOrCondPattern::Expression(e) => lower_expression(tree, e),
        _ => Err(FrontendError::ParseError("unsupported cond pattern".into())),
    }
}

fn lower_blocking(
    tree: &SyntaxTree,
    ba: &sv_parser::BlockingAssignment,
) -> Result<Stmt, FrontendError> {
    use sv_parser::BlockingAssignment as BA;
    match ba {
        BA::Variable(v) => {
            // BlockingAssignmentVariable.nodes = (VariableLvalue, "=", DelayOrEventControl, Expression)
            let lval = lower_var_lvalue(tree, &v.nodes.0)?;
            let expr = lower_expression(tree, &v.nodes.3)?;
            Ok(Stmt::BlockingAssign(lval, expr))
        }
        BA::OperatorAssignment(oa) => {
            // OperatorAssignment.nodes = (VariableLvalue, AssignmentOperator, Expression)
            let lval = lower_var_lvalue(tree, &oa.nodes.0)?;
            let expr = lower_expression(tree, &oa.nodes.2)?;
            Ok(Stmt::BlockingAssign(lval, expr))
        }
        _ => Err(unsupported("blocking assignment variant")),
    }
}

fn lower_nonblocking(
    tree: &SyntaxTree,
    na: &sv_parser::NonblockingAssignment,
) -> Result<Stmt, FrontendError> {
    // NonblockingAssignment.nodes = (VariableLvalue, "<=", Option<DelayOrEventControl>, Expression)
    let lval = lower_var_lvalue(tree, &na.nodes.0)?;
    let expr = lower_expression(tree, &na.nodes.3)?;
    Ok(Stmt::NbaAssign(lval, expr))
}

fn lower_timing_ctrl(
    tree: &SyntaxTree,
    ptcs: &sv_parser::ProceduralTimingControlStatement,
) -> Result<Stmt, FrontendError> {
    use sv_parser::ProceduralTimingControl as PTC;
    let body = lower_stmt_or_null(tree, &ptcs.nodes.1)?;
    match &ptcs.nodes.0 {
        PTC::DelayControl(dc) => {
            let delay = extract_delay_value(tree, dc);
            Ok(Stmt::Delay(delay, Box::new(body)))
        }
        PTC::EventControl(ec) => {
            let sens = lower_event_control(tree, ec)?;
            Ok(Stmt::EventCtl(sens, Box::new(body)))
        }
        _ => Ok(body),
    }
}

fn extract_delay_value(tree: &SyntaxTree, dc: &sv_parser::DelayControl) -> u64 {
    if let sv_parser::DelayControl::Delay(d) = dc {
        // DelayControlDelay.nodes = (Symbol "#", DelayValue)
        if let Some(text) = tree.get_str_trim(&d.nodes.1) {
            if let Ok(n) = text.trim().parse::<u64>() {
                return n;
            }
        }
    }
    0
}

fn lower_syscall_stmt(
    tree: &SyntaxTree,
    sc: &sv_parser::SubroutineCallStatement,
) -> Result<Stmt, FrontendError> {
    use sv_parser::SubroutineCallStatement as SCS;
    match sc {
        SCS::SubroutineCall(sub) => lower_subcall(tree, &sub.0),
        _ => Err(unsupported("subroutine call statement")),
    }
}

fn lower_subcall(tree: &SyntaxTree, sc: &sv_parser::SubroutineCall) -> Result<Stmt, FrontendError> {
    match sc {
        sv_parser::SubroutineCall::SystemTfCall(sys) => lower_system_task(tree, sys),
        sv_parser::SubroutineCall::TfCall(tf) => {
            let name = get_id(tree, RefNode::PsOrHierarchicalTfIdentifier(&tf.nodes.0))
                .ok_or_else(|| FrontendError::ParseError("task call name missing".into()))?;
            let args = collect_tf_call_args(tree, tf);
            Ok(Stmt::TaskCall(name, args))
        }
        _ => Err(unsupported("non-tf subroutine call")),
    }
}

fn collect_tf_call_args(tree: &SyntaxTree, tf: &sv_parser::TfCall) -> Vec<Expr> {
    let mut args = Vec::new();
    if let Some(paren) = &tf.nodes.2 {
        if let sv_parser::ListOfArguments::Ordered(o) = &paren.nodes.1 {
            for e in o.nodes.0.contents().into_iter().flatten() {
                if let Ok(expr) = lower_expression(tree, e) {
                    args.push(expr);
                }
            }
        }
    }
    args
}

fn system_tf_name<'a>(tree: &'a SyntaxTree, sys: &'a sv_parser::SystemTfCall) -> &'a str {
    let name_node = match sys {
        sv_parser::SystemTfCall::ArgOptionl(s) => RefNode::SystemTfIdentifier(&s.nodes.0),
        sv_parser::SystemTfCall::ArgExpression(s) => RefNode::SystemTfIdentifier(&s.nodes.0),
        sv_parser::SystemTfCall::ArgDataType(s) => RefNode::SystemTfIdentifier(&s.nodes.0),
    };
    if let RefNode::SystemTfIdentifier(tf) = name_node {
        tree.get_str(&tf.nodes.0).unwrap_or("?")
    } else {
        "?"
    }
}

fn lower_system_task(
    tree: &SyntaxTree,
    sys: &sv_parser::SystemTfCall,
) -> Result<Stmt, FrontendError> {
    let name_text = system_tf_name(tree, sys);

    let task = match name_text {
        "$display" => SysTask::Display,
        "$write" => SysTask::Write,
        "$monitor" => SysTask::Monitor,
        "$finish" => SysTask::Finish,
        "$time" => SysTask::Time,
        "$dumpfile" => SysTask::DumpFile,
        "$dumpvars" => SysTask::DumpVars,
        "$readmemh" => SysTask::ReadMemH,
        "$readmemb" => SysTask::ReadMemB,
        n => return Err(unsupported(&format!("system task {}", n))),
    };

    let args = collect_syscall_args(tree, sys);
    Ok(Stmt::SysCall(task, args))
}

fn collect_syscall_args(tree: &SyntaxTree, sys: &sv_parser::SystemTfCall) -> Vec<Expr> {
    let mut args = Vec::new();
    match sys {
        sv_parser::SystemTfCall::ArgExpression(s) => {
            // nodes.1 is Paren<(List<Symbol, Option<Expression>>, ...)>
            // .nodes.1 is the inner tuple, .0 is the List
            let list = &s.nodes.1.nodes.1 .0;
            for e in list.contents().into_iter().flatten() {
                if let Ok(expr) = lower_expression(tree, e) {
                    args.push(expr);
                }
            }
        }
        sv_parser::SystemTfCall::ArgOptionl(s) => {
            if let Some(paren) = &s.nodes.1 {
                // Paren.nodes.1 is ListOfArguments
                if let sv_parser::ListOfArguments::Ordered(o) = &paren.nodes.1 {
                    // ListOfArgumentsOrdered.nodes.0 is List<Symbol, Option<Expression>>
                    for e in o.nodes.0.contents().into_iter().flatten() {
                        if let Ok(expr) = lower_expression(tree, e) {
                            args.push(expr);
                        }
                    }
                }
            }
        }
        _ => {}
    }
    args
}

fn lower_case(tree: &SyntaxTree, cs: &sv_parser::CaseStatement) -> Result<Stmt, FrontendError> {
    use sv_parser::CaseStatement as CS;
    if let CS::Normal(n) = cs {
        let kind = match &n.nodes.1 {
            sv_parser::CaseKeyword::Case(_) => CaseKind::Case,
            sv_parser::CaseKeyword::Casez(_) => CaseKind::CaseZ,
            sv_parser::CaseKeyword::Casex(_) => CaseKind::CaseX,
        };
        // n.nodes.2 is Paren<CaseExpression>; CaseExpression.nodes.0 is Expression
        let sel = lower_expression(tree, &n.nodes.2.nodes.1.nodes.0)?;

        let mut arms: Vec<(Vec<Expr>, Box<Stmt>)> = Vec::new();
        let mut default: Option<Box<Stmt>> = None;

        // n.nodes.3 is first CaseItem, n.nodes.4 is Vec<CaseItem>
        let mut all_items = vec![&n.nodes.3];
        all_items.extend(n.nodes.4.iter());

        for item in all_items {
            match item {
                sv_parser::CaseItem::NonDefault(nd) => {
                    // CaseItemNondefault.nodes = (List<Symbol, CaseItemExpression>, Symbol, StatementOrNull)
                    let patterns: Result<Vec<_>, _> = nd
                        .nodes
                        .0
                        .contents()
                        .into_iter()
                        .map(|cie| lower_expression(tree, &cie.nodes.0))
                        .collect();
                    let body = lower_stmt_or_null(tree, &nd.nodes.2)?;
                    arms.push((patterns?, Box::new(body)));
                }
                sv_parser::CaseItem::Default(d) => {
                    // CaseItemDefault.nodes = (Keyword, Option<Symbol>, StatementOrNull)
                    default = Some(Box::new(lower_stmt_or_null(tree, &d.nodes.2)?));
                }
            }
        }
        return Ok(Stmt::Case {
            sel,
            arms,
            default,
            kind,
        });
    }
    Err(unsupported("case statement variant"))
}

// ── lvalue ────────────────────────────────────────────────────────────────────

fn lower_var_lvalue(
    tree: &SyntaxTree,
    lv: &sv_parser::VariableLvalue,
) -> Result<LValue, FrontendError> {
    use sv_parser::VariableLvalue as VL;
    match lv {
        VL::Identifier(h) => {
            if let Some(name) = get_id(tree, RefNode::VariableLvalueIdentifier(h)) {
                // Check for array index select: mem[idx]
                // h.nodes.2 = Select, h.nodes.2.nodes.1 = BitSelect, .nodes.0 = Vec<Bracket<Expr>>
                let bit_selects = &h.nodes.2.nodes.1.nodes.0;
                if let Some(bracket) = bit_selects.first() {
                    let idx_expr = lower_expression(tree, &bracket.nodes.1)?;
                    return Ok(LValue::IndexSel(name, Box::new(idx_expr)));
                }
                // part select: net[hi:lo] (h.nodes.2.nodes.2 = Option<Bracket<PartSelectRange>>)
                if let Some(bracket) = &h.nodes.2.nodes.2 {
                    return lower_lvalue_part_select(tree, name, &bracket.nodes.1);
                }
                return Ok(LValue::Net(name));
            }
            Err(FrontendError::ParseError(
                "lvalue identifier missing".into(),
            ))
        }
        VL::Lvalue(c) => {
            // Brace<List<Symbol, VariableLvalue>>
            let list = &c.nodes.0.nodes.1;
            let mut parts: Vec<LValue> = Vec::new();
            for lv_inner in list.contents() {
                parts.push(lower_var_lvalue(tree, lv_inner)?);
            }
            if parts.is_empty() {
                return Err(FrontendError::ParseError("empty concat lvalue".into()));
            }
            Ok(LValue::Concat(parts))
        }
        _ => Err(unsupported("variable lvalue")),
    }
}

/// `net[hi:lo] <= ...` の定数レンジ部分選択を HIR の lvalue `PartSelect` に落とす。
/// `+:` / `-:`（IndexedRange）は未対応（黙って全ビットにフォールバックさせない）。
fn lower_lvalue_part_select(
    tree: &SyntaxTree,
    name: SmolStr,
    psr: &sv_parser::PartSelectRange,
) -> Result<LValue, FrontendError> {
    match psr {
        sv_parser::PartSelectRange::ConstantRange(cr) => {
            let left = lower_const_expr(tree, &cr.nodes.0)?;
            let right = lower_const_expr(tree, &cr.nodes.2)?;
            Ok(LValue::PartSelect(
                Box::new(LValue::Net(name)),
                Range {
                    left: Box::new(left),
                    right: Box::new(right),
                },
            ))
        }
        sv_parser::PartSelectRange::IndexedRange(ir) => {
            // ir.nodes = (Expression /*base*/, Symbol /*"+:" or "-:"*/, ConstantExpression /*width*/)
            let base = lower_expression(tree, &ir.nodes.0)?;
            let width = lower_const_expr(tree, &ir.nodes.2)?;
            let plus_dir = tree.get_str(&ir.nodes.1).unwrap_or("+:").trim() == "+:";
            Ok(LValue::IndexedPartSelect(
                Box::new(LValue::Net(name)),
                Box::new(base),
                Box::new(width),
                plus_dir,
            ))
        }
    }
}

// ── expression lowering ───────────────────────────────────────────────────────

fn lower_expression(
    tree: &SyntaxTree,
    expr: &sv_parser::Expression,
) -> Result<Expr, FrontendError> {
    use sv_parser::Expression as E;
    match expr {
        E::Primary(p) => lower_primary(tree, p),
        E::Unary(u) => {
            let op = lower_unary_op(tree, &u.nodes.0)?;
            let inner = lower_primary(tree, &u.nodes.2)?;
            Ok(Expr::Un(op, Box::new(inner)))
        }
        E::Binary(_) => {
            // sv-parser の Expression::Binary は演算子優先順位を考慮せず、パック
            // ラット左再帰の種growingにより常に右結合の木（a op1 (b op2 c) ...）を
            // 返す（`tree.get_str` で実機確認済み、PLAN.md 実装課題 A10 参照）。
            // ここで一旦フラットな (演算子列, オペランド列) に展開し直し、
            // Verilog の演算子優先順位表（IEEE 1364-2001 Table 5-4）に基づく
            // 演算子優先順位法（shunting-yard 相当）で正しく再結合する。
            let mut operands = Vec::new();
            let mut ops = Vec::new();
            let tail = flatten_binary_chain(tree, expr, &mut operands, &mut ops)?;
            let combined = build_binop_tree(operands, ops);
            match tail {
                Some((then_e, else_e)) => Ok(Expr::Cond(
                    Box::new(combined),
                    Box::new(then_e),
                    Box::new(else_e),
                )),
                None => Ok(combined),
            }
        }
        E::ConditionalExpression(ce) => {
            // ConditionalExpression.nodes = (CondPredicate, "?", Vec<Attr>, Expression, ":", Expression)
            let cond = lower_cond_pred(tree, &ce.nodes.0)?;
            let then_e = lower_expression(tree, &ce.nodes.3)?;
            let else_e = lower_expression(tree, &ce.nodes.5)?;
            Ok(Expr::Cond(
                Box::new(cond),
                Box::new(then_e),
                Box::new(else_e),
            ))
        }
        _ => Err(unsupported("expression kind")),
    }
}

/// `Expression::Binary` の木を in-order にたどり、オペランド列と演算子列に
/// フラット化する。チェーン末尾の裸の条件演算子は、条件部を最後の
/// オペランドとして加え、then/else 部を戻り値で呼び出し元へ返す。これにより
/// `a || b ? c : d` を `(a || b) ? c : d` として再構築する。
/// 括弧で明示的にグループ化された部分式は `Primary::MintypmaxExpression` として
/// リーフ扱いになるため、フラット化の対象にならず正しく1オペランドとして扱う。
fn flatten_binary_chain(
    tree: &SyntaxTree,
    expr: &sv_parser::Expression,
    operands: &mut Vec<Expr>,
    ops: &mut Vec<BinOp>,
) -> Result<Option<(Expr, Expr)>, FrontendError> {
    use sv_parser::Expression as E;
    match expr {
        E::Binary(b) => {
            let left_tail = flatten_binary_chain(tree, &b.nodes.0, operands, ops)?;
            if left_tail.is_some() {
                return Err(FrontendError::ParseError(
                    "二項演算子チェーンの左オペランドに予期しない三項演算子があります".into(),
                ));
            }
            ops.push(lower_binary_op(tree, &b.nodes.1)?);
            flatten_binary_chain(tree, &b.nodes.3, operands, ops)
        }
        E::ConditionalExpression(ce) => {
            let cond = lower_cond_pred(tree, &ce.nodes.0)?;
            operands.push(cond);
            let then_e = lower_expression(tree, &ce.nodes.3)?;
            let else_e = lower_expression(tree, &ce.nodes.5)?;
            Ok(Some((then_e, else_e)))
        }
        other => {
            operands.push(lower_expression(tree, other)?);
            Ok(None)
        }
    }
}

/// `ConstantExpression::Binary` についても、チェーン末尾の裸の三項演算子を
/// 二項演算子チェーンの外側へ持ち上げ、通常の式と同じ優先順位を保つ。
fn flatten_constant_binary_chain(
    tree: &SyntaxTree,
    ce: &sv_parser::ConstantExpression,
    operands: &mut Vec<Expr>,
    ops: &mut Vec<BinOp>,
) -> Result<Option<(Expr, Expr)>, FrontendError> {
    use sv_parser::ConstantExpression as CE;
    match ce {
        CE::Binary(b) => {
            let left_tail = flatten_constant_binary_chain(tree, &b.nodes.0, operands, ops)?;
            if left_tail.is_some() {
                return Err(FrontendError::ParseError(
                    "定数二項演算子チェーンの左オペランドに予期しない三項演算子があります".into(),
                ));
            }
            ops.push(lower_binary_op(tree, &b.nodes.1)?);
            flatten_constant_binary_chain(tree, &b.nodes.3, operands, ops)
        }
        CE::Ternary(t) => {
            let cond = lower_constant_expr(tree, &t.nodes.0)?;
            operands.push(cond);
            let then_e = lower_constant_expr(tree, &t.nodes.3)?;
            let else_e = lower_constant_expr(tree, &t.nodes.5)?;
            Ok(Some((then_e, else_e)))
        }
        other => {
            operands.push(lower_constant_expr(tree, other)?);
            Ok(None)
        }
    }
}

/// IEEE 1364-2001 Table 5-4 の演算子優先順位（数値が大きいほど強く結合）。
/// `**` は本サブセット未対応のためここには含まない。
fn binop_precedence(op: BinOp) -> u8 {
    use BinOp::*;
    match op {
        Mul | Div | Mod => 10,
        Add | Sub => 9,
        Shl | Shr | Ashl | Ashr => 8,
        Lt | Le | Gt | Ge => 7,
        Eq | Ne | CaseEq | CaseNe => 6,
        BitAnd => 5,
        BitXor | BitXnor | BitNand | BitNor => 4,
        BitOr => 3,
        LogAnd => 2,
        LogOr => 1,
    }
}

/// フラット化されたオペランド列・演算子列から、演算子優先順位法
/// （全演算子は左結合）で正しい二分木を再構築する。
/// 事前条件: `operands.len() == ops.len() + 1`。
fn build_binop_tree(operands: Vec<Expr>, ops: Vec<BinOp>) -> Expr {
    let mut operands = operands.into_iter();
    let mut expr_stack: Vec<Expr> =
        vec![operands.next().expect("flatten always yields >=1 operand")];
    let mut op_stack: Vec<BinOp> = Vec::new();
    for op in ops {
        while let Some(&top_op) = op_stack.last() {
            if binop_precedence(top_op) >= binop_precedence(op) {
                let top_op = op_stack.pop().unwrap();
                let rhs = expr_stack.pop().unwrap();
                let lhs = expr_stack.pop().unwrap();
                expr_stack.push(Expr::Bin(top_op, Box::new(lhs), Box::new(rhs)));
            } else {
                break;
            }
        }
        op_stack.push(op);
        expr_stack.push(operands.next().expect("flatten operand/op count mismatch"));
    }
    while let Some(op) = op_stack.pop() {
        let rhs = expr_stack.pop().unwrap();
        let lhs = expr_stack.pop().unwrap();
        expr_stack.push(Expr::Bin(op, Box::new(lhs), Box::new(rhs)));
    }
    expr_stack
        .pop()
        .expect("expr_stack must have exactly one element")
}

fn lower_primary(tree: &SyntaxTree, p: &sv_parser::Primary) -> Result<Expr, FrontendError> {
    use sv_parser::Primary as P;
    match p {
        P::Hierarchical(h) => {
            if let Some(name) = get_id(tree, RefNode::PrimaryHierarchical(h)) {
                // Check for array index select: mem[idx] or bit select: net[idx]
                // h.nodes.2 = Select, h.nodes.2.nodes.1 = BitSelect, .nodes.0 = Vec<Bracket<Expr>>
                let bit_selects = &h.nodes.2.nodes.1.nodes.0;
                if let Some(bracket) = bit_selects.first() {
                    let idx_expr = lower_expression(tree, &bracket.nodes.1)?;
                    return Ok(Expr::IndexSel(name, Box::new(idx_expr)));
                }
                // part select: net[hi:lo] (h.nodes.2.nodes.2 = Option<Bracket<PartSelectRange>>)
                if let Some(bracket) = &h.nodes.2.nodes.2 {
                    return lower_part_select(tree, name, &bracket.nodes.1);
                }
                Ok(Expr::Net(name))
            } else {
                Err(FrontendError::ParseError(
                    "identifier in expression missing".into(),
                ))
            }
        }
        P::PrimaryLiteral(lit) => lower_primary_literal(tree, lit),
        P::Concatenation(c) => {
            // Brace<List<Symbol, Expression>> の直接の子 Expression のみを parts にする。
            // （subtree 全走査ではビット選択のインデックス式などネストした式まで混入する）
            let brace = &c.nodes.0;
            let list = &brace.nodes.0.nodes.1;
            let mut parts = Vec::new();
            for e in list.contents() {
                parts.push(lower_expression(tree, e)?);
            }
            Ok(Expr::Concat(parts))
        }
        P::MultipleConcatenation(mc) => {
            let mca = mc.as_ref();
            let count_expr = if let Some(RefNode::ConstantExpression(ce)) =
                unwrap_node!(mca, ConstantExpression)
            {
                lower_const_expr(tree, ce)?
            } else {
                Expr::Const(lv(1, 32))
            };
            let inner = if let Some(RefNode::Expression(e)) = unwrap_node!(mca, Expression) {
                lower_expression(tree, e)?
            } else {
                Expr::Const(lv(0, 1))
            };
            Ok(Expr::Repeat(Box::new(count_expr), vec![inner]))
        }
        P::FunctionSubroutineCall(fsc) => match &fsc.nodes.0 {
            sv_parser::SubroutineCall::TfCall(tf) => {
                let name = get_id(tree, RefNode::PsOrHierarchicalTfIdentifier(&tf.nodes.0))
                    .ok_or_else(|| {
                        FrontendError::ParseError("function call name missing".into())
                    })?;
                let args = collect_tf_call_args(tree, tf);
                Ok(Expr::Call(name, args))
            }
            sv_parser::SubroutineCall::SystemTfCall(sys) => {
                let name_text = system_tf_name(tree, sys);
                match name_text {
                    "$random" => {
                        let args = collect_syscall_args(tree, sys);
                        Ok(Expr::SysFunc(SysFuncKind::Random, args))
                    }
                    "$signed" | "$unsigned" => {
                        let args = collect_syscall_args(tree, sys);
                        if args.len() != 1 {
                            return Err(FrontendError::ParseError(format!(
                                "{} requires exactly 1 argument",
                                name_text
                            )));
                        }
                        let kind = if name_text == "$signed" {
                            SysFuncKind::Signed
                        } else {
                            SysFuncKind::Unsigned
                        };
                        Ok(Expr::SysFunc(kind, args))
                    }
                    n => Err(unsupported(&format!("system function {} in expression", n))),
                }
            }
            _ => Err(unsupported("non-tf subroutine call in expression")),
        },
        P::MintypmaxExpression(m) => {
            let ma = m.as_ref();
            if let Some(RefNode::Expression(e)) = unwrap_node!(ma, Expression) {
                lower_expression(tree, e)
            } else {
                Err(FrontendError::ParseError("paren expression missing".into()))
            }
        }
        _ => Err(unsupported("primary")),
    }
}

/// `net[hi:lo]` の定数レンジ部分選択を HIR の PartSel に落とす。
/// `+:` / `-:`（IndexedRange）は未対応（黙って全ビットにフォールバックさせない）。
fn lower_part_select(
    tree: &SyntaxTree,
    name: SmolStr,
    psr: &sv_parser::PartSelectRange,
) -> Result<Expr, FrontendError> {
    match psr {
        sv_parser::PartSelectRange::ConstantRange(cr) => {
            let left = lower_const_expr(tree, &cr.nodes.0)?;
            let right = lower_const_expr(tree, &cr.nodes.2)?;
            Ok(Expr::PartSel(
                Box::new(Expr::Net(name)),
                Box::new(Range {
                    left: Box::new(left),
                    right: Box::new(right),
                }),
            ))
        }
        sv_parser::PartSelectRange::IndexedRange(ir) => {
            // ir.nodes = (Expression /*base*/, Symbol /*"+:" or "-:"*/, ConstantExpression /*width*/)
            let base = lower_expression(tree, &ir.nodes.0)?;
            let width = lower_const_expr(tree, &ir.nodes.2)?;
            let plus_dir = tree.get_str(&ir.nodes.1).unwrap_or("+:").trim() == "+:";
            Ok(Expr::IndexedPartSel(
                Box::new(Expr::Net(name)),
                Box::new(base),
                Box::new(width),
                plus_dir,
            ))
        }
    }
}

fn lower_primary_literal(
    tree: &SyntaxTree,
    lit: &sv_parser::PrimaryLiteral,
) -> Result<Expr, FrontendError> {
    use sv_parser::PrimaryLiteral as PL;
    match lit {
        PL::Number(n) => lower_number(tree, n),
        PL::StringLiteral(s) => {
            let raw = tree.get_str(s).unwrap_or("\"\"");
            // Strip surrounding quotes
            let inner = raw.trim_start_matches('"').trim_end_matches('"');
            Ok(Expr::StringLit(SmolStr::from(inner)))
        }
        _ => Err(unsupported("primary literal")),
    }
}

fn lower_number(tree: &SyntaxTree, n: &sv_parser::Number) -> Result<Expr, FrontendError> {
    let text = tree.get_str(n).unwrap_or("0");
    let (v, signed) = parse_number_text(text.trim());
    Ok(if signed {
        Expr::SignedConst(v)
    } else {
        Expr::Const(v)
    })
}

fn lower_const_expr(
    tree: &SyntaxTree,
    ce: &sv_parser::ConstantExpression,
) -> Result<Expr, FrontendError> {
    if let Some(text) = tree.get_str(ce) {
        let (v, signed) = parse_number_text(text.trim());
        return Ok(if signed {
            Expr::SignedConst(v)
        } else {
            Expr::Const(v)
        });
    }
    Ok(Expr::Const(lv(1, 32)))
}

/// リテラルの値と、signed文脈で扱うべきか（IEEE 1364-2001 4.8）を返す。
/// 符号無し10進の即値（`-7`, `2` 等、`'`基数指定なし）と `'s` 基数指定
/// （`4'sd5` 等）はsigned、それ以外の基数付きリテラルはunsigned。
fn parse_number_text(text: &str) -> (LogicVal, bool) {
    if let Some(tick) = text.find('\'') {
        let size: u32 = text[..tick].trim().parse().unwrap_or(32);
        let rest = &text[tick + 1..];
        let (signed, rest) = match rest.strip_prefix(['s', 'S']) {
            Some(r) => (true, r),
            None => (false, rest),
        };
        let (base, digits) = match rest.chars().next().unwrap_or('d') {
            'd' | 'D' => (10u32, &rest[1..]),
            'h' | 'H' => (16u32, &rest[1..]),
            'b' | 'B' => (2u32, &rest[1..]),
            'o' | 'O' => (8u32, &rest[1..]),
            _ => (10u32, rest),
        };
        let clean: String = digits.chars().filter(|c| *c != '_').collect();
        (parse_based_digits(&clean, base, size), signed)
    } else {
        let val: i64 = text.trim().parse().unwrap_or(0);
        (lv(val as u64, 32), true)
    }
}

/// 基数付きリテラル（`8'b1010xxxx`等）の数字部分をbit単位でX/Zを保持してパースする。
/// 2進/8進/16進は桁ごとにX/Zを展開し、10進は値全体がx/zの場合のみ対応する
/// （10進では桁ごとのx/z混在はVerilog仕様上存在しない）。
fn parse_based_digits(digits: &str, radix: u32, width: u32) -> LogicVal {
    let n_chunks = (width as usize).div_ceil(64);
    if radix == 10 {
        if digits.eq_ignore_ascii_case("x") {
            return LogicVal::from_chunks(
                width,
                &vec![u64::MAX; n_chunks],
                &vec![u64::MAX; n_chunks],
            );
        }
        if digits.eq_ignore_ascii_case("z") || digits == "?" {
            return LogicVal::from_chunks(width, &vec![0u64; n_chunks], &vec![u64::MAX; n_chunks]);
        }
        let val = u64::from_str_radix(digits, radix).unwrap_or(0);
        return LogicVal::from_chunks(width, &[val], &vec![0u64; n_chunks]);
    }
    let bits_per_digit = match radix {
        2 => 1,
        8 => 3,
        16 => 4,
        _ => 1,
    };
    let mut a = vec![0u64; n_chunks];
    let mut b = vec![0u64; n_chunks];
    let mut bitpos: usize = 0;
    for ch in digits.chars().rev() {
        if bitpos >= width as usize {
            break;
        }
        let digit_bit: Box<dyn Fn(u32) -> (u64, u64)> = match ch {
            'x' | 'X' => Box::new(|_| (1, 1)),
            'z' | 'Z' | '?' => Box::new(|_| (0, 1)),
            _ => match ch.to_digit(radix) {
                Some(d) => Box::new(move |bit| (((d as u64) >> bit) & 1, 0)),
                None => continue,
            },
        };
        for bit in 0..bits_per_digit {
            if bitpos >= width as usize {
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
    LogicVal::from_chunks(width, &a, &b)
}

fn lower_unary_op(tree: &SyntaxTree, op: &sv_parser::UnaryOperator) -> Result<UnOp, FrontendError> {
    let text = tree.get_str(op).unwrap_or("?");
    match text.trim() {
        "+" => Ok(UnOp::Pos),
        "-" => Ok(UnOp::Neg),
        "!" => Ok(UnOp::LogNot),
        "~" => Ok(UnOp::BitNot),
        "&" => Ok(UnOp::RedAnd),
        "~&" => Ok(UnOp::RedNand),
        "|" => Ok(UnOp::RedOr),
        "~|" => Ok(UnOp::RedNor),
        "^" => Ok(UnOp::RedXor),
        "~^" | "^~" => Ok(UnOp::RedXnor),
        other => Err(FrontendError::UnsupportedConstruct(format!(
            "unary operator: {other}"
        ))),
    }
}

fn lower_binary_op(
    tree: &SyntaxTree,
    op: &sv_parser::BinaryOperator,
) -> Result<BinOp, FrontendError> {
    let text = tree.get_str(op).unwrap_or("?");
    match text.trim() {
        "+" => Ok(BinOp::Add),
        "-" => Ok(BinOp::Sub),
        "*" => Ok(BinOp::Mul),
        "/" => Ok(BinOp::Div),
        "%" => Ok(BinOp::Mod),
        "&&" => Ok(BinOp::LogAnd),
        "||" => Ok(BinOp::LogOr),
        "&" => Ok(BinOp::BitAnd),
        "|" => Ok(BinOp::BitOr),
        "^" => Ok(BinOp::BitXor),
        "~&" => Ok(BinOp::BitNand),
        "~|" => Ok(BinOp::BitNor),
        "~^" | "^~" => Ok(BinOp::BitXnor),
        "==" => Ok(BinOp::Eq),
        "!=" => Ok(BinOp::Ne),
        "===" => Ok(BinOp::CaseEq),
        "!==" => Ok(BinOp::CaseNe),
        "<" => Ok(BinOp::Lt),
        ">" => Ok(BinOp::Gt),
        "<=" => Ok(BinOp::Le),
        ">=" => Ok(BinOp::Ge),
        "<<" => Ok(BinOp::Shl),
        ">>" => Ok(BinOp::Shr),
        "<<<" => Ok(BinOp::Ashl),
        ">>>" => Ok(BinOp::Ashr),
        t => Err(unsupported(&format!("binary op '{}'", t))),
    }
}

// ── module instantiation ──────────────────────────────────────────────────────

fn lower_module_inst(
    tree: &SyntaxTree,
    mi: &sv_parser::ModuleInstantiation,
) -> Result<ModuleInstance, FrontendError> {
    // ModuleInstantiation.nodes = (ModuleIdentifier, Option<ParameterValueAssignment>, List<Symbol, HierarchicalInstance>, Symbol)
    let module = get_id(tree, RefNode::ModuleIdentifier(&mi.nodes.0))
        .ok_or_else(|| FrontendError::ParseError("module name missing".into()))?;

    let params = lower_param_overrides(tree, &mi.nodes.1)?;

    // List<Symbol, HierarchicalInstance> - take first instance
    let hi = &mi.nodes.2.nodes.0;
    let name =
        get_id(tree, RefNode::NameOfInstance(&hi.nodes.0)).unwrap_or_else(|| SmolStr::new("inst"));
    let ports = lower_port_connections(tree, &hi.nodes.1.nodes.1)?;

    Ok(ModuleInstance {
        name,
        module,
        params,
        ports,
    })
}

fn lower_param_overrides(
    tree: &SyntaxTree,
    pva: &Option<sv_parser::ParameterValueAssignment>,
) -> Result<Vec<ParamOverride>, FrontendError> {
    let mut out = Vec::new();
    if let Some(pv) = pva {
        // ParameterValueAssignment.nodes = (Symbol "#", Paren<Option<ListOfParameterAssignments>>)
        if let Some(list) = &pv.nodes.1.nodes.1 {
            match list {
                sv_parser::ListOfParameterAssignments::Named(n) => {
                    for item in n.nodes.0.contents() {
                        let pname = get_id(tree, RefNode::ParameterIdentifier(&item.nodes.1));
                        // item.nodes.2 = Paren<Option<ParamExpression>>
                        let value = if let Some(pe) = &item.nodes.2.nodes.1 {
                            lower_param_expr(tree, pe)?
                        } else {
                            Expr::Const(lv(0, 32))
                        };
                        out.push(ParamOverride { name: pname, value });
                    }
                }
                sv_parser::ListOfParameterAssignments::Ordered(o) => {
                    for item in o.nodes.0.contents() {
                        let value = lower_param_expr(tree, &item.nodes.0)?;
                        out.push(ParamOverride { name: None, value });
                    }
                }
            }
        }
    }
    Ok(out)
}

fn lower_port_connections(
    tree: &SyntaxTree,
    conns: &Option<sv_parser::ListOfPortConnections>,
) -> Result<Vec<PortConnection>, FrontendError> {
    let mut out = Vec::new();
    if let Some(c) = conns {
        match c {
            sv_parser::ListOfPortConnections::Named(n) => {
                for conn in n.nodes.0.contents() {
                    if let sv_parser::NamedPortConnection::Identifier(id) = conn {
                        let port_name = get_id(tree, RefNode::PortIdentifier(&id.nodes.2));
                        let expr = if let Some(paren) = &id.nodes.3 {
                            // Paren<Option<Expression>>.nodes.1 is Option<Expression>
                            if let Some(e) = &paren.nodes.1 {
                                lower_expression(tree, e).unwrap_or(Expr::Const(lv(0, 1)))
                            } else {
                                Expr::Const(lv(0, 1))
                            }
                        } else {
                            Expr::Const(lv(0, 1))
                        };
                        out.push(PortConnection {
                            name: port_name,
                            expr,
                        });
                    }
                }
            }
            sv_parser::ListOfPortConnections::Ordered(o) => {
                for conn in o.nodes.0.contents() {
                    let expr = if let Some(e) = &conn.nodes.1 {
                        lower_expression(tree, e).unwrap_or(Expr::Const(lv(0, 1)))
                    } else {
                        Expr::Const(lv(0, 1))
                    };
                    out.push(PortConnection { name: None, expr });
                }
            }
        }
    }
    Ok(out)
}
