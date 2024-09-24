use crate::OrdError;

/// A type alias for `Result<T, OrdError>`.
pub type OrdResult<T> = std::result::Result<T, OrdError>;
