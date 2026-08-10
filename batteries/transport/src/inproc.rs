use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use gmr_core::{Derivation, Facts, Kind, Outcome, ProbeName, ProbeVersion, Verifiability};
use serde_json::Value;

use gmr_probe::{ProbeCall, ProbeError, ProbeErrorCode, Transport};

pub use gmr_probe::{Budget, Spent};

pub struct Reach {
    pub cwd: PathBuf,
    pub position: Value,
    pub params: Value,
    pub budget: Budget,
}

#[derive(Debug)]
pub enum ExtractError {
    Spent(Spent),
    Refused(String),
}

impl std::fmt::Display for ExtractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spent(spent) => f.write_str(spent.as_str()),
            Self::Refused(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for ExtractError {}

pub type Extract = dyn Fn(&Reach) -> Result<Value, ExtractError> + Send + Sync;

pub struct Registered {
    pub version: ProbeVersion,
    pub extract: Arc<Extract>,
}

pub struct InProcess {
    kind: Kind,
    cwd: PathBuf,
    probes: BTreeMap<ProbeName, Registered>,
}

impl InProcess {
    pub fn new(cwd: impl Into<PathBuf>, probes: BTreeMap<ProbeName, Registered>) -> Self {
        Self {
            kind: Kind::new("builtin"),
            cwd: cwd.into(),
            probes,
        }
    }

    pub fn names(&self) -> impl Iterator<Item = &ProbeName> {
        self.probes.keys()
    }
}

#[async_trait]
impl Transport for InProcess {
    fn kind(&self) -> &Kind {
        &self.kind
    }

    fn resolve(&self, name: &ProbeName) -> Option<Derivation> {
        Some(Derivation {
            version: self.probes.get(name)?.version.clone(),
            verifiability: Verifiability::Closed,
        })
    }

    async fn invoke(&self, call: &ProbeCall<'_>) -> Result<Outcome, ProbeError> {
        let name = &call.probe.name;
        let registered = self.probes.get(name).ok_or_else(|| {
            ProbeError::with_code(
                gmr_core::ReasonClass::Unusable,
                ProbeErrorCode::ArtifactInvalid,
                format!("no probe named `{name}` is linked into this build"),
            )
        })?;

        let extract = Arc::clone(&registered.extract);
        let reach = Reach {
            cwd: self.cwd.clone(),
            position: call.position.clone(),
            params: call.probe.params.clone(),
            budget: call.budget.clone(),
        };
        let work = tokio::task::spawn_blocking(move || extract(&reach));

        let Some(left) = call.budget.remaining() else {
            call.budget.cancel();
            return Err(ProbeError::spent(Spent::Deadline, call.budget));
        };

        let joined = match tokio::time::timeout(left, work).await {
            Ok(joined) => joined,
            Err(_) => {
                call.budget.cancel();
                return Err(ProbeError::spent(Spent::Deadline, call.budget));
            }
        };

        let facts = joined
            .map_err(|e| {
                ProbeError::with_code(
                    gmr_core::ReasonClass::Unreachable,
                    ProbeErrorCode::ProcessFailed,
                    format!("probe `{name}` panicked: {e}"),
                )
            })?
            .map_err(|e| match e {
                ExtractError::Spent(spent) => ProbeError::spent(spent, call.budget),
                ExtractError::Refused(why) => {
                    ProbeError::unreachable(format!("probe `{name}`: {why}"))
                }
            })?;

        let size = facts.to_string().len();
        if size > call.budget.output_cap() {
            return Err(ProbeError::too_large(size, call.budget.output_cap()));
        }

        Ok(match facts.is_null() {
            true => Outcome::NotFound,
            false => Outcome::Found {
                facts: Facts::new(facts),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gmr_core::ProbeRef;
    use serde_json::json;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    fn version(c: &str) -> ProbeVersion {
        ProbeVersion::new(c.repeat(64))
    }

    fn linked(name: &str, extract: Arc<Extract>) -> InProcess {
        InProcess::new(
            ".",
            BTreeMap::from([(
                ProbeName::new(name),
                Registered {
                    version: version("a"),
                    extract,
                },
            )]),
        )
    }

    fn transport(
        name: &str,
        f: impl Fn() -> Result<Value, ExtractError> + Send + Sync + 'static,
    ) -> InProcess {
        linked(name, Arc::new(move |_| f()))
    }

    fn probe(name: &str) -> ProbeRef {
        ProbeRef::new(Kind::new("builtin"), ProbeName::new(name), json!({}))
    }

    fn wide() -> Budget {
        Budget::within(Duration::from_secs(30), 1 << 20)
    }

    fn call<'a>(probe: &'a ProbeRef, position: &'a Value, budget: &'a Budget) -> ProbeCall<'a> {
        ProbeCall {
            probe,
            position,
            budget,
        }
    }

    #[tokio::test]
    async fn structured_output_is_the_state_vector() {
        let t = transport("p", || Ok(json!({ "count": 2 })));
        let (p, b) = (probe("p"), wide());
        let Outcome::Found { facts } = t.invoke(&call(&p, &Value::Null, &b)).await.unwrap() else {
            panic!("expected a found outcome")
        };
        assert_eq!(facts.as_value()["count"], json!(2));
    }

    #[tokio::test]
    async fn null_is_the_worlds_absence_and_it_is_a_real_answer() {
        let t = transport("p", || Ok(Value::Null));
        let (p, b) = (probe("p"), wide());
        assert_eq!(
            t.invoke(&call(&p, &Value::Null, &b)).await.unwrap(),
            Outcome::NotFound
        );
    }

    #[tokio::test]
    async fn a_refusal_is_our_failure_not_the_worlds_answer() {
        let t = transport("p", || {
            Err(ExtractError::Refused("no such credential".into()))
        });
        let (p, b) = (probe("p"), wide());
        let e = t.invoke(&call(&p, &Value::Null, &b)).await.unwrap_err();
        assert_eq!(e.reason, gmr_core::ReasonClass::Unreachable);
        assert!(e.message.contains("no such credential"), "{}", e.message);
    }

    #[tokio::test]
    async fn a_panic_is_recorded_not_propagated() {
        let t = transport("p", || panic!("index out of bounds"));
        let (p, b) = (probe("p"), wide());
        let e = t.invoke(&call(&p, &Value::Null, &b)).await.unwrap_err();
        assert_eq!(e.code, ProbeErrorCode::ProcessFailed);
        assert_eq!(e.reason, gmr_core::ReasonClass::Unreachable);
    }

    #[tokio::test]
    async fn a_silent_probe_times_out_as_our_failure() {
        let t = transport("p", || {
            std::thread::sleep(Duration::from_millis(800));
            Ok(json!({}))
        });
        let (p, b) = (
            probe("p"),
            Budget::within(Duration::from_millis(60), 1 << 20),
        );
        let e = t.invoke(&call(&p, &Value::Null, &b)).await.unwrap_err();
        assert_eq!(e.code, ProbeErrorCode::TimedOut);
    }

    #[tokio::test]
    async fn a_probe_handed_a_budget_that_is_already_gone_is_not_even_started() {
        let ran = Arc::new(AtomicBool::new(false));
        let started = Arc::clone(&ran);
        let t = linked(
            "p",
            Arc::new(move |_| {
                started.store(true, Ordering::SeqCst);
                Ok(json!({}))
            }),
        );
        let (p, b) = (
            probe("p"),
            Budget::until(std::time::Instant::now(), 1 << 20),
        );
        let e = t.invoke(&call(&p, &Value::Null, &b)).await.unwrap_err();
        assert_eq!(e.code, ProbeErrorCode::TimedOut);
    }

    #[tokio::test]
    async fn work_that_outran_its_budget_is_told_nobody_is_waiting() {
        let noticed = Arc::new(AtomicBool::new(false));
        let saw = Arc::clone(&noticed);
        let t = linked(
            "p",
            Arc::new(move |reach: &Reach| {
                let began = std::time::Instant::now();
                while began.elapsed() < Duration::from_secs(5) {
                    if let Err(spent) = reach.budget.checkpoint() {
                        saw.store(true, Ordering::SeqCst);
                        return Err(ExtractError::Spent(spent));
                    }
                    std::thread::sleep(Duration::from_millis(2));
                }
                Ok(json!({}))
            }),
        );

        let (p, b) = (
            probe("p"),
            Budget::within(Duration::from_millis(60), 1 << 20),
        );
        let e = t.invoke(&call(&p, &Value::Null, &b)).await.unwrap_err();
        assert_eq!(e.code, ProbeErrorCode::TimedOut);

        for _ in 0..200 {
            if noticed.load(Ordering::SeqCst) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(
            noticed.load(Ordering::SeqCst),
            "giving up on the race is not cancelling: a blocking extractor can only stop \
             if the abandonment reaches it, and this is the signal it reads"
        );
    }

    #[tokio::test]
    async fn oversized_output_is_refused_never_truncated() {
        let t = transport("p", || Ok(json!({ "x": "y".repeat(100) })));
        let (p, b) = (probe("p"), Budget::within(Duration::from_secs(30), 16));
        let e = t.invoke(&call(&p, &Value::Null, &b)).await.unwrap_err();
        assert_eq!(e.code, ProbeErrorCode::OutputTooLarge);
    }

    #[tokio::test]
    async fn a_name_nothing_is_linked_under_is_our_failure() {
        let t = transport("p", || Ok(json!({})));
        let (p, b) = (probe("absent"), wide());
        let e = t.invoke(&call(&p, &Value::Null, &b)).await.unwrap_err();
        assert_eq!(e.code, ProbeErrorCode::ArtifactInvalid);
        assert_eq!(e.reason, gmr_core::ReasonClass::Unusable);
    }

    #[test]
    fn the_identity_answered_before_the_call_is_the_one_that_runs() {
        let t = transport("p", || Ok(json!({})));
        assert_eq!(
            t.resolve(&ProbeName::new("p")).unwrap().version,
            version("a")
        );
        assert_eq!(
            t.resolve(&ProbeName::new("p")).unwrap().verifiability,
            Verifiability::Closed
        );
        assert!(t.resolve(&ProbeName::new("absent")).is_none());
    }

    #[tokio::test]
    async fn the_position_and_the_params_both_reach_the_probe() {
        let t = linked(
            "p",
            Arc::new(|reach: &Reach| Ok(json!({ "at": reach.position, "with": reach.params }))),
        );
        let p = ProbeRef::new(
            Kind::new("builtin"),
            ProbeName::new("p"),
            json!({ "root": "src" }),
        );
        let (at, b) = (json!({ "file": "a.rs" }), wide());
        let Outcome::Found { facts } = t.invoke(&call(&p, &at, &b)).await.unwrap() else {
            panic!("expected a found outcome")
        };
        assert_eq!(facts.as_value()["at"], json!({ "file": "a.rs" }));
        assert_eq!(facts.as_value()["with"], json!({ "root": "src" }));
    }
}
