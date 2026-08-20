use gmr::{Retain, RunSettings};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub struct Declared {
    #[serde(default)]
    pub retain_full: Option<bool>,
    #[serde(default)]
    pub cadence_secs: Option<u64>,
    #[serde(default)]
    pub budget_ms: Option<u64>,
}

impl Declared {
    pub fn stated(retain_full: bool, cadence_secs: Option<u64>, budget_ms: Option<u64>) -> Self {
        Self {
            retain_full: retain_full.then_some(true),
            cadence_secs,
            budget_ms,
        }
    }

    pub fn at_open(&self) -> RunSettings {
        RunSettings {
            retain: match self.retain_full {
                Some(true) => Retain::Full,
                _ => Retain::Tick,
            },
            cadence_secs: self.cadence_secs,
            budget_ms: self.budget_ms,
        }
    }

    pub fn overlaid(&self, running: &RunSettings) -> Option<RunSettings> {
        let next = RunSettings {
            retain: match self.retain_full {
                Some(true) => Retain::Full,
                Some(false) => Retain::Tick,
                None => running.retain,
            },
            cadence_secs: self.cadence_secs.or(running.cadence_secs),
            budget_ms: self.budget_ms.or(running.budget_ms),
        };
        (next != *running).then_some(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn running(retain: Retain, cadence: Option<u64>, budget: Option<u64>) -> RunSettings {
        RunSettings {
            retain,
            cadence_secs: cadence,
            budget_ms: budget,
        }
    }

    #[test]
    fn a_declaration_that_says_nothing_changes_nothing() {
        let tuned = running(Retain::Full, Some(900), Some(7000));
        assert_eq!(
            Declared::default().overlaid(&tuned),
            None,
            "`about: <coordinate>` is one line and states no knob at all. Comparing a whole \
             RunSettings against one built from that line reset every knob the anchor was \
             opened with, on every sync, and reported it as `resettled` — a verb undoing \
             something no declaration anywhere could ask for"
        );
    }

    #[test]
    fn a_declaration_moves_the_knob_it_names_and_leaves_the_rest() {
        let tuned = running(Retain::Full, Some(900), Some(7000));
        let said = Declared {
            cadence_secs: Some(60),
            ..Default::default()
        };
        assert_eq!(
            said.overlaid(&tuned),
            Some(running(Retain::Full, Some(60), Some(7000)))
        );
    }

    #[test]
    fn every_value_is_still_reachable_by_saying_it() {
        let tuned = running(Retain::Full, Some(900), Some(7000));
        let back = Declared {
            retain_full: Some(false),
            cadence_secs: Some(300),
            budget_ms: Some(30_000),
        };
        assert_eq!(
            back.overlaid(&tuned),
            Some(running(Retain::Tick, Some(300), Some(30_000))),
            "unsaid means unchanged, so returning a knob to the deployment default is done \
             by naming that default rather than by deleting the line. `None` in the store \
             means the same thing as the default written out, so nothing is out of reach"
        );
    }

    #[test]
    fn opening_takes_the_deployment_default_for_whatever_was_not_said() {
        assert_eq!(
            Declared::default().at_open(),
            RunSettings::default(),
            "at open there is nothing to overwrite, so unsaid is the default rather than \
             unchanged — the two readings only differ once an anchor is running"
        );
        assert_eq!(
            Declared::stated(true, Some(60), Some(1000)).at_open(),
            running(Retain::Full, Some(60), Some(1000))
        );
    }

    #[test]
    fn a_knob_absent_is_not_a_knob_set_to_its_zero() {
        let tuned = running(Retain::Full, None, None);
        assert_eq!(Declared::default().overlaid(&tuned), None);
        assert_eq!(
            Declared {
                retain_full: Some(false),
                ..Default::default()
            }
            .overlaid(&tuned),
            Some(running(Retain::Tick, None, None)),
            "`retain_full = false` written out is a statement; the key being absent is not"
        );
    }
}
