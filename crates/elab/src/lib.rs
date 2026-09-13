pub mod elaborate;
pub mod error;
pub mod instance;
pub mod param;
pub mod width;

pub use elaborate::elaborate;
pub use error::ElabError;

pub use rverilog_hir::{
    BinOp, EdgeType, Expr, HirModule, ModuleInstance, ParamOverride, PortDirection, Sensitivity,
    UnOp,
};
