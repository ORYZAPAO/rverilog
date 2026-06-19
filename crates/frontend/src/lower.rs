use sv_parser::{parse_sv, unwrap_node, Define, DefineText, RefNode, SyntaxTree};
use std::collections::HashMap;
use std::path::PathBuf;
use rverilog_hir::{
    AlwaysConstruct, BinOp, CaseKind, ContinuousAssign, Design, EdgeType, Expr, HirModule,
    InitialConstruct, LValue, LocalParamDecl, MemDecl, ModuleInstance, NetDecl, NetKind,
    ParamDecl, ParamOverride, PortConnection, PortDecl, PortDirection, RegDecl, Sensitivity,
    SensitivityItem, Stmt, SysTask, SysFuncKind, UnOp,
};
use rverilog_mir::LogicVal;
use smol_str::SmolStr;
use indexmap::IndexMap;
use crate::error::FrontendError;

// ── helpers ───────────────────────────────────────────────────────────────────

fn lv(v: u64, w: u16) -> LogicVal {
    LogicVal::new(w, v, 0)
}

fn lv_x(w: u16) -> LogicVal {
    let mask = if w == 64 { u64::MAX } else { (1u64 << w) - 1 };
    LogicVal::new(w, mask, mask)
}

fn get_id(tree: &SyntaxTree, node: RefNode) -> Option<SmolStr> {
    match unwrap_node!(node, SimpleIdentifier, EscapedIdentifier) {
        Some(RefNode::SimpleIdentifier(x)) => Some(SmolStr::new(tree.get_str(&x.nodes.0)?)),
        Some(RefNode::EscapedIdentifier(x)) => Some(SmolStr::new(tree.get_str(&x.nodes.0)?)),
        _ => None,
    }
}

fn parse_packed_dim(s: &str) -> u32 {
    let s = s.trim().trim_start_matches('[').trim_end_matches(']');
    let mut it = s.splitn(2, ':');
    let msb: i64 = it.next().and_then(|x| x.trim().parse().ok()).unwrap_or(0);
    let lsb: i64 = it.next().and_then(|x| x.trim().parse().ok()).unwrap_or(0);
    (msb - lsb).unsigned_abs() as u32 + 1
}

fn packed_width(tree: &SyntaxTree, node: RefNode) -> u32 {
    if let Some(RefNode::PackedDimension(pd)) = unwrap_node!(node, PackedDimension) {
        if let Some(t) = tree.get_str(pd) {
            return parse_packed_dim(t);
        }
    }
    1
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
    let mut design = Design { modules: IndexMap::new() };

    let mut defs: HashMap<String, Option<Define>> = HashMap::new();
    for (k, v) in defines {
        defs.insert(k.clone(), Some(Define::new(k.clone(), vec![], Some(DefineText::new(v.clone(), None)))));
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
    let (nets, regs, mems, locals, assigns, initials, alwayses, instances) = lower_nonport_items(tree, &x.nodes.2)?;

    Ok(HirModule { name, ports, params, locals, nets, regs, mems, assigns, initials, alwayses, instances })
}

fn lower_module_nonansi(
    tree: &SyntaxTree,
    x: &sv_parser::ModuleDeclarationNonansi,
) -> Result<HirModule, FrontendError> {
    let header = &x.nodes.0;
    let name = get_id(tree, RefNode::ModuleIdentifier(&header.nodes.3))
        .ok_or_else(|| FrontendError::ParseError("missing module name".into()))?;

    let (nets, regs, mems, locals, assigns, initials, alwayses, instances) = lower_module_items(tree, &x.nodes.2)?;

    Ok(HirModule { name, ports: vec![], params: vec![], locals, nets, regs, mems, assigns, initials, alwayses, instances })
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
            ports.push(lower_ansi_port(tree, &first.1)?);
            for (_, item) in &list.nodes.1 {
                ports.push(lower_ansi_port(tree, &item.1)?);
            }
        }
    }
    Ok(ports)
}

