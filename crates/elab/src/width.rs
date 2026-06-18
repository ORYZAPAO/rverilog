use rverilog_hir::Expr;

use crate::ElabError;

pub fn infer_width(expr: &Expr, context_width: Option<u32>) -> Result<u32, ElabError> {
    match expr {
        Expr::Const(val) => Ok(val.width() as u32),
        _ => Ok(context_width.unwrap_or(1)),
    }
}

pub fn infer_signed(_expr: &Expr) -> bool {
    false
}
