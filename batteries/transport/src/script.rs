use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;

use async_trait::async_trait;
use gmr_core::{Derivation, Facts, Kind, Observes, Openness, Outcome, ProbeName, Verifiability};
use serde_json::Value;
use tokio::process::Command;

use gmr_budget::Spent;
use gmr_probe::{PARAMS_ENV, POSITION_ENV, ProbeCall, ProbeError, ProbeErrorCode, Transport};

use crate::closure;

pub struct Script {
    kind: Kind,
    cwd: PathBuf,
    paths: BTreeMap<ProbeName, PathBuf>,
}

impl Script {
    pub fn new(cwd: impl Into<PathBuf>, paths: BTreeMap<ProbeName, PathBuf>) -> Self {
        Self {
            kind: Kind::new("script"),
            cwd: cwd.into(),
            paths,
        }
    }

    fn entry(&self, name: &ProbeName) -> Option<PathBuf> {
        Some(self.cwd.join(self.paths.get(name)?))
    }
}

#[async_trait]
impl Transport for Script {
    fn kind(&self) -> &Kind {
        &self.kind
    }

    fn resolve(&self, name: &ProbeName) -> Option<Derivation> {
        Some(Derivation {
            version: closure::of_path(&self.entry(name)?)?,
            observes: Observes::Unknown,
            verifiability: Verifiability::open([Openness::Interpreter, Openness::HostEnv]),
        })
    }

