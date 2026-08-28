#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Busy,
    Constraint,
    Corrupt,
    Io,
    Other,
}

impl ErrorKind {
    pub fn is_retryable(self) -> bool {
        matches!(self, ErrorKind::Busy)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorCode {
    Busy,
    Constraint,
    Corrupt,
    Io,
    Other,
    HeadMoved,
    AppendOnly,
    SealedImmutable,
    SchemaVersionMismatch,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Busy => "busy",
            Self::Constraint => "constraint",
            Self::Corrupt => "corrupt",
            Self::Io => "io",
            Self::Other => "other",
            Self::HeadMoved => "head_moved",
            Self::AppendOnly => "append_only",
            Self::SealedImmutable => "sealed_immutable",
            Self::SchemaVersionMismatch => "schema_version_mismatch",
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct StoreError {
    pub kind: ErrorKind,
    pub code: ErrorCode,
    pub message: String,
}

impl StoreError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        let code = match kind {
            ErrorKind::Busy => ErrorCode::Busy,
            ErrorKind::Constraint => ErrorCode::Constraint,
            ErrorKind::Corrupt => ErrorCode::Corrupt,
            ErrorKind::Io => ErrorCode::Io,
            ErrorKind::Other => ErrorCode::Other,
        };
        Self::with_code(kind, code, message)
    }

    pub fn with_code(kind: ErrorKind, code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            kind,
            code,
            message: message.into(),
        }
    }
    pub fn busy(m: impl Into<String>) -> Self {
        Self::new(ErrorKind::Busy, m)
    }
    pub fn constraint(m: impl Into<String>) -> Self {
        Self::new(ErrorKind::Constraint, m)
    }
    pub fn corrupt(m: impl Into<String>) -> Self {
        Self::new(ErrorKind::Corrupt, m)
    }
    pub fn io(m: impl Into<String>) -> Self {
        Self::new(ErrorKind::Io, m)
    }
    pub fn other(m: impl Into<String>) -> Self {
        Self::new(ErrorKind::Other, m)
    }

    pub fn code(&self) -> &'static str {
        self.code.as_str()
    }
}
