use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

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
    started: Instant,
    deadline: Instant,
    output_cap: usize,
    cancel: Arc<AtomicBool>,
    inherited: Vec<Arc<AtomicBool>>,
}

impl Budget {
    pub fn until(deadline: Instant, output_cap: usize) -> Self {
        Self {
            started: Instant::now(),
            deadline,
            output_cap,
            cancel: Arc::new(AtomicBool::new(false)),
            inherited: Vec::new(),
        }
    }

    pub fn within(span: Duration, output_cap: usize) -> Self {
        Self::until(Instant::now() + span, output_cap)
    }

    pub fn deadline(&self) -> Instant {
        self.deadline
    }

    pub fn width(&self) -> Duration {
        self.deadline.saturating_duration_since(self.started)
    }

    pub fn output_cap(&self) -> usize {
        self.output_cap
    }

    pub fn remaining(&self) -> Option<Duration> {
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|left| !left.is_zero())
    }

    pub fn narrowed(&self, span: Duration) -> Self {
        let mut inherited = self.inherited.clone();
        inherited.push(Arc::clone(&self.cancel));
        Self {
            inherited,
            ..Self::until((Instant::now() + span).min(self.deadline), self.output_cap)
        }
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    fn abandoned(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
            || self
                .inherited
                .iter()
                .any(|flag| flag.load(Ordering::SeqCst))
    }

    pub fn checkpoint(&self) -> Result<(), Spent> {
        if self.abandoned() {
            return Err(Spent::Cancelled);
        }
        match self.remaining() {
            Some(_) => Ok(()),
            None => Err(Spent::Deadline),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

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
    fn narrowing_can_only_tighten_a_budget_never_widen_it() {
        let batch = Budget::within(Duration::from_millis(50), 16);
        assert!(
            batch.narrowed(Duration::from_secs(3600)).deadline() <= batch.deadline(),
            "an anchor asking for an hour inside a batch worth 50ms must not get an hour, \
             or a per-anchor knob becomes a way around the batch's own bound"
        );
        assert!(batch.narrowed(Duration::from_millis(1)).deadline() < batch.deadline());
    }

    #[test]
    fn narrowing_gives_the_anchor_its_own_cancellation() {
        let batch = Budget::within(Duration::from_secs(600), 16);
        let one = batch.narrowed(Duration::from_secs(600));
        one.cancel();
        assert!(
            batch.checkpoint().is_ok(),
            "one anchor giving up must not cancel the rest of the batch with it"
        );
    }

    #[test]
    fn a_narrowed_budget_still_hears_the_batch_that_minted_it_give_up() {
        let batch = Budget::within(Duration::from_secs(600), 16);
        let one = batch.narrowed(Duration::from_secs(600));
        batch.cancel();
        assert_eq!(
            one.checkpoint(),
            Err(Spent::Cancelled),
            "cancellation runs down the tree, never up. An anchor with a per-anchor budget \
             that stopped inheriting the batch's cancellation would keep scanning after the \
             whole batch was abandoned — and which anchors do that would depend on whether \
             someone had set a per-anchor budget, which is not a difference anyone declared"
        );
    }

    #[test]
    fn a_grandchild_hears_the_whole_line_above_it() {
        let batch = Budget::within(Duration::from_secs(600), 16);
        let one = batch.narrowed(Duration::from_secs(600));
        let inner = one.narrowed(Duration::from_secs(600));
        batch.cancel();
        assert_eq!(inner.checkpoint(), Err(Spent::Cancelled));
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
