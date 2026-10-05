use serde::Serialize;

/// Error returned by every command. Serialized to the UI as `{ kind, message, field? }`.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{message}")]
    Validation { message: String, field: Option<String> },
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    NoWorkspace(String),
    #[error("{0}")]
    Provider(String),
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("File error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Data error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Other(String),
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn invalid(field: &str, message: impl Into<String>) -> Self {
        AppError::Validation { message: message.into(), field: Some(field.to_string()) }
    }
    pub fn msg(message: impl Into<String>) -> Self {
        AppError::Validation { message: message.into(), field: None }
    }
    fn kind(&self) -> &'static str {
        match self {
            AppError::Validation { .. } => "validation",
            AppError::NotFound(_) => "not_found",
            AppError::Conflict(_) => "conflict",
            AppError::NoWorkspace(_) => "no_workspace",
            AppError::Provider(_) => "provider",
            AppError::Db(_) => "database",
            AppError::Io(_) => "io",
            AppError::Json(_) => "data",
            AppError::Other(_) => "other",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("AppError", 3)?;
        st.serialize_field("kind", self.kind())?;
        st.serialize_field("message", &self.to_string())?;
        let field = match self {
            AppError::Validation { field, .. } => field.clone(),
            _ => None,
        };
        st.serialize_field("field", &field)?;
        st.end()
    }
}
