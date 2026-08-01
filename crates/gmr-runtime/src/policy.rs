#[derive(Debug, Clone)]
pub struct Policy {
    pub cadence_secs: u64,
    pub lease_secs: u64,
    pub backoff_base_secs: u64,
    pub backoff_cap_secs: u64,
    pub batch: usize,
    pub stalled_attempts: u32,
    pub stalled_staleness_secs: i64,
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
        }
    }
}

impl Policy {
    pub fn backoff_secs(&self, attempts: u32) -> i64 {
        let shift = attempts.saturating_sub(1).min(16);
        (self.backoff_base_secs.saturating_mul(1 << shift)).min(self.backoff_cap_secs) as i64
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
