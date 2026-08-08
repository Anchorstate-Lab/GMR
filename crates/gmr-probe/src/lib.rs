use async_trait::async_trait;
use gmr_core::{Derivation, Kind, Outcome, ProbeName, ProbeRef, ReasonClass};

pub const POSITION_ENV: &str = "GMR_POSITION";

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

impl From<ProbeErrorCode> for gmr_core::FailureCode {
    fn from(code: ProbeErrorCode) -> Self {
        match code {
            ProbeErrorCode::Unreachable => Self::Unreachable,
            ProbeErrorCode::Unusable => Self::Unusable,
            ProbeErrorCode::ArtifactInvalid => Self::ArtifactInvalid,
            ProbeErrorCode::TimedOut => Self::TimedOut,
            ProbeErrorCode::ProcessFailed => Self::ProcessFailed,
            ProbeErrorCode::OutputTooLarge => Self::OutputTooLarge,
            ProbeErrorCode::InvalidJson => Self::InvalidJson,
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

#[async_trait]
pub trait Transport: Send + Sync {
    fn kind(&self) -> &Kind;

    fn resolve(&self, name: &ProbeName) -> Option<Derivation>;

    async fn invoke(
        &self,
        probe: &ProbeRef,
        position: &serde_json::Value,
    ) -> Result<Outcome, ProbeError>;
}
