use thiserror::Error;

#[derive(Error, Debug)]
pub enum ElabError {
    #[error("Module not found: {0}")]
    ModuleNotFound(String),
    #[error("Unresolved name: {0}")]
    UnresolvedName(String),
    #[error("Unsupported construct: {0}")]
    UnsupportedConstruct(String),
    #[error("Parameter error: {0}")]
    ParamError(String),
    #[error("Width inference error: {0}")]
    WidthError(String),
}
