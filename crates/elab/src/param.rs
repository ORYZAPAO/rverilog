use crate::ElabError;
use rverilog_hir::{Expr, HirModule, ParamOverride};
use rverilog_mir::LogicVal;

pub fn resolve_params(
    _module: &HirModule,
    _overrides: &[ParamOverride],
) -> Result<Vec<(String, LogicVal)>, ElabError> {
    Ok(Vec::new())
}

pub fn eval_const(_expr: &Expr) -> Result<LogicVal, ElabError> {
    Ok(LogicVal::X)
}
