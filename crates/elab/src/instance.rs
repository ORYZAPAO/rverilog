use crate::ElabError;
use rverilog_hir::ModuleInstance;
use rverilog_mir::ScopeId;

pub fn flatten_instances(
    _module: &rverilog_hir::HirModule,
    _parent_scope: ScopeId,
) -> Result<Vec<ScopeId>, ElabError> {
    Ok(Vec::new())
}

pub fn resolve_instance(inst: &ModuleInstance) -> Result<String, ElabError> {
    Ok(inst.module.to_string())
}
