pub mod artifact;
pub mod manifest;
#[cfg(feature = "testkit")]
pub mod testkit;

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use gmr_core::{Derivation, Facts, Kind, Outcome, ProbeName, ProbeRef, Verifiability};
use serde_json::Value;
use tokio::process::Command;

use gmr_probe::{PARAMS_ENV, POSITION_ENV, ProbeError, ProbeErrorCode, Transport};

pub use artifact::{ArtifactError, Artifacts, publish};
pub use manifest::{FileEntry, MANIFEST_SCHEMA, Manifest, Platform};

/// Executes only artifacts. The anchor names a probe; the store says which
/// artifact stands for that name here, verified byte for byte before it runs.
pub struct Shell {
    kind: Kind,
    cwd: PathBuf,
    artifacts: Artifacts,
    timeout: Duration,
    output_cap: usize,
}

impl Shell {
    pub fn new(cwd: impl Into<PathBuf>, artifacts: impl Into<PathBuf>) -> Self {
        Self {
            kind: Kind::new("shell"),
            cwd: cwd.into(),
            artifacts: Artifacts::new(artifacts),
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

    pub fn artifacts(&self) -> &Artifacts {
        &self.artifacts
    }
}

#[async_trait]
impl Transport for Shell {
    fn kind(&self) -> &Kind {
        &self.kind
    }

    fn resolve(&self, name: &ProbeName) -> Option<Derivation> {
        let resolved = self.artifacts.resolve(name).ok()?;
        Some(Derivation {
            version: resolved.manifest.version(),
            verifiability: match resolved.manifest.env.is_empty() {
                true => Verifiability::Closed,
                false => Verifiability::Open,
            },
        })
    }

    async fn invoke(&self, probe: &ProbeRef, position: &Value) -> Result<Outcome, ProbeError> {
        // Resolution failure is unusable: if we cannot name the derivation rule,
        // we should not run it.
        let resolved = self.artifacts.resolve(&probe.name).map_err(|e| {
            ProbeError::with_code(
                gmr_core::ReasonClass::Unusable,
                ProbeErrorCode::ArtifactInvalid,
                e.0,
            )
        })?;

        let mut command = Command::new(resolved.entrypoint());
        command
            .args(&resolved.manifest.args)
            .current_dir(&self.cwd)
            // Start with defaults, then let the manifest override them. What the
            // manifest declares is what runs, and it enters the version.
            .env_clear()
            .env("LC_ALL", "C")
            .env("LANG", "C")
            .env("PATH", "/usr/bin:/bin")
            .envs(&resolved.manifest.env)
            .env(POSITION_ENV, position.to_string())
            .env(PARAMS_ENV, probe.params.to_string())
            .stdin(Stdio::null())
            .kill_on_drop(true);

        let out = tokio::time::timeout(self.timeout, command.output())
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
            })?
            .map_err(|e| ProbeError::unreachable(format!("cannot run probe: {e}")))?;

        let size = out.stdout.len() + out.stderr.len();
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

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let tail: String = stderr
                .trim()
                .chars()
                .rev()
                .take(400)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            return Err(ProbeError::with_code(
                gmr_core::ReasonClass::Unreachable,
                ProbeErrorCode::ProcessFailed,
                match out.status.code() {
                    Some(code) => format!("probe exited with status {code}: {tail}"),
                    None => format!("probe was interrupted by a signal: {tail}"),
                },
            ));
        }

        let stdout = String::from_utf8_lossy(&out.stdout);
        let stdout = stdout.trim();

        let facts: Value = serde_json::from_str(stdout).map_err(|e| {
            ProbeError::with_code(
                gmr_core::ReasonClass::Unusable,
                ProbeErrorCode::InvalidJson,
                format!(
                    "probe output is not JSON ({e}); the contract is an object or null. \
                     Received prefix: {}",
                    stdout.chars().take(120).collect::<String>()
                ),
            )
        })?;

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
    use gmr_core::{ProbeVersion, ReasonClass};
    use gmr_probe::ProbeErrorCode;
    use serde_json::json;

    struct World {
        _dir: tempfile::TempDir,
        cwd: PathBuf,
        store: PathBuf,
    }

    impl World {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let cwd = dir.path().join("cwd");
            let store = dir.path().join("probes");
            std::fs::create_dir_all(&cwd).unwrap();
            std::fs::create_dir_all(&store).unwrap();
            Self {
                _dir: dir,
                cwd,
                store,
            }
        }

        fn shell(&self) -> Shell {
            Shell::new(&self.cwd, &self.store)
        }

        fn probe(params: Value) -> ProbeRef {
            ProbeRef::new(Kind::new("shell"), ProbeName::new("p"), params)
        }

        fn publish_with_env(&self, body: &str, env: &[(&str, &str)]) -> ProbeVersion {
            self.publish_full(body, &[], env)
        }

        /// Publish an sh script as a probe.
        fn publish(&self, body: &str, args: &[&str]) -> ProbeVersion {
            self.publish_full(body, args, &[])
        }

