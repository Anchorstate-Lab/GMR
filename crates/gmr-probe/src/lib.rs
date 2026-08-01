use async_trait::async_trait;
use gmr_core::{Derivation, Kind, Outcome, ProbeRef, ReasonClass};

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

/// Everything one call produces: the world's answer, and **the identity of the
/// rule that actually derived it**. The transport hands that over rather than the
/// anchor computing it — only whoever executed knows what really ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sighted {
    pub outcome: Outcome,
    pub derivation: Derivation,
}

#[async_trait]
pub trait Transport: Send + Sync {
    fn kind(&self) -> &Kind;

    async fn invoke(
        &self,
        probe: &ProbeRef,
        position: &serde_json::Value,
    ) -> Result<Sighted, ProbeError>;
}
