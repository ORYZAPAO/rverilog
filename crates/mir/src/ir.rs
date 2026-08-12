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
    /// net.0 → [cont_id] 連続代入のsensitivity逆引きテーブル（cont_idはdesign.contsのインデックス）
    pub cont_sensitivity: IndexMap<u32, Vec<u32>>,
    /// exprs[i] → そのexprがsigned文脈で評価されるか（比較/除算/剰余/算術シフトの符号選択に使用）
    pub expr_signed: Vec<bool>,
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
    pub is_signed: bool,
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
    /// 動的インデックスでのビット選択（genvar 等、実行時に決まるインデックス）
    DynBitSelect(NetId, ExprId),
    PartSelect(NetId, u32, u32), // net, hi, lo
    /// indexed part-select (`net[base +: width]` / `net[base -: width]`)。
    /// base は実行時式、width は定数、bool は true=`+:` / false=`-:`
    DynPartSelect(NetId, ExprId, u32, bool),
    MemWrite(MemId, ExprId),
    /// LHS連結 `{a,b} <= x`。先頭要素がMSB。
    Concat(Vec<LValue>),
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
    /// `begin : label ... end`。`u32` はelaboration時に割り振る一意なブロックID。
    NamedBlock(u32, Vec<StmtId>),
    /// `disable label;`。同一プロセスのフレームスタックを対象ブロックIDまで巻き戻す。
    Disable(u32),
    /// `fork ... join`。各分岐を並行プロセスとして起動し、全分岐の完了を待つ。
    Fork(Vec<StmtId>),
    /// `$readmemh`/`$readmemb`。第1引数（文字列リテラル式）のファイルからメモリを初期化する。
    ReadMem(SysTask, ExprId, MemId),
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
    /// indexed part-select (`net[base +: width]` / `net[base -: width]`)。
    /// base は実行時式、width は定数、bool は true=`+:` / false=`-:`
    DynPartSel(NetId, ExprId, u32, bool),
    Concat(Vec<ExprId>),
    Repeat(u32, ExprId),
    Bin(BinOp, ExprId, ExprId),
    Un(UnOp, ExprId),
    Cond(ExprId, ExprId, ExprId),
    StringLit(SmolStr),
    MemRead(MemId, ExprId),
    /// Statements that must run before reading the net (function-call setup), then the net holds the result.
    CallResult(Vec<StmtId>, NetId),
    /// `$random`/`$random(seed)`。seed があれば評価して一度だけRNG状態を上書きする（読み取り専用、書き戻しなし）。
    Random(Option<ExprId>),
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
    ReadMemH,
    ReadMemB,
}

#[derive(Debug, Clone)]
pub struct Scope {
    pub parent: Option<ScopeId>,
    pub name: SmolStr,
    pub module_name: SmolStr,
}
