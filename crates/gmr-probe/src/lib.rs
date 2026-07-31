use async_trait::async_trait;
use gmr_core::{Declaration, Kind, Outcome, ReasonClass};

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct ProbeError {
    pub reason: ReasonClass,
    pub message: String,
}

impl ProbeError {
    pub fn unreachable(message: impl Into<String>) -> Self {
        Self {
            reason: ReasonClass::Unreachable,
            message: message.into(),
        }
    }

    pub fn unusable(message: impl Into<String>) -> Self {
        Self {
            reason: ReasonClass::Unusable,
            message: message.into(),
        }
    }
}

#[async_trait]
pub trait Transport: Send + Sync {
    fn kind(&self) -> &Kind;

    async fn invoke(
        &self,
        declaration: &Declaration,
        position: &serde_json::Value,
    ) -> Result<Outcome, ProbeError>;
}