        fn publish_full(&self, body: &str, args: &[&str], env: &[(&str, &str)]) -> ProbeVersion {
            let src = self._dir.path().join("src");
            let _ = std::fs::remove_dir_all(&src);
            std::fs::create_dir_all(&src).unwrap();
            let entry = src.join("probe");
            std::fs::write(&entry, format!("#!/bin/sh\n{body}\n")).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&entry, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
            let version = publish(
                &Artifacts::new(&self.store),
                &src,
                Kind::new("shell"),
                "probe",
                args.iter().map(|a| (*a).to_owned()).collect(),
                env.iter()
                    .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                    .collect(),
            )
            .unwrap();
            Artifacts::new(&self.store)
                .install(&ProbeName::new("p"), &version)
                .unwrap();
            version
        }

        async fn invoke(&self, params: Value, at: Value) -> Result<Outcome, ProbeError> {
            self.shell().invoke(&Self::probe(params), &at).await
        }

        fn resolve(&self) -> Option<Derivation> {
            self.shell().resolve(&ProbeName::new("p"))
        }
    }

    #[tokio::test]
    async fn structured_output_is_the_state_vector() {
        let w = World::new();
        w.publish(r#"echo '{"count":2,"names":["a","b"]}'"#, &[]);
        let got = w.invoke(json!({}), Value::Null).await.unwrap();
        let Outcome::Found { facts } = got else {
            panic!("expected a found outcome")
        };
        assert_eq!(facts.as_value()["count"], json!(2));
    }

    #[tokio::test]
    async fn the_version_is_the_artifact_that_actually_ran() {
        let w = World::new();
        let v = w.publish("echo '{}'", &[]);
        let d = w.resolve().expect("a published artifact resolves");
        assert_eq!(d.version, v);
        assert_eq!(d.verifiability, Verifiability::Closed);
    }

    #[tokio::test]
    async fn resolving_names_the_rule_without_running_it() {
        let w = World::new();
        let v = w.publish("echo 'never ran' >&2; exit 9", &[]);
        assert_eq!(w.resolve().unwrap().version, v);
    }

    #[test]
    fn a_name_nothing_stands_for_resolves_to_nothing() {
        let w = World::new();
        w.publish("echo '{}'", &[]);
        assert!(w.shell().resolve(&ProbeName::new("absent")).is_none());
    }

    #[tokio::test]
    async fn changing_one_byte_of_the_probe_changes_its_version() {
        let w = World::new();
        assert_ne!(
            w.publish("echo '{\"x\":1}'", &[]),
            w.publish("echo '{\"x\":2}'", &[]),
            "changing the implementation changes the derivation rule, so the version must change"
        );
    }

    #[tokio::test]
    async fn changing_only_the_args_changes_the_version() {
        let w = World::new();
        assert_ne!(
            w.publish("echo '{}'", &["--mode", "a"]),
            w.publish("echo '{}'", &["--mode", "b"])
        );
    }

    #[tokio::test]
    async fn a_tampered_artifact_is_refused_not_run() {
        let w = World::new();
        w.publish("echo '{\"x\":1}'", &[]);
        let entry = Artifacts::new(&w.store)
            .resolve(&ProbeName::new("p"))
            .unwrap()
            .root
            .join("probe");
        std::fs::write(&entry, "#!/bin/sh\necho '{\"x\":999}'\n").unwrap();

        let e = w.invoke(json!({}), Value::Null).await.unwrap_err();
        assert_eq!(e.reason, ReasonClass::Unusable);
        assert_eq!(e.code, ProbeErrorCode::ArtifactInvalid);
    }

    /// Our failure, not the world's absence; it must never fold into the state.
    #[tokio::test]
    async fn a_probe_that_is_not_installed_is_our_failure() {
        let w = World::new();
        let e = w.invoke(json!({}), Value::Null).await.unwrap_err();
        assert_eq!(e.reason, ReasonClass::Unusable);
        assert_eq!(e.code, ProbeErrorCode::ArtifactInvalid);
        assert!(e.message.contains('p'), "{}", e.message);
    }

    #[tokio::test]
    async fn null_is_the_worlds_absence_and_it_is_a_real_answer() {
        let w = World::new();
        w.publish("echo null", &[]);
        let got = w.invoke(json!({}), Value::Null).await.unwrap();
        assert_eq!(got, Outcome::NotFound);
    }

    #[tokio::test]
    async fn a_nonzero_exit_is_our_failure_not_the_worlds_answer() {
        let w = World::new();
        w.publish("exit 1", &[]);
        let e = w.invoke(json!({}), Value::Null).await.unwrap_err();
        assert_eq!(e.reason, ReasonClass::Unreachable);
        assert_eq!(e.code, ProbeErrorCode::ProcessFailed);
    }

    #[tokio::test]
    async fn stderr_comes_back_so_the_failure_can_be_read() {
        let w = World::new();
        w.publish("echo 'boom: no such credential' >&2; exit 3", &[]);
        let e = w.invoke(json!({}), Value::Null).await.unwrap_err();
        assert_eq!(e.code, ProbeErrorCode::ProcessFailed);
        assert!(e.message.contains("no such credential"), "{}", e.message);
    }

    #[tokio::test]
    async fn unstructured_output_is_a_failure_not_a_fact() {
        let w = World::new();
        w.publish("echo hello", &[]);
        let e = w.invoke(json!({}), Value::Null).await.unwrap_err();
        assert_eq!(e.reason, ReasonClass::Unusable);
        assert_eq!(e.code, ProbeErrorCode::InvalidJson);
    }

    #[tokio::test]
    async fn the_position_and_the_params_both_reach_the_probe() {
        let w = World::new();
        w.publish(
            &format!(r#"echo "{{\"at\": ${POSITION_ENV}, \"with\": ${PARAMS_ENV}}}""#),
            &[],
        );
        let got = w
            .invoke(json!({ "kind": "function" }), json!({ "file": "a.rs" }))
            .await
            .unwrap();
        let Outcome::Found { facts } = got else {
            panic!("expected a found outcome")
        };
        assert_eq!(facts.as_value()["at"], json!({ "file": "a.rs" }));
        assert_eq!(facts.as_value()["with"], json!({ "kind": "function" }));
    }

    #[tokio::test]
    async fn the_transport_never_looks_into_the_position() {
        let w = World::new();
        w.publish(&format!(r#"echo "{{\"p\": ${POSITION_ENV}}}""#), &[]);
        for p in [json!("a.rs"), json!({ "x": 1 }), json!([1, 2])] {
            let got = w.invoke(json!({}), p.clone()).await.unwrap();
            let Outcome::Found { facts } = got else {
                panic!("expected a found outcome")
            };
            assert_eq!(facts.as_value()["p"], p);
        }
    }

    #[tokio::test]
    async fn a_silent_probe_times_out_as_our_failure() {
        let w = World::new();
        w.publish("sleep 5", &[]);
        let e = w
            .shell()
            .with_timeout(Duration::from_millis(60))
            .invoke(&World::probe(json!({})), &Value::Null)
            .await
            .unwrap_err();
        assert_eq!(e.reason, ReasonClass::Unreachable);
        assert_eq!(e.code, ProbeErrorCode::TimedOut);
    }

    #[tokio::test]
    async fn oversized_output_is_refused_never_truncated() {
        let w = World::new();
        w.publish("printf 'x%.0s' $(seq 1 100)", &[]);
        let e = w
            .shell()
            .with_output_cap(16)
            .invoke(&World::probe(json!({})), &Value::Null)
            .await
            .unwrap_err();
        assert_eq!(e.reason, ReasonClass::Unusable);
        assert_eq!(e.code, ProbeErrorCode::OutputTooLarge);
    }

    #[tokio::test]
    async fn same_artifact_same_position_same_answer() {
        let w = World::new();
        w.publish("echo '{\"x\":1}'", &[]);
        let a = w.invoke(json!({}), Value::Null).await.unwrap();
        let b = w.invoke(json!({}), Value::Null).await.unwrap();
        assert_eq!(a, b);
        assert_eq!(w.resolve(), w.resolve());
    }

    #[test]
    fn the_derivation_is_the_installed_artifact_not_the_name() {
        let w = World::new();
        let built = w.publish("echo '{\"x\":1}'", &[]);
        assert_eq!(
            w.resolve().expect("an installed name resolves").version,
            built
        );
    }

    #[test]
    fn a_name_nothing_was_installed_under_stands_for_nothing() {
        let w = World::new();
        assert_eq!(
            Artifacts::new(&w.store)
                .installed(&ProbeName::new("never"))
                .unwrap(),
            None
        );
    }

    /// Otherwise an upgrade leaves this machine running the old binary forever.
    #[test]
    fn reinstalling_a_name_repoints_it() {
        let w = World::new();
        let first = w.publish("echo '{\"x\":1}'", &[]);
        let second = w.publish("echo '{\"x\":2}'", &[]);
        assert_ne!(first, second);
        assert_eq!(
            Artifacts::new(&w.store)
                .installed(&ProbeName::new("p"))
                .unwrap(),
            Some(second)
        );
    }

    #[test]
    fn an_install_index_from_a_future_schema_is_refused() {
        let w = World::new();
        std::fs::write(
            w.store.join(super::artifact::INSTALL_FILE),
            br#"{"schema":"gmr.probe-install.v99","installed":{}}"#,
        )
        .unwrap();
        let e = Artifacts::new(&w.store)
            .installed(&ProbeName::new("p"))
            .unwrap_err();
        assert!(e.0.contains("v99"), "{}", e.0);
    }

    /// Host env reaches the probe without entering the hash, so the closure is
    /// open however exactly the artifact is pinned.
    #[test]
    fn host_env_opens_the_closure() {
        let w = World::new();
        w.publish("echo '{}'", &[]);
        assert_eq!(w.resolve().unwrap().verifiability, Verifiability::Closed);

        w.publish_with_env("echo '{}'", &[("HOME", "/somewhere")]);
        assert_eq!(w.resolve().unwrap().verifiability, Verifiability::Open);
    }
}
