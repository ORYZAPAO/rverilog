use indexmap::IndexMap;
use smol_str::SmolStr;
use crate::logicval::LogicVal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NetId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProcessId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScopeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StmtId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExprId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MemId(pub u32);

#[derive(Debug, Clone)]
pub struct MemInfo {
    pub depth: u32,
    pub elem_width: u32,
    pub scope: ScopeId,
    pub name: SmolStr,
}

#[derive(Debug, Clone)]
pub struct ElaboratedDesign {
    pub nets: Vec<NetInfo>,
    pub memories: Vec<MemInfo>,
    pub stmts: Vec<Stmt>,
    pub exprs: Vec<Expr>,
    pub processes: Vec<Process>,
    pub conts: Vec<ContAssign>,
    pub scopes: Vec<Scope>,
    pub top: ScopeId,
    /// net.0 → [process.0] sensitivity reverse table
    pub sensitivity_table: IndexMap<u32, Vec<u32>>,
}

impl ElaboratedDesign {
    pub fn get_net(&self, id: NetId) -> &NetInfo {
        &self.nets[id.0 as usize]
    }
    pub fn get_mem(&self, id: MemId) -> &MemInfo {
        &self.memories[id.0 as usize]
    }
    pub fn get_stmt(&self, id: StmtId) -> &Stmt {
        &self.stmts[id.0 as usize]
    }
    pub fn get_expr(&self, id: ExprId) -> &Expr {
        &self.exprs[id.0 as usize]
    }
    pub fn get_process(&self, id: ProcessId) -> &Process {
        &self.processes[id.0 as usize]
    }
}

#[derive(Debug, Clone)]
pub struct NetInfo {
    pub width: u32,
    pub kind: NetKind,
    pub scope: ScopeId,
    pub name: SmolStr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetKind {
    Wire,
    Reg,
    Integer,
}

#[derive(Debug, Clone)]
pub struct Process {
    pub scope: ScopeId,
    pub body: StmtId,
    pub kind: ProcessKind,
    pub sensitivity: Sensitivity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessKind {
    Initial,
    Always,
}

#[derive(Debug, Clone)]
pub enum Sensitivity {
    /// @*
    All,
    /// @(posedge clk, negedge rst, ...)
    Items(Vec<SensitivityEdge>),
}

#[derive(Debug, Clone)]
pub struct SensitivityEdge {
    pub edge: Option<EdgeType>,
    pub net: NetId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeType {
    Posedge,
    Negedge,
}

#[derive(Debug, Clone)]
pub struct ContAssign {
    pub lval: LValue,
    pub expr: ExprId,
}

#[derive(Debug, Clone)]
pub enum LValue {
    Net(NetId),
    BitSelect(NetId, u32),
    PartSelect(NetId, u32, u32), // net, hi, lo
    MemWrite(MemId, ExprId),
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Block(Vec<StmtId>),
    If(ExprId, StmtId, Option<StmtId>),
    Case { sel: ExprId, arms: Vec<(Vec<ExprId>, StmtId)>, default: Option<StmtId>, kind: CaseKind },
    BlockingAssign(LValue, ExprId),
    NbaAssign(LValue, ExprId),
    Delay(u64, StmtId),
    EventCtl(Sensitivity, StmtId),
    SysCall(SysTask, Vec<ExprId>),
    While(ExprId, StmtId),
    Null,
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
    Net(NetId),
    BitSel(NetId, ExprId),
    PartSel(NetId, u32, u32), // net, hi, lo
    Concat(Vec<ExprId>),
    Repeat(u32, ExprId),
    Bin(BinOp, ExprId, ExprId),
    Un(UnOp, ExprId),
    Cond(ExprId, ExprId, ExprId),
    StringLit(SmolStr),
    MemRead(MemId, ExprId),
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
    RedAnd, RedNand, RedOr, RedNor, RedXor, RedXnor,
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
}

#[derive(Debug, Clone)]
pub struct Scope {
    pub parent: Option<ScopeId>,
    pub name: SmolStr,
    pub module_name: SmolStr,
}
