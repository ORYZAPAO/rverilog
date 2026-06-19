use thiserror::Error;

#[derive(Error, Debug)]
pub enum FrontendError {
    #[error("Unsupported syntax node: {0}")]
    UnsupportedNode(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Unsupported construct: {0}")]
    UnsupportedConstruct(String),
    #[error("Internal error: {0}")]
    Internal(String),
}