fn lower_ansi_port(
    tree: &SyntaxTree,
    port: &sv_parser::AnsiPortDeclaration,
) -> Result<PortDecl, FrontendError> {
    match port {
        sv_parser::AnsiPortDeclaration::Net(n) => lower_ansi_port_net(tree, n),
        sv_parser::AnsiPortDeclaration::Variable(v) => lower_ansi_port_variable(tree, v),
        sv_parser::AnsiPortDeclaration::Paren(p) => {
            let dir = match &p.nodes.0 {
                Some(d) => lower_port_direction(d),
                None => PortDirection::Input,
            };
            let name = get_id(tree, RefNode::PortIdentifier(&p.nodes.2))
                .ok_or_else(|| FrontendError::ParseError("port name missing".into()))?;
            Ok(PortDecl { name, direction: dir, width: 1, width_expr: Expr::Const(lv(1, 32)) })
        }
    }
}

fn lower_ansi_port_net(
    tree: &SyntaxTree,
    x: &sv_parser::AnsiPortDeclarationNet,
) -> Result<PortDecl, FrontendError> {
    let dir = if let Some(hdr) = &x.nodes.0 {
        match hdr {
            sv_parser::NetPortHeaderOrInterfacePortHeader::NetPortHeader(h) => {
                h.nodes.0.as_ref().map(lower_port_direction).unwrap_or(PortDirection::Input)
            }
            _ => PortDirection::Input,
        }
    } else {
        PortDirection::Input
    };
    let (width, width_expr) = packed_width_expr(tree, RefNode::AnsiPortDeclarationNet(x));
    let name = get_id(tree, RefNode::PortIdentifier(&x.nodes.1))
        .ok_or_else(|| FrontendError::ParseError("port name missing".into()))?;
    Ok(PortDecl { name, direction: dir, width, width_expr })
}

