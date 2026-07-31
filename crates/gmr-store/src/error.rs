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

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct StoreError {
    pub kind: ErrorKind,
    pub message: String,
}

impl StoreError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
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
}
