use indexmap::IndexMap;
use smol_str::SmolStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetKind {
    Wire,
    Reg,
    Integer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SysFuncKind {
    Clog2,
    Random,
    Signed,
    Unsigned,
}

#[derive(Debug, Clone)]
pub struct Design {
    pub modules: IndexMap<SmolStr, HirModule>,
}

#[derive(Debug, Clone)]
pub struct HirModule {
    pub name: SmolStr,
    pub ports: Vec<PortDecl>,
    pub params: Vec<ParamDecl>,
    pub locals: Vec<LocalParamDecl>,
    pub nets: Vec<NetDecl>,
    pub regs: Vec<RegDecl>,
    pub mems: Vec<MemDecl>,
    pub assigns: Vec<ContinuousAssign>,
    pub initials: Vec<InitialConstruct>,
    pub alwayses: Vec<AlwaysConstruct>,
    pub instances: Vec<ModuleInstance>,
    pub functions: Vec<FunctionDecl>,
    pub tasks: Vec<TaskDecl>,
    pub generates: GenerateItems,
}

/// Items contributed by a `generate`/`endgenerate` region or a conditional/loop
/// generate construct. Mirrors the flat item lists on `HirModule`, plus nested
/// generate constructs for recursive expansion at elaboration time.
#[derive(Debug, Clone, Default)]
pub struct GenerateItems {
    pub nets: Vec<NetDecl>,
    pub regs: Vec<RegDecl>,
    pub mems: Vec<MemDecl>,
    pub locals: Vec<LocalParamDecl>,
    pub assigns: Vec<ContinuousAssign>,
    pub initials: Vec<InitialConstruct>,
    pub alwayses: Vec<AlwaysConstruct>,
    pub instances: Vec<ModuleInstance>,
    pub nested: Vec<GenerateConstruct>,
}

#[derive(Debug, Clone)]
pub enum GenerateConstruct {
    If(GenerateIf),
    Case(GenerateCase),
    For(GenerateFor),
}

#[derive(Debug, Clone)]
pub struct GenerateIf {
    pub cond: Expr,
    pub then_items: GenerateItems,
    pub else_items: GenerateItems,
}

#[derive(Debug, Clone)]
pub struct GenerateCase {
    pub sel: Expr,
    pub arms: Vec<(Vec<Expr>, GenerateItems)>,
    pub default: Option<GenerateItems>,
}

#[derive(Debug, Clone)]
pub struct GenerateFor {
    pub var: SmolStr,
    pub init: Expr,
    pub cond: Expr,
    /// expression for the new value of `var` after each iteration (e.g. `i+1`)
    pub step: Expr,
    pub body: GenerateItems,
}

#[derive(Debug, Clone)]
pub struct FunctionDecl {
    pub name: SmolStr,
    pub width: u32,
    pub width_expr: Expr,
    pub signed: bool,
    pub args: Vec<TfArg>,
    pub locals: Vec<RegDecl>,
    pub body: Stmt,
}

#[derive(Debug, Clone)]
pub struct TaskDecl {
    pub name: SmolStr,
    pub args: Vec<TfArg>,
    pub locals: Vec<RegDecl>,
    pub body: Stmt,
}

#[derive(Debug, Clone)]
pub struct TfArg {
    pub name: SmolStr,
    pub width_expr: Expr,
    pub direction: PortDirection,
    pub signed: bool,
}

#[derive(Debug, Clone)]
pub struct PortDecl {
    pub name: SmolStr,
    pub direction: PortDirection,
    pub width: u32,        // static fallback (1 if param-dependent)
    pub width_expr: Expr,  // authoritative width expression
    pub signed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortDirection {
    Input,
    Output,
    Inout,
}

#[derive(Debug, Clone)]
pub struct ParamDecl {
    pub name: SmolStr,
    pub value: Expr,
}

#[derive(Debug, Clone)]
pub struct LocalParamDecl {
    pub name: SmolStr,
    pub value: Expr,
}

#[derive(Debug, Clone)]
pub struct NetDecl {
    pub name: SmolStr,
    pub width: u32,
    pub kind: NetKind,
    pub width_expr: Expr,
    pub signed: bool,
}

#[derive(Debug, Clone)]
pub struct RegDecl {
    pub name: SmolStr,
    pub width: u32,
    pub width_expr: Expr,
    pub signed: bool,
}

#[derive(Debug, Clone)]
pub struct MemDecl {
    pub name: SmolStr,
    pub elem_width: Expr,
    pub depth: Expr,
}

#[derive(Debug, Clone)]
pub struct ContinuousAssign {
    pub lval: LValue,
    pub expr: Expr,
}

#[derive(Debug, Clone)]
pub struct InitialConstruct {
    pub body: Stmt,
}

#[derive(Debug, Clone)]
pub struct AlwaysConstruct {
    pub sensitivity: Option<Sensitivity>,
    pub body: Stmt,
}

#[derive(Debug, Clone)]
pub struct ModuleInstance {
    pub name: SmolStr,
    pub module: SmolStr,
    pub params: Vec<ParamOverride>,
    pub ports: Vec<PortConnection>,
}

#[derive(Debug, Clone)]
pub struct ParamOverride {
    pub name: Option<SmolStr>,
    pub value: Expr,
}

#[derive(Debug, Clone)]
pub struct PortConnection {
    pub name: Option<SmolStr>,
    pub expr: Expr,
}

#[derive(Debug, Clone)]
pub enum LValue {
    Net(SmolStr),
    BitSelect(Box<LValue>, u32),
    PartSelect(Box<LValue>, Range),
    IndexSel(SmolStr, Box<Expr>),
}

#[derive(Debug, Clone)]
pub struct Range {
    pub left: Box<Expr>,
    pub right: Box<Expr>,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Block(Vec<Stmt>),
    If(Expr, Box<Stmt>, Option<Box<Stmt>>),
    Case { sel: Expr, arms: Vec<(Vec<Expr>, Box<Stmt>)>, default: Option<Box<Stmt>>, kind: CaseKind },
    BlockingAssign(LValue, Expr),
    NbaAssign(LValue, Expr),
    Delay(u64, Box<Stmt>),
    EventCtl(Sensitivity, Box<Stmt>),
    SysCall(SysTask, Vec<Expr>),
    TaskCall(SmolStr, Vec<Expr>),
    For {
        var: SmolStr,
        init: Box<Stmt>,
        cond: Expr,
        step: Box<Stmt>,
        body: Box<Stmt>,
    },
    /// `begin : label ... end`。`disable label;` の対象になり得る名前付きブロック。
    NamedBlock(SmolStr, Vec<Stmt>),
    /// `disable label;`。同一プロセス内の名前付きブロックを中断して抜ける（M2サブセット：他プロセスのタスク/ブロックの中断は未対応）。
    Disable(SmolStr),
    /// `fork ... join`。各分岐を並行プロセスとして起動し、全分岐の完了を待つ（join_any/join_noneはjoinとして扱う）。
    Fork(Vec<Stmt>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseKind {
    Case,
    CaseZ,
    CaseX,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Const(LogicVal),
    /// signed literal: 符号無しの10進即値（`-7`, `2` 等）、または `'s` 基数指定
    /// （`4'sd5` 等）。IEEE 1364-2001 4.8: これらは既定でsigned文脈になる。
    SignedConst(LogicVal),
    Net(SmolStr),
    StringLit(SmolStr),
    BitSel(Box<Expr>, Box<Expr>),
    PartSel(Box<Expr>, Box<Range>),
    Concat(Vec<Expr>),
    Repeat(Box<Expr>, Vec<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    Un(UnOp, Box<Expr>),
    Cond(Box<Expr>, Box<Expr>, Box<Expr>),
    IndexSel(SmolStr, Box<Expr>),
    SysFunc(SysFuncKind, Vec<Expr>),
    Call(SmolStr, Vec<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add, Sub, Mul, Div, Mod,
    LogAnd, LogOr,
    BitAnd, BitOr, BitXor, BitNand, BitNor, BitXnor,
    Eq, Ne, CaseEq, CaseNe, Lt, Gt, Le, Ge,
    Shl, Shr, Ashl, Ashr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Pos, Neg, LogNot, BitNot,
}

#[derive(Debug, Clone)]
pub enum Sensitivity {
    All,
    Items(Vec<SensitivityItem>),
}

#[derive(Debug, Clone)]
pub struct SensitivityItem {
    pub edge: Option<EdgeType>,
    pub signal: SmolStr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeType {
    Posedge,
    Negedge,
}

#[derive(Debug, Clone)]
pub enum SysTask {
    Display,
    Write,
    Monitor,
    Finish,
    Time,
    DumpFile,
    DumpVars,
    ReadMemH,
    ReadMemB,
}

use rverilog_mir::LogicVal;
