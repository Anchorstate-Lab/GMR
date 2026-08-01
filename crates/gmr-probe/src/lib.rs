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

/// 一次调用的全部产出：世界的答案，以及**实际算出它的那条规则的身份**。
/// 身份由传输给出而不是由锚算 —— 只有执行的那一方知道它真的跑了什么。
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