fn lower_ansi_port_variable(
    tree: &SyntaxTree,
    x: &sv_parser::AnsiPortDeclarationVariable,
) -> Result<PortDecl, FrontendError> {
    let dir = if let Some(hdr) = &x.nodes.0 {
        hdr.nodes.0.as_ref().map(lower_port_direction).unwrap_or(PortDirection::Output)
    } else {
        PortDirection::Output
    };
    let (width, width_expr) = packed_width_expr(tree, RefNode::AnsiPortDeclarationVariable(x));
    let name = get_id(tree, RefNode::PortIdentifier(&x.nodes.1))
        .ok_or_else(|| FrontendError::ParseError("port name missing".into()))?;
    Ok(PortDecl { name, direction: dir, width, width_expr })
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
                for pa in a.nodes.1.nodes.1.0.nodes.0.contents() {
                    if let Some(name) = get_id(tree, RefNode::ParameterIdentifier(&pa.nodes.0)) {
                        let value = if let Some((_, cpe)) = &pa.nodes.2 {
                            let text = tree.get_str(cpe).unwrap_or("0");
                            parse_simple_const_expr(text.trim())
                        } else {
                            Expr::Const(lv(0, 32))
                        };
                        params.push(ParamDecl { name, value });
                    }
                }
            }
            sv_parser::ParameterPortList::Declaration(d) => {
                for item in d.nodes.1.nodes.1.contents() {
                    if let Some(name_node) = unwrap_node!(item, ParameterIdentifier) {
                        if let Some(name) = get_id(tree, name_node) {
                            params.push(ParamDecl { name, value: Expr::Const(lv(0, 32)) });
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

type Items = (Vec<NetDecl>, Vec<RegDecl>, Vec<MemDecl>, Vec<LocalParamDecl>, Vec<ContinuousAssign>, Vec<InitialConstruct>, Vec<AlwaysConstruct>, Vec<ModuleInstance>);

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

    for item in items {
        if let sv_parser::NonPortModuleItem::ModuleOrGenerateItem(mogi) = item {
            process_mogi(tree, mogi, &mut nets, &mut regs, &mut mems, &mut locals, &mut assigns, &mut initials, &mut alwayses, &mut instances)?;
        }
    }
    Ok((nets, regs, mems, locals, assigns, initials, alwayses, instances))
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

    for item in items {
        if let sv_parser::ModuleItem::NonPortModuleItem(npmi) = item {
            if let sv_parser::NonPortModuleItem::ModuleOrGenerateItem(mogi) = npmi.as_ref() {
                process_mogi(tree, mogi, &mut nets, &mut regs, &mut mems, &mut locals, &mut assigns, &mut initials, &mut alwayses, &mut instances)?;
            }
        }
    }
    Ok((nets, regs, mems, locals, assigns, initials, alwayses, instances))
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
) -> Result<(), FrontendError> {
    use sv_parser::ModuleOrGenerateItem as MOGI;
    match mogi {
        MOGI::Module(m) => {
            instances.push(lower_module_inst(tree, &m.nodes.1)?);
        }
        MOGI::ModuleItem(mi) => {
            use sv_parser::ModuleCommonItem as MCI;
            match &mi.nodes.1 {
                MCI::AlwaysConstruct(ac) => alwayses.push(lower_always(tree, ac)?),
                MCI::InitialConstruct(ic) => initials.push(lower_initial(tree, ic)?),
                MCI::ContinuousAssign(ca) => assigns.extend(lower_continuous_assign(tree, ca)?),
                MCI::ModuleOrGenerateItemDeclaration(d) => lower_decl(tree, d, nets, regs, mems, locals)?,
                _ => {}
            }
        }
        _ => {}
    }
    Ok(())
}

// ── declarations ──────────────────────────────────────────────────────────────

fn lower_decl(
    tree: &SyntaxTree,
    d: &sv_parser::ModuleOrGenerateItemDeclaration,
    nets: &mut Vec<NetDecl>,
    regs: &mut Vec<RegDecl>,
    mems: &mut Vec<MemDecl>,
    locals: &mut Vec<LocalParamDecl>,
) -> Result<(), FrontendError> {
    use sv_parser::ModuleOrGenerateItemDeclaration as D;
    if let D::PackageOrGenerateItemDeclaration(p) = d {
        use sv_parser::PackageOrGenerateItemDeclaration as PD;
        match p.as_ref() {
            PD::NetDeclaration(nd) => nets.extend(lower_net_decl(tree, nd)?),
            PD::DataDeclaration(dd) => {
                let (new_regs, new_mems) = lower_data_decl(tree, dd)?;
                regs.extend(new_regs);
                mems.extend(new_mems);
            }
            PD::LocalParameterDeclaration(lp) => locals.extend(lower_localparam(tree, lp)?),
            _ => {}
        }
    }
    Ok(())
}

fn lower_net_decl(tree: &SyntaxTree, nd: &sv_parser::NetDeclaration) -> Result<Vec<NetDecl>, FrontendError> {
    let mut out = Vec::new();
    if let sv_parser::NetDeclaration::NetType(nt) = nd {
        let (width, width_expr) = packed_width_expr(tree, RefNode::NetDeclarationNetType(nt));
        // ListOfNetDeclAssignments → NetDeclAssignment → NetIdentifier
        for name_node in unwrap_all_net_identifiers(RefNode::NetDeclarationNetType(nt)) {
            if let Some(name) = get_id(tree, name_node) {
                out.push(NetDecl { name, width, kind: NetKind::Wire, width_expr: width_expr.clone() });
            }
        }
    }
    Ok(out)
}

fn lower_data_decl(tree: &SyntaxTree, dd: &sv_parser::DataDeclaration) -> Result<(Vec<RegDecl>, Vec<MemDecl>), FrontendError> {
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

        // Check for unpacked dimension (indicates memory array)
        for vda_node in unwrap_all_var_decl_assignments(RefNode::DataDeclarationVariable(dv)) {
            let name = match get_id(tree, vda_node.clone()) {
                Some(n) => n,
                None => continue,
            };
            let depth_expr = extract_unpacked_dim_depth(tree, vda_node);
            if let Some(depth) = depth_expr {
                mems.push(MemDecl { name, elem_width: width_expr.clone(), depth });
            } else {
                regs.push(RegDecl { name, width, width_expr: width_expr.clone() });
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
            let width_e = Expr::Bin(BinOp::Add,
                Box::new(Expr::Bin(BinOp::Sub, Box::new(msb_e.clone()), Box::new(lsb_e))),
                Box::new(Expr::Const(lv(1, 32))));
            // Try to compute literal width for static declarations
            let literal_width = if let Expr::Const(v) = &msb_e {
                v.pad_to_width(32) as u32 + 1
            } else {
                0  // dynamic (param-dependent)
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
        let rhs = s[pos+1..].trim();
        if !lhs.is_empty() && !rhs.is_empty() {
            return Expr::Bin(BinOp::Sub, Box::new(parse_simple_const_expr(lhs)), Box::new(parse_simple_const_expr(rhs)));
        }
    }
    if let Some(pos) = s.find('+') {
        let lhs = s[..pos].trim();
        let rhs = s[pos+1..].trim();
        if !lhs.is_empty() {
            return Expr::Bin(BinOp::Add, Box::new(parse_simple_const_expr(lhs)), Box::new(parse_simple_const_expr(rhs)));
        }
    }
    // Check for $clog2(...)
    if s.starts_with("$clog2(") && s.ends_with(')') {
        let inner = &s[7..s.len()-1];
        return Expr::SysFunc(SysFuncKind::Clog2, vec![parse_simple_const_expr(inner)]);
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
                let d = Expr::Bin(BinOp::Add,
                    Box::new(Expr::Bin(BinOp::Sub, Box::new(hi_e), Box::new(lo_e))),
                    Box::new(Expr::Const(lv(1, 32))));
                return Some(d);
            }
        }
    }
    None
}

fn lower_localparam(
    tree: &SyntaxTree,
    lp: &Box<(sv_parser::LocalParameterDeclaration, sv_parser::Symbol)>,
) -> Result<Vec<LocalParamDecl>, FrontendError> {
    let mut out = Vec::new();
    if let sv_parser::LocalParameterDeclaration::Param(lpdp) = &lp.0 {
        for pa in lpdp.nodes.2.nodes.0.contents() {
            if let Some(name) = get_id(tree, RefNode::ParameterIdentifier(&pa.nodes.0)) {
                let value = if let Some((_, cpe)) = &pa.nodes.2 {
                    let text = tree.get_str(cpe).unwrap_or("0");
                    parse_simple_const_expr(text.trim())
                } else {
                    Expr::Const(lv(0, 32))
                };
                out.push(LocalParamDecl { name, value });
            }
        }
    }
    Ok(out)
}

fn unwrap_all_net_identifiers(node: RefNode) -> Vec<RefNode> {
    let mut out = Vec::new();
    for inner in node {
        if let RefNode::NetIdentifier(_) = &inner {
            out.push(inner);
        }
    }
    out
}

fn unwrap_all_variable_identifiers(node: RefNode) -> Vec<RefNode> {
    let mut out = Vec::new();
    for inner in node {
        if let RefNode::VariableIdentifier(_) = &inner {
            out.push(inner);
        }
    }
    out
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

fn lower_always(tree: &SyntaxTree, ac: &sv_parser::AlwaysConstruct) -> Result<AlwaysConstruct, FrontendError> {
    let (sensitivity, body) = split_timing_from_stmt(tree, &ac.nodes.1)?;
    Ok(AlwaysConstruct { sensitivity, body })
}

fn lower_initial(tree: &SyntaxTree, ic: &sv_parser::InitialConstruct) -> Result<InitialConstruct, FrontendError> {
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
        match &ptcs.nodes.0 {
            PTC::EventControl(ec) => {
                let sens = lower_event_control(tree, ec)?;
                let body = lower_stmt_or_null(tree, &ptcs.nodes.1)?;
                return Ok((Some(sens), body));
            }
            _ => {}
        }
    }
    Ok((None, lower_statement(tree, stmt)?))
}

fn lower_event_control(tree: &SyntaxTree, ec: &sv_parser::EventControl) -> Result<Sensitivity, FrontendError> {
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

fn collect_sens_items(tree: &SyntaxTree, expr: &sv_parser::EventExpression) -> Result<Vec<SensitivityItem>, FrontendError> {
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
            let signal = get_id(tree, RefNode::Expression(&e.nodes.1))
                .unwrap_or_else(|| SmolStr::new("?"));
            Ok(vec![SensitivityItem { edge, signal }])
        }
        _ => Ok(vec![]),
    }
}

// ── statement lowering ────────────────────────────────────────────────────────

fn lower_stmt_or_null(tree: &SyntaxTree, s: &sv_parser::StatementOrNull) -> Result<Stmt, FrontendError> {
    match s {
        sv_parser::StatementOrNull::Statement(stmt) => lower_statement(tree, stmt),
        sv_parser::StatementOrNull::Attribute(_) => Ok(Stmt::Block(vec![])),
    }
}

fn lower_statement(tree: &SyntaxTree, stmt: &sv_parser::Statement) -> Result<Stmt, FrontendError> {
    lower_stmt_item(tree, &stmt.nodes.2)
}

fn lower_stmt_item(tree: &SyntaxTree, item: &sv_parser::StatementItem) -> Result<Stmt, FrontendError> {
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
        _ => Err(unsupported("statement kind")),
    }
}

fn lower_loop_stmt(tree: &SyntaxTree, ls: &sv_parser::LoopStatement) -> Result<Stmt, FrontendError> {
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
            Ok(Stmt::For { var, init: Box::new(init_stmt), cond, step: Box::new(step_stmt), body: Box::new(body) })
        }
        _ => Err(unsupported("loop statement variant")),
    }
}

