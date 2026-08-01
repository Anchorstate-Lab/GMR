pub mod artifact;
#[cfg(feature = "testkit")]
pub mod testkit;

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use gmr_core::{Derivation, Facts, Kind, Outcome, ProbeRef, Verifiability};
use serde_json::Value;
use tokio::process::Command;

use gmr_probe::{ProbeError, Sighted, Transport};

pub use artifact::{ArtifactError, Artifacts, publish};

pub const POSITION_ENV: &str = "GMR_POSITION";

pub const PARAMS_ENV: &str = "GMR_PARAMS";

/// 只执行 artifact。锚上写的是「哪一个 artifact」，这里把它解析出来、
/// 逐字节校验、再跑它的入口 —— 于是日志里那个版本号是挣来的。
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

    async fn invoke(&self, probe: &ProbeRef, position: &Value) -> Result<Sighted, ProbeError> {
        // 解析失败是 unusable：我们说不出这次会用哪条派生规则，就不该去跑。
        let resolved = self
            .artifacts
            .resolve(&probe.artifact)
            .map_err(|e| ProbeError::unusable(e.0))?;

        let mut command = Command::new(resolved.entrypoint());
        command
            .args(&resolved.manifest.args)
            .current_dir(&self.cwd)
            // 先铺默认，再让清单覆盖：清单声明什么就是什么，而它进版本号。
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
                ProbeError::unreachable(format!(
                    "探针在 {:?} 后仍未返回；它的沉默不是证据",
                    self.timeout
                ))
            })?
            .map_err(|e| ProbeError::unreachable(format!("探针跑不起来：{e}")))?;

        let size = out.stdout.len() + out.stderr.len();
        if size > self.output_cap {
            return Err(ProbeError::unusable(format!(
                "探针输出 {size} 字节，超过 {} 上限；**拒绝而不是截断** —— \
                 截断过的读数存成事实就是撒谎。让它打印结构，不要打印转储",
                self.output_cap
            )));
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
            return Err(ProbeError::unreachable(match out.status.code() {
                Some(code) => format!("探针以退出码 {code} 结束：{tail}"),
                None => format!("探针被信号打断：{tail}"),
            }));
        }

        let stdout = String::from_utf8_lossy(&out.stdout);
        let stdout = stdout.trim();

        let facts: Value = serde_json::from_str(stdout).map_err(|e| {
            ProbeError::unusable(format!(
                "探针吐的不是 JSON（{e}）；约定是「一个对象，或 null」。\
                 收到的开头是：{}",
                stdout.chars().take(120).collect::<String>()
            ))
        })?;

        let outcome = if facts.is_null() {
            Outcome::NotFound
        } else {
            Outcome::Found {
                facts: Facts::new(facts),
            }
        };

        Ok(Sighted {
            outcome,
            derivation: Derivation {
                version: probe.artifact.clone(),
                verifiability: Verifiability::ContentAddressed,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gmr_core::{ProbeVersion, ReasonClass};
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

        /// 发布一个 sh 脚本当探针。
        fn publish(&self, body: &str, args: &[&str]) -> ProbeVersion {
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
            publish(
                &Artifacts::new(&self.store),
                &src,
                Kind::new("shell"),
                "probe",
                args.iter().map(|a| (*a).to_owned()).collect(),
                Default::default(),
            )
            .unwrap()
        }

        async fn invoke(
            &self,
            v: &ProbeVersion,
            params: Value,
            at: Value,
        ) -> Result<Sighted, ProbeError> {
            self.shell()
                .invoke(&ProbeRef::new(Kind::new("shell"), v.clone(), params), &at)
                .await
        }
    }

    #[tokio::test]
    async fn structured_output_is_the_state_vector() {
        let w = World::new();
        let v = w.publish(r#"echo '{"count":2,"names":["a","b"]}'"#, &[]);
        let got = w.invoke(&v, json!({}), Value::Null).await.unwrap();
        let Outcome::Found { facts } = got.outcome else {
            panic!("该找到")
        };
        assert_eq!(facts.as_value()["count"], json!(2));
    }

    #[tokio::test]
    async fn the_version_is_the_artifact_that_actually_ran() {
        let w = World::new();
        let v = w.publish("echo '{}'", &[]);
        let got = w.invoke(&v, json!({}), Value::Null).await.unwrap();
        assert_eq!(got.derivation.version, v);
        assert_eq!(
            got.derivation.verifiability,
            Verifiability::ContentAddressed
        );
    }

    #[tokio::test]
    async fn changing_one_byte_of_the_probe_changes_its_version() {
        let w = World::new();
        assert_ne!(
            w.publish("echo '{\"x\":1}'", &[]),
            w.publish("echo '{\"x\":2}'", &[]),
            "改了实现就是换了派生规则 —— 版本必须跟着动"
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
        let v = w.publish("echo '{\"x\":1}'", &[]);
        let entry = Artifacts::new(&w.store).dir(&v).join("probe");
        std::fs::write(&entry, "#!/bin/sh\necho '{\"x\":999}'\n").unwrap();

        let e = w.invoke(&v, json!({}), Value::Null).await.unwrap_err();
        assert_eq!(e.reason, ReasonClass::Unusable);
        assert!(e.message.contains("拒绝执行"), "{}", e.message);
    }

    #[tokio::test]
    async fn an_artifact_that_is_not_there_is_our_failure() {
        let w = World::new();
        let e = w
            .invoke(&ProbeVersion::new("f".repeat(64)), json!({}), Value::Null)
            .await
            .unwrap_err();
        assert_eq!(e.reason, ReasonClass::Unusable);
    }

    #[tokio::test]
    async fn null_is_the_worlds_absence_and_it_is_a_real_answer() {
        let w = World::new();
        let v = w.publish("echo null", &[]);
        let got = w.invoke(&v, json!({}), Value::Null).await.unwrap();
        assert_eq!(got.outcome, Outcome::NotFound);
    }

    #[tokio::test]
    async fn a_nonzero_exit_is_our_failure_not_the_worlds_answer() {
        let w = World::new();
        let v = w.publish("exit 1", &[]);
        let e = w.invoke(&v, json!({}), Value::Null).await.unwrap_err();
        assert_eq!(e.reason, ReasonClass::Unreachable);
        assert!(e.message.contains("退出码 1"));
    }

    #[tokio::test]
    async fn stderr_comes_back_so_the_failure_can_be_read() {
        let w = World::new();
        let v = w.publish("echo 'boom: no such credential' >&2; exit 3", &[]);
        let e = w.invoke(&v, json!({}), Value::Null).await.unwrap_err();
        assert!(e.message.contains("no such credential"), "{}", e.message);
    }

    #[tokio::test]
    async fn unstructured_output_is_a_failure_not_a_fact() {
        let w = World::new();
        let v = w.publish("echo hello", &[]);
        let e = w.invoke(&v, json!({}), Value::Null).await.unwrap_err();
        assert_eq!(e.reason, ReasonClass::Unusable);
        assert!(e.message.contains("不是 JSON"));
    }

    #[tokio::test]
    async fn the_position_and_the_params_both_reach_the_probe() {
        let w = World::new();
        let v = w.publish(
            &format!(r#"echo "{{\"at\": ${POSITION_ENV}, \"with\": ${PARAMS_ENV}}}""#),
            &[],
        );
        let got = w
            .invoke(&v, json!({ "kind": "function" }), json!({ "file": "a.rs" }))
            .await
            .unwrap();
        let Outcome::Found { facts } = got.outcome else {
            panic!("该找到")
        };
        assert_eq!(facts.as_value()["at"], json!({ "file": "a.rs" }));
        assert_eq!(facts.as_value()["with"], json!({ "kind": "function" }));
    }

    #[tokio::test]
    async fn the_transport_never_looks_into_the_position() {
        let w = World::new();
        let v = w.publish(&format!(r#"echo "{{\"p\": ${POSITION_ENV}}}""#), &[]);
        for p in [json!("a.rs"), json!({ "x": 1 }), json!([1, 2])] {
            let got = w.invoke(&v, json!({}), p.clone()).await.unwrap();
            let Outcome::Found { facts } = got.outcome else {
                panic!("该找到")
            };
            assert_eq!(facts.as_value()["p"], p);
        }
    }

    #[tokio::test]
    async fn a_silent_probe_times_out_as_our_failure() {
        let w = World::new();
        let v = w.publish("sleep 5", &[]);
        let e = w
            .shell()
            .with_timeout(Duration::from_millis(60))
            .invoke(
                &ProbeRef::new(Kind::new("shell"), v, json!({})),
                &Value::Null,
            )
            .await
            .unwrap_err();
        assert_eq!(e.reason, ReasonClass::Unreachable);
        assert!(e.message.contains("沉默不是证据"));
    }

    #[tokio::test]
    async fn oversized_output_is_refused_never_truncated() {
        let w = World::new();
        let v = w.publish("printf 'x%.0s' $(seq 1 100)", &[]);
        let e = w
            .shell()
            .with_output_cap(16)
            .invoke(
                &ProbeRef::new(Kind::new("shell"), v, json!({})),
                &Value::Null,
            )
            .await
            .unwrap_err();
        assert_eq!(e.reason, ReasonClass::Unusable);
        assert!(e.message.contains("拒绝而不是截断"));
    }

    #[tokio::test]
    async fn same_artifact_same_position_same_answer() {
        let w = World::new();
        let v = w.publish("echo '{\"x\":1}'", &[]);
        let a = w.invoke(&v, json!({}), Value::Null).await.unwrap();
        let b = w.invoke(&v, json!({}), Value::Null).await.unwrap();
        assert_eq!(a.outcome, b.outcome);
        assert_eq!(a.derivation, b.derivation);
    }
}