    async fn invoke(&self, call: &ProbeCall<'_>) -> Result<Outcome, ProbeError> {
        let name = &call.probe.name;
        let entry = self.entry(name).ok_or_else(|| {
            ProbeError::with_code(
                gmr_core::ReasonClass::Unusable,
                ProbeErrorCode::ArtifactInvalid,
                format!("no script is declared for the probe named `{name}`"),
            )
        })?;

        let mut command = Command::new(&entry);
        command
            .current_dir(&self.cwd)
            .env(POSITION_ENV, call.position.to_string())
            .env(PARAMS_ENV, call.probe.params.to_string())
            .stdin(Stdio::null())
            .kill_on_drop(true);

        let left = call
            .budget
            .remaining()
            .ok_or_else(|| ProbeError::spent(Spent::Deadline, call.budget))?;

        let out = tokio::time::timeout(left, command.output())
            .await
            .map_err(|_| ProbeError::spent(Spent::Deadline, call.budget))?
            .map_err(|e| ProbeError::unreachable(format!("cannot run {}: {e}", entry.display())))?;

        let size = out.stdout.len() + out.stderr.len();
        if size > call.budget.output_cap() {
            return Err(ProbeError::too_large(size, call.budget.output_cap()));
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
    use gmr_budget::Budget;
    use gmr_core::{ProbeRef, ReasonClass};
    use serde_json::json;
    use std::time::Duration;

    fn wide() -> Budget {
        Budget::within(Duration::from_secs(30), 1 << 20)
    }

    struct World {
        dir: tempfile::TempDir,
    }

    impl World {
        fn new() -> Self {
            Self {
                dir: tempfile::tempdir().unwrap(),
            }
        }

        fn write(&self, rel: &str, body: &str) {
            let path = self.dir.path().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
        }

        fn script(&self, rel: &str) -> Script {
            Script::new(
                self.dir.path(),
                BTreeMap::from([(ProbeName::new("deploy"), PathBuf::from(rel))]),
            )
        }

        async fn invoke(&self, rel: &str, position: Value) -> Result<Outcome, ProbeError> {
            self.spend(rel, position, wide()).await
        }

        async fn spend(
            &self,
            rel: &str,
            position: Value,
            budget: Budget,
        ) -> Result<Outcome, ProbeError> {
            let probe = ProbeRef::new(Kind::new("script"), ProbeName::new("deploy"), json!({}));
            self.script(rel)
                .invoke(&ProbeCall {
                    probe: &probe,
                    position: &position,
                    budget: &budget,
                })
                .await
        }
    }

    #[tokio::test]
    async fn a_script_answers_the_same_contract() {
        let w = World::new();
        w.write("scripts/deploy.sh", r#"echo '{"sha":"abc","age":3}'"#);
        let Outcome::Found { facts } = w.invoke("scripts/deploy.sh", Value::Null).await.unwrap()
        else {
            panic!("expected a found outcome")
        };
        assert_eq!(facts.as_value()["sha"], json!("abc"));
    }

    #[tokio::test]
    async fn the_identity_is_the_file_and_one_byte_moves_it() {
        let w = World::new();
        w.write("p.sh", "echo '{\"x\":1}'");
        let before = w.script("p.sh").resolve(&ProbeName::new("deploy")).unwrap();
        assert_eq!(
            before.verifiability,
            Verifiability::open([Openness::Interpreter, Openness::HostEnv]),
            "a script names what it does not close over, so a later grading can tell an \
             interpreter on PATH from a network call"
        );

        w.write("p.sh", "echo '{\"x\":2}'");
        let after = w.script("p.sh").resolve(&ProbeName::new("deploy")).unwrap();
        assert_ne!(before.version, after.version);
    }

    #[test]
    fn a_directory_probe_hashes_every_file_under_it() {
        let w = World::new();
        w.write("probe/run", "echo '{}'");
        w.write("probe/lib.sh", "helper() { :; }");
        let before = w
            .script("probe")
            .resolve(&ProbeName::new("deploy"))
            .unwrap();

        w.write("probe/lib.sh", "helper() { echo different; }");
        let after = w
            .script("probe")
            .resolve(&ProbeName::new("deploy"))
            .unwrap();
        assert_ne!(
            before.version, after.version,
            "a helper the entrypoint reads is part of the rule"
        );
    }

    #[test]
    fn moving_a_line_between_files_moves_the_version() {
        let w = World::new();
        w.write("probe/a", "one");
        w.write("probe/b", "two");
        let before = w
            .script("probe")
            .resolve(&ProbeName::new("deploy"))
            .unwrap();

        w.write("probe/a", "two");
        w.write("probe/b", "one");
        let after = w
            .script("probe")
            .resolve(&ProbeName::new("deploy"))
            .unwrap();
        assert_ne!(before.version, after.version);
    }

    #[test]
    fn a_script_that_is_not_there_resolves_to_nothing() {
        let w = World::new();
        assert!(
            w.script("absent.sh")
                .resolve(&ProbeName::new("deploy"))
                .is_none()
        );
    }

    #[tokio::test]
    async fn null_is_the_worlds_absence_and_it_is_a_real_answer() {
        let w = World::new();
        w.write("p.sh", "echo null");
        assert_eq!(
            w.invoke("p.sh", Value::Null).await.unwrap(),
            Outcome::NotFound
        );
    }

    #[tokio::test]
    async fn a_nonzero_exit_is_our_failure_not_the_worlds_answer() {
        let w = World::new();
        w.write("p.sh", "echo 'no credential' >&2; exit 2");
        let e = w.invoke("p.sh", Value::Null).await.unwrap_err();
        assert_eq!(e.reason, ReasonClass::Unreachable);
        assert_eq!(e.code, ProbeErrorCode::ProcessFailed);
        assert!(e.message.contains("no credential"), "{}", e.message);
    }

    #[tokio::test]
    async fn unstructured_output_is_a_failure_not_a_fact() {
        let w = World::new();
        w.write("p.sh", "echo hello");
        let e = w.invoke("p.sh", Value::Null).await.unwrap_err();
        assert_eq!(e.code, ProbeErrorCode::InvalidJson);
    }

    #[tokio::test]
    async fn the_position_reaches_the_script() {
        let w = World::new();
        w.write("p.sh", &format!(r#"echo "{{\"at\": ${POSITION_ENV}}}""#));
        let Outcome::Found { facts } = w.invoke("p.sh", json!({ "env": "staging" })).await.unwrap()
        else {
            panic!("expected a found outcome")
        };
        assert_eq!(facts.as_value()["at"], json!({ "env": "staging" }));
    }

    #[tokio::test]
    async fn the_environment_is_inherited() {
        let w = World::new();
        w.write("p.sh", r#"echo "{\"path\": \"$PATH\"}""#);
        let Outcome::Found { facts } = w.invoke("p.sh", Value::Null).await.unwrap() else {
            panic!("expected a found outcome")
        };
        assert!(!facts.as_value()["path"].as_str().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_silent_script_times_out_as_our_failure() {
        let w = World::new();
        w.write("p.sh", "sleep 5");
        let e = w
            .spend(
                "p.sh",
                Value::Null,
                Budget::within(Duration::from_millis(60), 1 << 20),
            )
            .await
            .unwrap_err();
        assert_eq!(e.code, ProbeErrorCode::TimedOut);
    }

    #[tokio::test]
    async fn a_script_is_killed_when_the_budget_runs_out_rather_than_left_running() {
        let w = World::new();
        let marker = w.dir.path().join("still-running");
        w.write("p.sh", &format!("sleep 2; printf x > {}", marker.display()));
        let e = w
            .spend(
                "p.sh",
                Value::Null,
                Budget::within(Duration::from_millis(60), 1 << 20),
            )
            .await
            .unwrap_err();
        assert_eq!(e.code, ProbeErrorCode::TimedOut);

        tokio::time::sleep(Duration::from_millis(2500)).await;
        assert!(
            !marker.exists(),
            "kill_on_drop has to make the timeout a real cancellation for a subprocess; \
             if the child ran to completion it only looked cancelled"
        );
    }

    #[tokio::test]
    async fn oversized_output_is_refused_never_truncated() {
        let w = World::new();
        w.write(
            "p.sh",
            "printf '{\"x\":\"%s\"}' $(printf 'y%.0s' $(seq 1 100))",
        );
        let e = w
            .spend(
                "p.sh",
                Value::Null,
                Budget::within(Duration::from_secs(30), 16),
            )
            .await
            .unwrap_err();
        assert_eq!(e.code, ProbeErrorCode::OutputTooLarge);
    }
}