fn lower_for_init(tree: &SyntaxTree, init: Option<&sv_parser::ForInitialization>) -> Result<(Option<SmolStr>, Stmt), FrontendError> {
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
                    if var_name.is_none() { var_name = Some(name.clone()); }
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
        match fsa {
            FSA::OperatorAssignment(oa) => {
                let lval = lower_var_lvalue(tree, &oa.nodes.0)?;
                let expr = lower_expression(tree, &oa.nodes.2)?;
                stmts.push(Stmt::BlockingAssign(lval, expr));
            }
            _ => {}
        }
    }
    Ok(Stmt::Block(stmts))
}

fn lower_seq_block(tree: &SyntaxTree, sb: &sv_parser::SeqBlock) -> Result<Stmt, FrontendError> {
    // SeqBlock.nodes = (begin, Option<...>, Vec<BlockItemDecl>, Vec<StatementOrNull>, end, ...)
    let mut stmts = Vec::new();
    for s in &sb.nodes.3 {
        stmts.push(lower_stmt_or_null(tree, s)?);
    }
    Ok(Stmt::Block(stmts))
}

fn lower_conditional(tree: &SyntaxTree, cs: &sv_parser::ConditionalStatement) -> Result<Stmt, FrontendError> {
    // nodes = (Option<UniquePriority>, "if", Paren<CondPredicate>, StatementOrNull,
    //          Vec<(else_kw, if_kw, Paren<CondPredicate>, StatementOrNull)>, Option<(else_kw, StatementOrNull)>)
    let cond = lower_cond_pred(tree, &cs.nodes.2.nodes.1)?;
    let then_b = lower_stmt_or_null(tree, &cs.nodes.3)?;
    let else_b = if let Some((_, else_s)) = &cs.nodes.5 {
        Some(Box::new(lower_stmt_or_null(tree, else_s)?))
    } else {
        None
    };
    Ok(Stmt::If(cond, Box::new(then_b), else_b))
}

