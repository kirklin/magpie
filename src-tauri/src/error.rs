/// Application-level error for the Rust side.
///
/// This is intentionally internal: IPC commands keep returning `Result<T, String>`
/// at the boundary (they map `AppError` to a String), so the generated TypeScript
/// bindings are unchanged and no `specta::Type` impl is needed here. Each message
/// carries a stable prefix token (e.g. `db_unavailable:`) so a future typed
/// frontend could split on `:` without a wire change.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("db_unavailable: database not available")]
    DbUnavailable,

    #[error("sql: {0}")]
    Sql(#[from] sqlx::Error),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("validation: {0}")]
    Validation(String),

    #[error("{0}")]
    Other(String),
}

impl From<AppError> for String {
    fn from(e: AppError) -> Self {
        e.to_string()
    }
}

impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError::Other(s)
    }
}

impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        AppError::Other(s.to_string())
    }
}
