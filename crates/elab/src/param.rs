use rverilog_hir::{HirModule, ParamOverride, Expr};
use rverilog_mir::LogicVal;
use crate::ElabError;

pub fn resolve_params(
    _module: &HirModule,
    _overrides: &[ParamOverride],
) -> Result<Vec<(String, LogicVal)>, ElabError> {
    Ok(Vec::new())
}

pub fn eval_const(_expr: &Expr) -> Result<LogicVal, ElabError> {
    Ok(LogicVal::X)
}
