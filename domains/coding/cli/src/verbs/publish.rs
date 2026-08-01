use std::path::Path;

use gmr::Kind;
use gmr_transport_shell::{Artifacts, publish};

use crate::error::CliError;

/// 把一棵目录发布成探针 artifact，打印它挣来的版本号。
pub fn run(
    root: &Path,
    from: String,
    entrypoint: String,
    args: Vec<String>,
    env: Vec<String>,
    json: bool,
) -> Result<i32, CliError> {
    let from = root.join(&from);
    // 声明的 env 进清单，也就进版本号：它是派生规则闭包的一部分。
    let env = env
        .iter()
        .map(|kv| {
            kv.split_once('=')
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .ok_or_else(|| CliError(format!("--env 要写成 K=V，收到 `{kv}`")))
        })
        .collect::<Result<_, _>>()?;

    let version = publish(
        &Artifacts::new(crate::probes_dir(root)),
        &from,
        Kind::new("shell"),
        &entrypoint,
        args,
        env,
    )
    .map_err(|e| CliError(e.0))?;

    if json {
        println!("{}", serde_json::json!({ "artifact": version }));
    } else {
        println!("{version}");
        println!("  这是它挣来的版本 —— 改一个字节就是另一个号，也就是另一条派生规则");
    }
    Ok(0)
}
