use async_trait::async_trait;
use gmr_core::{Derivation, Kind, Outcome, ProbeRef, ReasonClass};

/// Env var a shell-style transport hands the position through. Shared by every
/// caller (`transport-shell`) and every callee (`probe-coord`) so the name only
/// has one owner instead of a copy each side can drift from independently.
pub const POSITION_ENV: &str = "GMR_POSITION";

/// Env var a shell-style transport hands the probe's params through.
pub const PARAMS_ENV: &str = "GMR_PARAMS";

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct ProbeError {
    pub reason: ReasonClass,
    pub code: ProbeErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProbeErrorCode {
    Unreachable,
    Unusable,
    ArtifactInvalid,
    TimedOut,
    ProcessFailed,
    OutputTooLarge,
    InvalidJson,
}

impl ProbeErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unreachable => "probe_unreachable",
            Self::Unusable => "probe_unusable",
            Self::ArtifactInvalid => "artifact_invalid",
            Self::TimedOut => "probe_timed_out",
            Self::ProcessFailed => "probe_process_failed",
            Self::OutputTooLarge => "probe_output_too_large",
            Self::InvalidJson => "probe_invalid_json",
        }
    }
}

impl ProbeError {
    pub fn unreachable(message: impl Into<String>) -> Self {
        Self::with_code(
            ReasonClass::Unreachable,
            ProbeErrorCode::Unreachable,
            message,
        )
    }

    pub fn unusable(message: impl Into<String>) -> Self {
        Self::with_code(ReasonClass::Unusable, ProbeErrorCode::Unusable, message)
    }

    pub fn with_code(
        reason: ReasonClass,
        code: ProbeErrorCode,
        message: impl Into<String>,
    ) -> Self {
        Self {
            reason,
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code.as_str()
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
