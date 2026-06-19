pub mod error;
pub mod instance;
pub mod param;
pub mod width;
pub mod elaborate;

pub use error::ElabError;
pub use elaborate::elaborate;

pub use rverilog_hir::{HirModule, ModuleInstance, ParamOverride, Expr, BinOp, UnOp, Sensitivity, EdgeType, PortDirection};
