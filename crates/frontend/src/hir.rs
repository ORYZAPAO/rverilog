#![allow(dead_code)]
use smol_str::SmolStr;
use indexmap::IndexMap;
use smallvec::SmallVec;

use rverilog_mir::{NetId, ProcessId, ContId, StmtId, ExprId, Sensitivity, LValue, NetKind};

#[derive(Debug, Clone)]
pub struct Range {
    pub left: u32,
    pub right: u32,
}

#[derive(Debug, Clone)]
pub struct Design {
    pub modules: IndexMap<SmolStr, Module>,
}

#[derive(Debug, Clone)]
pub struct Module {
    pub name: SmolStr,
    pub ports: Vec<Port>,
    pub nets: Vec<Net>,
    pub assigns: Vec<Assign>,
    pub processes: Vec<Process>,
    pub children: Vec<Instance>,
}

#[derive(Debug, Clone)]
pub struct Port {
    pub name: SmolStr,
    pub dir: PortDir,
    pub range: Option<Range>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortDir {
    Input,
    Output,
    Inout,
}

#[derive(Debug, Clone)]
pub struct Net {
    pub id: NetId,
    pub name: SmolStr,
    pub kind: NetKind,
    pub range: Range,
}

#[derive(Debug, Clone)]
pub struct Assign {
    pub id: ContId,
    pub lvalue: LValue,
    pub expr: ExprId,
}

#[derive(Debug, Clone)]
pub struct Process {
    pub id: ProcessId,
    pub name: SmolStr,
    pub sensitivity: Vec<Sensitivity>,
    pub body: StmtId,
}

#[derive(Debug, Clone)]
pub struct Instance {
    pub name: SmolStr,
    pub module: SmolStr,
    pub connections: Vec<(SmolStr, ExprId)>,
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Identifier(SmolStr),
    Constant { value: String, range: Range },
    Concat(Vec<ExprId>),
    Replicate { times: ExprId, body: ExprId },
    Binary { op: BinaryOp, lhs: ExprId, rhs: ExprId },
    Unary { op: UnaryOp, expr: ExprId },
    Conditional { cond: ExprId, tbranch: ExprId, fbranch: ExprId },
    PartSelect { base: ExprId, range: Range },
    Indexed { expr: ExprId, index: ExprId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add, Sub, Mul, Div, Mod,
    And, Or, Xor,
    Lt, Le, Gt, Ge, Eq, Ne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Pos, Neg, Not, BitNot,
}

pub type ExprArena = Vec<Expr>;

#[derive(Debug, Clone)]
pub struct Stmt {
    pub kind: StmtKind,
}

#[derive(Debug, Clone)]
pub enum StmtKind {
    Block { name: Option<SmolStr>, stmts: Vec<StmtId> },
    Assign { lvalue: LValue, expr: ExprId },
    If { cond: ExprId, tbranch: StmtId, fbranch: Option<StmtId> },
    Case { expr: ExprId, cases: Vec<CaseItem> },
    While { cond: ExprId, body: StmtId },
    Loop { body: StmtId },
    Disable(SmolStr),
    Wait { cond: ExprId },
    Delay { value: ExprId, body: StmtId },
    SystemTask { name: SmolStr, args: Vec<ExprId> },
}

#[derive(Debug, Clone)]
pub struct CaseItem {
    pub exprs: SmallVec<[ExprId; 1]>,
    pub body: StmtId,
}

pub type StmtArena = Vec<Stmt>;