fn lower_cond_pred(tree: &SyntaxTree, cp: &sv_parser::CondPredicate) -> Result<Expr, FrontendError> {
    // CondPredicate.nodes.0 is List<Symbol, ExpressionOrCondPattern>
    // List.nodes.0 is first item
    let first = &cp.nodes.0.nodes.0;
    match first {
        sv_parser::ExpressionOrCondPattern::Expression(e) => lower_expression(tree, e),
        _ => Err(FrontendError::ParseError("unsupported cond pattern".into())),
    }
}

fn lower_blocking(tree: &SyntaxTree, ba: &sv_parser::BlockingAssignment) -> Result<Stmt, FrontendError> {
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

fn lower_nonblocking(tree: &SyntaxTree, na: &sv_parser::NonblockingAssignment) -> Result<Stmt, FrontendError> {
    // NonblockingAssignment.nodes = (VariableLvalue, "<=", Option<DelayOrEventControl>, Expression)
    let lval = lower_var_lvalue(tree, &na.nodes.0)?;
    let expr = lower_expression(tree, &na.nodes.3)?;
    Ok(Stmt::NbaAssign(lval, expr))
}

fn lower_timing_ctrl(tree: &SyntaxTree, ptcs: &sv_parser::ProceduralTimingControlStatement) -> Result<Stmt, FrontendError> {
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

fn lower_syscall_stmt(tree: &SyntaxTree, sc: &sv_parser::SubroutineCallStatement) -> Result<Stmt, FrontendError> {
    use sv_parser::SubroutineCallStatement as SCS;
    match sc {
        SCS::SubroutineCall(sub) => lower_subcall(tree, &sub.0),
        _ => Err(unsupported("subroutine call statement")),
    }
}

fn lower_subcall(tree: &SyntaxTree, sc: &sv_parser::SubroutineCall) -> Result<Stmt, FrontendError> {
    if let sv_parser::SubroutineCall::SystemTfCall(sys) = sc {
        return lower_system_task(tree, sys);
    }
    Err(unsupported("non-system subroutine call"))
}

fn lower_system_task(tree: &SyntaxTree, sys: &sv_parser::SystemTfCall) -> Result<Stmt, FrontendError> {
    let name_node = match sys {
        sv_parser::SystemTfCall::ArgOptionl(s) => RefNode::SystemTfIdentifier(&s.nodes.0),
        sv_parser::SystemTfCall::ArgExpression(s) => RefNode::SystemTfIdentifier(&s.nodes.0),
        sv_parser::SystemTfCall::ArgDataType(s) => RefNode::SystemTfIdentifier(&s.nodes.0),
    };
    let name_text = if let Some(RefNode::SystemTfIdentifier(tf)) = Some(name_node) {
        tree.get_str(&tf.nodes.0).unwrap_or("?")
    } else {
        "?"
    };

    let task = match name_text {
        "$display" => SysTask::Display,
        "$write" => SysTask::Write,
        "$monitor" => SysTask::Monitor,
        "$finish" => SysTask::Finish,
        "$time" => SysTask::Time,
        "$dumpfile" => SysTask::DumpFile,
        "$dumpvars" => SysTask::DumpVars,
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
            let list = &s.nodes.1.nodes.1.0;
            for opt_expr in list.contents() {
                if let Some(e) = opt_expr {
                    if let Ok(expr) = lower_expression(tree, e) {
                        args.push(expr);
                    }
                }
            }
        }
        sv_parser::SystemTfCall::ArgOptionl(s) => {
            if let Some(paren) = &s.nodes.1 {
                // Paren.nodes.1 is ListOfArguments
                if let sv_parser::ListOfArguments::Ordered(o) = &paren.nodes.1 {
                    // ListOfArgumentsOrdered.nodes.0 is List<Symbol, Option<Expression>>
                    for opt_expr in o.nodes.0.contents() {
                        if let Some(e) = opt_expr {
                            if let Ok(expr) = lower_expression(tree, e) {
                                args.push(expr);
                            }
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
                    let patterns: Result<Vec<_>, _> = nd.nodes.0.contents()
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
        return Ok(Stmt::Case { sel, arms, default, kind });
    }
    Err(unsupported("case statement variant"))
}

// ── lvalue ────────────────────────────────────────────────────────────────────

fn lower_var_lvalue(tree: &SyntaxTree, lv: &sv_parser::VariableLvalue) -> Result<LValue, FrontendError> {
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
                return Ok(LValue::Net(name));
            }
            Err(FrontendError::ParseError("lvalue identifier missing".into()))
        }
        VL::Lvalue(c) => {
            // Brace<List<Symbol, VariableLvalue>>
            let list = &c.nodes.0.nodes.1;
            let mut parts: Vec<LValue> = Vec::new();
            for lv_inner in list.contents() {
                parts.push(lower_var_lvalue(tree, lv_inner)?);
            }
            parts.into_iter().next()
                .ok_or_else(|| FrontendError::ParseError("empty concat lvalue".into()))
        }
        _ => Err(unsupported("variable lvalue")),
    }
}

// ── expression lowering ───────────────────────────────────────────────────────

fn lower_expression(tree: &SyntaxTree, expr: &sv_parser::Expression) -> Result<Expr, FrontendError> {
    use sv_parser::Expression as E;
    match expr {
        E::Primary(p) => lower_primary(tree, p),
        E::Unary(u) => {
            let op = lower_unary_op(tree, &u.nodes.0)?;
            let inner = lower_primary(tree, &u.nodes.2)?;
            Ok(Expr::Un(op, Box::new(inner)))
        }
        E::Binary(b) => {
            let lhs = lower_expression(tree, &b.nodes.0)?;
            let op = lower_binary_op(tree, &b.nodes.1)?;
            let rhs = lower_expression(tree, &b.nodes.3)?;
            Ok(Expr::Bin(op, Box::new(lhs), Box::new(rhs)))
        }
        E::ConditionalExpression(ce) => {
            // ConditionalExpression.nodes = (CondPredicate, "?", Vec<Attr>, Expression, ":", Expression)
            let cond = lower_cond_pred(tree, &ce.nodes.0)?;
            let then_e = lower_expression(tree, &ce.nodes.3)?;
            let else_e = lower_expression(tree, &ce.nodes.5)?;
            Ok(Expr::Cond(Box::new(cond), Box::new(then_e), Box::new(else_e)))
        }
        _ => Err(unsupported("expression kind")),
    }
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
                Ok(Expr::Net(name))
            } else {
                Err(FrontendError::ParseError("identifier in expression missing".into()))
            }
        }
        P::PrimaryLiteral(lit) => lower_primary_literal(tree, lit),
        P::Concatenation(c) => {
            let mut parts = Vec::new();
            for inner in c.as_ref() {
                if let RefNode::Expression(e) = inner {
                    parts.push(lower_expression(tree, e)?);
                }
            }
            Ok(Expr::Concat(parts))
        }
        P::MultipleConcatenation(mc) => {
            let mca = mc.as_ref();
            let count_expr = if let Some(RefNode::ConstantExpression(ce)) = unwrap_node!(mca, ConstantExpression) {
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

fn lower_primary_literal(tree: &SyntaxTree, lit: &sv_parser::PrimaryLiteral) -> Result<Expr, FrontendError> {
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
    Ok(Expr::Const(parse_number_text(text.trim())))
}

fn lower_const_expr(tree: &SyntaxTree, ce: &sv_parser::ConstantExpression) -> Result<Expr, FrontendError> {
    if let Some(text) = tree.get_str(ce) {
        return Ok(Expr::Const(parse_number_text(text.trim())));
    }
    Ok(Expr::Const(lv(1, 32)))
}

fn parse_number_text(text: &str) -> LogicVal {
    if let Some(tick) = text.find('\'') {
        let size: u32 = text[..tick].trim().parse().unwrap_or(32);
        let rest = &text[tick + 1..];
        let (base, digits) = match rest.chars().next().unwrap_or('d') {
            'd' | 'D' => (10u32, &rest[1..]),
            'h' | 'H' => (16u32, &rest[1..]),
            'b' | 'B' => (2u32, &rest[1..]),
            'o' | 'O' => (8u32, &rest[1..]),
            _ => (10u32, rest),
        };
        let clean: String = digits.chars().filter(|c| *c != '_').collect();
        if clean.chars().any(|c| c == 'x' || c == 'X' || c == 'z' || c == 'Z') {
            return lv_x(size as u16);
        }
        let val = u64::from_str_radix(&clean, base).unwrap_or(0);
        lv(val, size as u16)
    } else {
        let val: u64 = text.trim().parse().unwrap_or(0);
        lv(val, 32)
    }
}

fn lower_unary_op(tree: &SyntaxTree, op: &sv_parser::UnaryOperator) -> Result<UnOp, FrontendError> {
    let text = tree.get_str(op).unwrap_or("?");
    match text.trim() {
        "+" => Ok(UnOp::Pos),
        "-" => Ok(UnOp::Neg),
        "!" => Ok(UnOp::LogNot),
        "~" => Ok(UnOp::BitNot),
        _ => Ok(UnOp::BitNot),
    }
}

fn lower_binary_op(tree: &SyntaxTree, op: &sv_parser::BinaryOperator) -> Result<BinOp, FrontendError> {
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

fn lower_module_inst(tree: &SyntaxTree, mi: &sv_parser::ModuleInstantiation) -> Result<ModuleInstance, FrontendError> {
    // ModuleInstantiation.nodes = (ModuleIdentifier, Option<ParameterValueAssignment>, List<Symbol, HierarchicalInstance>, Symbol)
    let module = get_id(tree, RefNode::ModuleIdentifier(&mi.nodes.0))
        .ok_or_else(|| FrontendError::ParseError("module name missing".into()))?;

    let params = lower_param_overrides(tree, &mi.nodes.1)?;

    // List<Symbol, HierarchicalInstance> - take first instance
    let hi = &mi.nodes.2.nodes.0;
    let name = get_id(tree, RefNode::NameOfInstance(&hi.nodes.0))
        .unwrap_or_else(|| SmolStr::new("inst"));
    let ports = lower_port_connections(tree, &hi.nodes.1.nodes.1)?;

    Ok(ModuleInstance { name, module, params, ports })
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
                            let text = tree.get_str(pe).unwrap_or("0");
                            parse_simple_const_expr(text.trim())
                        } else {
                            Expr::Const(lv(0, 32))
                        };
                        out.push(ParamOverride { name: pname, value });
                    }
                }
                sv_parser::ListOfParameterAssignments::Ordered(o) => {
                    for item in o.nodes.0.contents() {
                        let text = tree.get_str(&item.nodes.0).unwrap_or("0");
                        let value = parse_simple_const_expr(text.trim());
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
                        out.push(PortConnection { name: port_name, expr });
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
