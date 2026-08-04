use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use gmr_core::{
    Derivation, Facts, Kind, Outcome, ProbeName, ProbeRef, ProbeVersion, Verifiability,
};
use serde_json::Value;

use gmr_probe::{ProbeError, ProbeErrorCode, Transport};

/// `(cwd, position, params) -> facts | null`. Same contract the subprocess
/// probes answer on stdout, minus the process.
pub type Extract = dyn Fn(&Path, &Value, &Value) -> Result<Value, String> + Send + Sync;

/// One probe, and the hash of everything that can change what it returns.
pub struct Registered {
    pub version: ProbeVersion,
    pub extract: Arc<Extract>,
}

/// Runs probes linked into this binary. Which probes exist, and what each one's
/// closure hashes over, are the assembly's to state — this only carries them.
///
/// [`Verifiability::Closed`] holds by construction: the version handed to
/// [`Transport::resolve`] belongs to the very function [`Transport::invoke`]
/// then calls.
pub struct InProcess {
    kind: Kind,
    cwd: PathBuf,
    probes: BTreeMap<ProbeName, Registered>,
    timeout: Duration,
    output_cap: usize,
}

impl InProcess {
    pub fn new(cwd: impl Into<PathBuf>, probes: BTreeMap<ProbeName, Registered>) -> Self {
        Self {
            kind: Kind::new("builtin"),
            cwd: cwd.into(),
            probes,
            timeout: Duration::from_secs(30),
            output_cap: 1024 * 1024,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_output_cap(mut self, bytes: usize) -> Self {
        self.output_cap = bytes;
        self
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

    async fn invoke(&self, probe: &ProbeRef, position: &Value) -> Result<Outcome, ProbeError> {
        let registered = self.probes.get(&probe.name).ok_or_else(|| {
            ProbeError::with_code(
                gmr_core::ReasonClass::Unusable,
                ProbeErrorCode::ArtifactInvalid,
                format!("no probe named `{}` is linked into this build", probe.name),
            )
        })?;

        let extract = Arc::clone(&registered.extract);
        let cwd = self.cwd.clone();
        let position = position.clone();
        let params = probe.params.clone();
        let work = tokio::task::spawn_blocking(move || extract(&cwd, &position, &params));

        // Giving up waiting is all we can do: a blocking thread cannot be
        // cancelled, so it runs to completion unobserved. That is the price of
        // not paying for a process boundary, and it belongs in the open.
        let joined = tokio::time::timeout(self.timeout, work)
            .await
            .map_err(|_| {
                ProbeError::with_code(
                    gmr_core::ReasonClass::Unreachable,
                    ProbeErrorCode::TimedOut,
                    format!(
                        "probe did not return within {:?}; silence is not evidence",
                        self.timeout
                    ),
                )
            })?;

        // A panic here would otherwise take the whole process with it, and a
        // crash is not an entry. It is our failure, and it gets recorded as one.
        let facts = joined
            .map_err(|e| {
                ProbeError::with_code(
                    gmr_core::ReasonClass::Unreachable,
                    ProbeErrorCode::ProcessFailed,
                    format!("probe `{}` panicked: {e}", probe.name),
                )
            })?
            .map_err(|e| ProbeError::unreachable(format!("probe `{}`: {e}", probe.name)))?;

        let size = facts.to_string().len();
        if size > self.output_cap {
            return Err(ProbeError::with_code(
                gmr_core::ReasonClass::Unusable,
                ProbeErrorCode::OutputTooLarge,
                format!(
                    "probe output is {size} bytes, above the {} byte limit; refusing to truncate. \
                     Storing a truncated reading as fact would be a lie. Print structure, not dumps",
                    self.output_cap
                ),
            ));
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
    use serde_json::json;

    fn version(c: &str) -> ProbeVersion {
        ProbeVersion::new(c.repeat(64))
    }

    fn transport(
        name: &str,
        f: impl Fn() -> Result<Value, String> + Send + Sync + 'static,
    ) -> InProcess {
        InProcess::new(
            ".",
            BTreeMap::from([(
                ProbeName::new(name),
                Registered {
                    version: version("a"),
                    extract: Arc::new(move |_, _, _| f()),
                },
            )]),
        )
    }

    fn probe(name: &str) -> ProbeRef {
        ProbeRef::new(Kind::new("builtin"), ProbeName::new(name), json!({}))
    }

    #[tokio::test]
    async fn structured_output_is_the_state_vector() {
        let t = transport("p", || Ok(json!({ "count": 2 })));
        let Outcome::Found { facts } = t.invoke(&probe("p"), &Value::Null).await.unwrap() else {
            panic!("expected a found outcome")
        };
        assert_eq!(facts.as_value()["count"], json!(2));
    }

    #[tokio::test]
    async fn null_is_the_worlds_absence_and_it_is_a_real_answer() {
        let t = transport("p", || Ok(Value::Null));
        assert_eq!(
            t.invoke(&probe("p"), &Value::Null).await.unwrap(),
            Outcome::NotFound
        );
    }

    #[tokio::test]
    async fn a_refusal_is_our_failure_not_the_worlds_answer() {
        let t = transport("p", || Err("no such credential".into()));
        let e = t.invoke(&probe("p"), &Value::Null).await.unwrap_err();
        assert_eq!(e.reason, gmr_core::ReasonClass::Unreachable);
        assert!(e.message.contains("no such credential"), "{}", e.message);
    }

    /// Without this the whole CLI dies and nothing is written down.
    #[tokio::test]
    async fn a_panic_is_recorded_not_propagated() {
        let t = transport("p", || panic!("index out of bounds"));
        let e = t.invoke(&probe("p"), &Value::Null).await.unwrap_err();
        assert_eq!(e.code, ProbeErrorCode::ProcessFailed);
        assert_eq!(e.reason, gmr_core::ReasonClass::Unreachable);
    }

    #[tokio::test]
    async fn a_silent_probe_times_out_as_our_failure() {
        let t = transport("p", || {
            std::thread::sleep(Duration::from_secs(5));
            Ok(json!({}))
        })
        .with_timeout(Duration::from_millis(60));
        let e = t.invoke(&probe("p"), &Value::Null).await.unwrap_err();
        assert_eq!(e.code, ProbeErrorCode::TimedOut);
    }

    #[tokio::test]
    async fn oversized_output_is_refused_never_truncated() {
        let t = transport("p", || Ok(json!({ "x": "y".repeat(100) }))).with_output_cap(16);
        let e = t.invoke(&probe("p"), &Value::Null).await.unwrap_err();
        assert_eq!(e.code, ProbeErrorCode::OutputTooLarge);
    }

    #[tokio::test]
    async fn a_name_nothing_is_linked_under_is_our_failure() {
        let t = transport("p", || Ok(json!({})));
        let e = t.invoke(&probe("absent"), &Value::Null).await.unwrap_err();
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
        let t = InProcess::new(
            ".",
            BTreeMap::from([(
                ProbeName::new("p"),
                Registered {
                    version: version("a"),
                    extract: Arc::new(|_, pos, params| Ok(json!({ "at": pos, "with": params }))),
                },
            )]),
        );
        let p = ProbeRef::new(
            Kind::new("builtin"),
            ProbeName::new("p"),
            json!({ "root": "src" }),
        );
        let Outcome::Found { facts } = t.invoke(&p, &json!({ "file": "a.rs" })).await.unwrap()
        else {
            panic!("expected a found outcome")
        };
        assert_eq!(facts.as_value()["at"], json!({ "file": "a.rs" }));
        assert_eq!(facts.as_value()["with"], json!({ "root": "src" }));
    }
}
