/// Application-level error for the Rust side, surfaced to the frontend.
///
/// Serializable + `specta::Type` and internally tagged on `kind`, so commands
/// can return `Result<T, AppError>` and the generated TypeScript bindings get a
/// typed discriminated union:
/// `{ kind: "DbUnavailable" } | { kind: "Sql"; message: string } | …`.
///
/// `Sql`/`Io` carry a `message: String` rather than the underlying
/// `sqlx::Error` / `std::io::Error` (which are not serializable); the manual
/// `From` impls below stringify them. `Display` still emits the same stable
/// prefixes (`db_unavailable:` / `sql:` / …), so any remaining
/// `Result<T, String>` command keeps its old wire string.
#[derive(Debug, thiserror::Error, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(tag = "kind")]
pub enum AppError {
    #[error("db_unavailable: database not available")]
    DbUnavailable,

    #[error("sql: {message}")]
    Sql { message: String },

    #[error("io: {message}")]
    Io { message: String },

    #[error("validation: {message}")]
    Validation { message: String },

    #[error("{message}")]
    Other { message: String },
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        AppError::Sql { message: e.to_string() }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io { message: e.to_string() }
    }
}

impl From<AppError> for String {
    fn from(e: AppError) -> Self {
        e.to_string()
    }
}

impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError::Other { message: s }
    }
}

impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        AppError::Other { message: s.to_string() }
    }
}
