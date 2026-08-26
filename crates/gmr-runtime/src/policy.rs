use std::time::Duration;

use gmr_budget::Budget;

#[derive(Debug, Clone)]
pub struct Policy {
    pub cadence_secs: u64,
    pub lease_secs: u64,
    pub backoff_base_secs: u64,
    pub backoff_cap_secs: u64,
    pub batch: usize,
    pub stalled_attempts: u32,
    pub stalled_staleness_secs: i64,
    pub probe_budget_ms: u64,
    pub probe_output_cap: usize,
    pub content_call_ms: u64,
    pub content_total_ms: u64,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            cadence_secs: 300,
            lease_secs: 60,
            backoff_base_secs: 30,
            backoff_cap_secs: 3600,
            batch: 64,
            stalled_attempts: 3,
            stalled_staleness_secs: 24 * 3600,
            probe_budget_ms: 30_000,
            probe_output_cap: 1024 * 1024,
            content_call_ms: 5_000,
            content_total_ms: 30_000,
        }
    }
}

impl Policy {
    pub fn backoff_secs(&self, attempts: u32) -> i64 {
        let shift = attempts.saturating_sub(1).min(16);
        (self.backoff_base_secs.saturating_mul(1 << shift)).min(self.backoff_cap_secs) as i64
    }

    pub fn budget(&self) -> Budget {
        Budget::within(
            Duration::from_millis(self.probe_budget_ms),
            self.probe_output_cap,
        )
    }

    pub fn content_budget(&self) -> Budget {
        Budget::within(Duration::from_millis(self.content_total_ms), usize::MAX)
    }

    pub fn content_call(&self) -> Duration {
        Duration::from_millis(self.content_call_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_and_caps() {
        let p = Policy::default();
        assert_eq!(p.backoff_secs(1), 30);
        assert_eq!(p.backoff_secs(2), 60);
        assert_eq!(p.backoff_secs(3), 120);
        assert_eq!(p.backoff_secs(30), 3600);
    }
}
