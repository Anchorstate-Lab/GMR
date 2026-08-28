use async_trait::async_trait;
use gmr_budget::{Budget, Spent};
use gmr_core::{Derivation, Kind, Outcome, ProbeName, ProbeRef, ReasonClass};

pub const POSITION_ENV: &str = "GMR_POSITION";

pub const PARAMS_ENV: &str = "GMR_PARAMS";

pub struct ProbeCall<'a> {
    pub probe: &'a ProbeRef,
    pub position: &'a serde_json::Value,
    pub budget: &'a Budget,
}

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

    async fn invoke(&self, call: &ProbeCall<'_>) -> Result<Outcome, ProbeError>;
}

impl ProbeError {
    pub fn spent(spent: Spent, budget: &Budget) -> Self {
        Self::with_code(
            ReasonClass::Unreachable,
            ProbeErrorCode::TimedOut,
            format!(
                "{}; silence is not evidence. It was {:?} wide",
                spent.as_str(),
                budget.width()
            ),
        )
    }

    pub fn too_large(size: usize, cap: usize) -> Self {
        Self::with_code(
            ReasonClass::Unusable,
            ProbeErrorCode::OutputTooLarge,
            format!(
                "probe output is {size} bytes, above the {cap} byte limit; refusing to truncate. \
                 Storing a truncated reading as fact would be a lie. Print structure, not dumps"
            ),
        )
    }
}
