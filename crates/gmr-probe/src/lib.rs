use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use gmr_core::{Derivation, Kind, Outcome, ProbeName, ProbeRef, ReasonClass};

pub const POSITION_ENV: &str = "GMR_POSITION";

pub const PARAMS_ENV: &str = "GMR_PARAMS";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spent {
    Deadline,
    Cancelled,
}

impl Spent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Deadline => "the budget for this call ran out",
            Self::Cancelled => "the call was abandoned by whoever asked for it",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Budget {
    deadline: Instant,
    output_cap: usize,
    cancel: Arc<AtomicBool>,
}

impl Budget {
    pub fn until(deadline: Instant, output_cap: usize) -> Self {
        Self {
            deadline,
            output_cap,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn within(span: Duration, output_cap: usize) -> Self {
        Self::until(Instant::now() + span, output_cap)
    }

    pub fn deadline(&self) -> Instant {
        self.deadline
    }

    pub fn output_cap(&self) -> usize {
        self.output_cap
    }

    pub fn remaining(&self) -> Option<Duration> {
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|left| !left.is_zero())
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    pub fn checkpoint(&self) -> Result<(), Spent> {
        if self.cancel.load(Ordering::SeqCst) {
            return Err(Spent::Cancelled);
        }
        match self.remaining() {
            Some(_) => Ok(()),
            None => Err(Spent::Deadline),
        }
    }
}

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
                "{}; silence is not evidence. The budget was {:?} wide",
                spent.as_str(),
                budget.deadline()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_budget_that_has_run_out_has_nothing_left_and_says_so() {
        let spent = Budget::until(Instant::now() - Duration::from_secs(1), 16);
        assert_eq!(spent.remaining(), None);
        assert_eq!(spent.checkpoint(), Err(Spent::Deadline));
    }

    #[test]
    fn cancelling_is_visible_to_the_work_even_while_time_is_left() {
        let budget = Budget::within(Duration::from_secs(600), 16);
        assert!(budget.checkpoint().is_ok());
        budget.cancel();
        assert_eq!(
            budget.checkpoint(),
            Err(Spent::Cancelled),
            "a blocking extractor can only stop if it can see that nobody is waiting"
        );
    }

    #[test]
    fn a_clone_shares_the_one_cancellation_everybody_is_watching() {
        let budget = Budget::within(Duration::from_secs(600), 16);
        let carried = budget.clone();
        budget.cancel();
        assert_eq!(
            carried.checkpoint(),
            Err(Spent::Cancelled),
            "the transport cancels the copy it kept; the copy the work holds has to see it"
        );
    }

    #[test]
    fn the_deadline_is_absolute_so_passing_it_on_cannot_widen_it() {
        let budget = Budget::within(Duration::from_secs(600), 16);
        let carried = budget.clone();
        assert_eq!(
            budget.deadline(),
            carried.deadline(),
            "a batch hands the same budget to every anchor; if handing it on restarted \
             the clock, a batch of 64 would be 64 times its own budget"
        );
    }
}
