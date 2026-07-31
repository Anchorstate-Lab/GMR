use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use gmr_core::{Declaration, Facts, Kind, Outcome};
use serde_json::Value;
use tokio::process::Command;

use gmr_probe::{ProbeError, Transport};

pub const POSITION_ENV: &str = "GMR_POSITION";

pub struct Shell {
    kind: Kind,
    root: PathBuf,
    timeout: Duration,
    output_cap: usize,
}

impl Shell {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            kind: Kind::new("shell"),
            root: root.into(),
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
}

#[async_trait]
impl Transport for Shell {
    fn kind(&self) -> &Kind {
        &self.kind
    }

    async fn invoke(
        &self,
        declaration: &Declaration,
        position: &Value,
    ) -> Result<Outcome, ProbeError> {
        let run = declaration
            .0
            .get("run")
            .and_then(Value::as_str)
            .ok_or_else(|| ProbeError::unusable("shell 声明里缺 `run`"))?;

        let out = tokio::time::timeout(
            self.timeout,
            Command::new("sh")
                .arg("-c")
                .arg(run)
                .current_dir(&self.root)
                .env("LC_ALL", "C")
                .env("LANG", "C")
                .env(POSITION_ENV, position.to_string())
                .stdin(Stdio::null())
                .kill_on_drop(true)
                .output(),
        )
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

        if facts.is_null() {
            return Ok(Outcome::NotFound);
        }

        Ok(Outcome::Found {
            facts: Facts::new(facts),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gmr_core::ReasonClass;
    use serde_json::json;

    fn shell() -> Shell {
        Shell::new(".")
    }

    fn decl(run: &str) -> Declaration {
        Declaration(json!({ "run": run }))
    }

    async fn invoke(run: &str) -> Result<Outcome, ProbeError> {
        shell().invoke(&decl(run), &Value::Null).await
    }

    async fn at(run: &str, position: Value) -> Result<Outcome, ProbeError> {
        shell().invoke(&decl(run), &position).await
    }

    #[tokio::test]
    async fn structured_output_is_the_state_vector() {
        let out = invoke(r#"echo '{"count":2,"names":["a","b"]}'"#)
            .await
            .unwrap();
        let Outcome::Found { facts } = out else {
            panic!("该找到")
        };
        assert_eq!(facts.as_value()["count"], json!(2));
        assert_eq!(facts.as_value()["names"], json!(["a", "b"]));
    }

    #[tokio::test]
    async fn null_is_the_worlds_absence_and_it_is_a_real_answer() {
        assert_eq!(invoke("echo null").await.unwrap(), Outcome::NotFound);
    }

    #[tokio::test]
    async fn a_nonzero_exit_is_our_failure_not_the_worlds_answer() {
        let e = invoke("exit 1").await.unwrap_err();
        assert_eq!(e.reason, ReasonClass::Unreachable);
        assert!(e.message.contains("退出码 1"));
    }

    #[tokio::test]
    async fn a_crashing_script_does_not_masquerade_as_an_empty_world() {
        let e = invoke("jq-that-is-not-installed .").await.unwrap_err();
        assert_eq!(e.reason, ReasonClass::Unreachable);
    }

    #[tokio::test]
    async fn stderr_comes_back_so_the_failure_can_be_read() {
        let e = invoke("echo 'boom: no such credential' >&2; exit 3")
            .await
            .unwrap_err();
        assert!(e.message.contains("no such credential"), "{}", e.message);
    }

    #[tokio::test]
    async fn unstructured_output_is_a_failure_not_a_fact() {
        let e = invoke("echo hello").await.unwrap_err();
        assert_eq!(e.reason, ReasonClass::Unusable);
        assert!(e.message.contains("不是 JSON"));
    }

    #[tokio::test]
    async fn the_position_reaches_the_script() {
        let out = at(
            &format!(r#"echo "{{\"saw\": ${POSITION_ENV}}}""#),
            json!({ "file": "core/a.rs" }),
        )
        .await
        .unwrap();
        let Outcome::Found { facts } = out else {
            panic!("该找到")
        };
        assert_eq!(facts.as_value()["saw"], json!({ "file": "core/a.rs" }));
    }

    #[tokio::test]
    async fn the_transport_never_looks_into_the_position() {
        for p in [json!("a.rs"), json!({ "x": 1 }), json!([1, 2])] {
            let out = at(&format!(r#"echo "{{\"p\": ${POSITION_ENV}}}""#), p.clone())
                .await
                .unwrap();
            let Outcome::Found { facts } = out else {
                panic!("该找到")
            };
            assert_eq!(facts.as_value()["p"], p);
        }
    }

    #[tokio::test]
    async fn a_silent_probe_times_out_as_our_failure() {
        let e = shell()
            .with_timeout(Duration::from_millis(60))
            .invoke(&decl("sleep 5"), &Value::Null)
            .await
            .unwrap_err();
        assert_eq!(e.reason, ReasonClass::Unreachable);
        assert!(e.message.contains("沉默不是证据"));
    }

    #[tokio::test]
    async fn oversized_output_is_refused_never_truncated() {
        let e = shell()
            .with_output_cap(16)
            .invoke(&decl("printf 'x%.0s' $(seq 1 100)"), &Value::Null)
            .await
            .unwrap_err();
        assert_eq!(e.reason, ReasonClass::Unusable);
        assert!(e.message.contains("拒绝而不是截断"));
    }

    #[tokio::test]
    async fn same_script_same_position_same_answer() {
        let a = invoke("echo '{\"x\":1}'").await.unwrap();
        let b = invoke("echo '{\"x\":1}'").await.unwrap();
        assert_eq!(a, b);
    }
}
