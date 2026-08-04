use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use gmr_core::{Derivation, Facts, Kind, Outcome, ProbeName, ProbeRef, Verifiability};
use serde_json::Value;
use tokio::process::Command;

use gmr_probe::{PARAMS_ENV, POSITION_ENV, ProbeError, ProbeErrorCode, Transport};

use crate::closure;

/// Runs a file in the user's own repository. Identity is that file's content,
/// hashed at call time — the script does not get to say what it is.
///
/// [`Verifiability::Open`] always: the interpreter that reads the script is not
/// in the hash, and the environment is inherited rather than cleared. Clearing
/// it would only mean the user cannot find their own python; the honest move is
/// to inherit and say the closure is open.
pub struct Script {
    kind: Kind,
    cwd: PathBuf,
    paths: BTreeMap<ProbeName, PathBuf>,
    timeout: Duration,
    output_cap: usize,
}

impl Script {
    pub fn new(cwd: impl Into<PathBuf>, paths: BTreeMap<ProbeName, PathBuf>) -> Self {
        Self {
            kind: Kind::new("script"),
            cwd: cwd.into(),
            paths,
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
            verifiability: Verifiability::Open,
        })
    }

    async fn invoke(&self, probe: &ProbeRef, position: &Value) -> Result<Outcome, ProbeError> {
        let entry = self.entry(&probe.name).ok_or_else(|| {
            ProbeError::with_code(
                gmr_core::ReasonClass::Unusable,
                ProbeErrorCode::ArtifactInvalid,
                format!("no script is declared for the probe named `{}`", probe.name),
            )
        })?;

        let mut command = Command::new(&entry);
        command
            .current_dir(&self.cwd)
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
            .map_err(|e| ProbeError::unreachable(format!("cannot run {}: {e}", entry.display())))?;

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
    use gmr_core::ReasonClass;
    use serde_json::json;

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
            self.script(rel)
                .invoke(
                    &ProbeRef::new(Kind::new("script"), ProbeName::new("deploy"), json!({})),
                    &position,
                )
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

    /// The script does not get to say what it is.
    #[tokio::test]
    async fn the_identity_is_the_file_and_one_byte_moves_it() {
        let w = World::new();
        w.write("p.sh", "echo '{\"x\":1}'");
        let before = w.script("p.sh").resolve(&ProbeName::new("deploy")).unwrap();
        assert_eq!(before.verifiability, Verifiability::Open);

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

    /// Moving code between files changes nothing about the bytes; it does change
    /// what runs.
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

    /// Clearing it would only mean the user cannot find their own interpreter.
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
            .script("p.sh")
            .with_timeout(Duration::from_millis(60))
            .invoke(
                &ProbeRef::new(Kind::new("script"), ProbeName::new("deploy"), json!({})),
                &Value::Null,
            )
            .await
            .unwrap_err();
        assert_eq!(e.code, ProbeErrorCode::TimedOut);
    }
}
