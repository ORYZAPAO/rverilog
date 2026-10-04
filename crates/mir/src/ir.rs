use crate::logicval::LogicVal;
use indexmap::IndexMap;
use smol_str::SmolStr;

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
    /// net.0 → 既定のwire以外のネット型（wand/wor/trireg/supply0/supply1）。
    pub net_resolve: IndexMap<u32, NetResolve>,
    /// net.0 → [(lo, width, one)] pull指定のビット範囲（tri0/tri1/pullup/pulldown）。
    /// Zのビットを `one` の値（pullup=1 / pulldown=0）へ置き換える。
    pub net_pulls: IndexMap<u32, Vec<(u32, u32, bool)>>,
    /// net.0 → [(cont_id, part_idx)] 解決が必要なネットのドライバ一覧。`part_idx` は連続代入の
    /// lvalueを `LValue::flatten_parts` で展開したときの位置（連結でなければ0）。
    /// wire/wand/worは2つ以上、pull・tregは1つ以上のドライバを持つネットのみ。
    /// simは各ドライバ値のビット単位解決（Z中立・不一致はX）でネット値を更新する。
    pub net_drivers: IndexMap<u32, Vec<(u32, u32)>>,
    /// mem_id.0 → [cont_id] 連続代入のsensitivity逆引きテーブル（cont_idはdesign.contsのインデックス）
    pub mem_sensitivity: IndexMap<u32, Vec<u32>>,
    /// exprs[i] → そのexprがsigned文脈で評価されるか（比較/除算/剰余/算術シフトの符号選択に使用）
    pub expr_signed: Vec<bool>,
    /// exprs[i] の文脈決定の評価幅（IEEE 1364-2001 5.4: 代入のLHS幅と式内の最大の自己決定幅の最大値）。
    /// 算術・ビット演算・単項`+ - ~`・シフトのBin/Unに対して記録される。simはオペランドを
    /// この幅へ（式がsignedなら符号拡張、でなければゼロ拡張で）拡張してから演算する。
    /// シフトの右辺（シフト量）は自己決定のため対象外。Noneは自己決定幅のまま評価する。
    pub expr_ctx_width: Vec<Option<u32>>,
}

impl LValue {
    /// 連結を再帰的に展開した要素（先頭がMSB）。連結でなければ自身のみ。
    pub fn flatten_parts(&self) -> Vec<&LValue> {
        match self {
            LValue::Concat(parts) => parts.iter().flat_map(|p| p.flatten_parts()).collect(),
            other => vec![other],
        }
    }
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

/// 既定のwire以外のネット型の解決規則（`ElaboratedDesign::net_resolve`）。
/// pull（tri0/tri1/pullup/pulldown）はビット範囲指定のため `net_pulls` で別に持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetResolve {
    /// wand/triand: 0が支配
    Wand,
    /// wor/trior: 1が支配
    Wor,
    /// trireg: 全ドライバがZのとき直前の値を保持（charge storage、strength大小は無視）
    Trireg,
    /// supply0: 常に0
    Supply0,
    /// supply1: 常に1
    Supply1,
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
    Case {
        sel: ExprId,
        arms: Vec<(Vec<ExprId>, StmtId)>,
        default: Option<StmtId>,
        kind: CaseKind,
    },
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
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    LogAnd,
    LogOr,
    BitAnd,
    BitOr,
    BitXor,
    BitNand,
    BitNor,
    BitXnor,
    Eq,
    Ne,
    CaseEq,
    CaseNe,
    Lt,
    Gt,
    Le,
    Ge,
    Shl,
    Shr,
    Ashl,
    Ashr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Pos,
    Neg,
    LogNot,
    BitNot,
    RedAnd,
    RedNand,
    RedOr,
    RedNor,
    RedXor,
    RedXnor,
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
