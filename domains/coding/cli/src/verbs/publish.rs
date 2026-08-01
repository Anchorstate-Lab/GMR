use std::path::Path;

use gmr::Kind;
use gmr_transport_shell::{Artifacts, publish};

use crate::error::CliError;

/// Publish a directory as a probe artifact and print its earned version.
pub fn run(
    root: &Path,
    from: String,
    entrypoint: String,
    args: Vec<String>,
    env: Vec<String>,
    json: bool,
) -> Result<i32, CliError> {
    let from = root.join(&from);
    // Declared env enters the manifest and therefore the version; it is part of
    // the derivation closure.
    let env = env
        .iter()
        .map(|kv| {
            kv.split_once('=')
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .ok_or_else(|| CliError(format!("--env must be K=V, got `{kv}`")))
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
        println!("  this version is earned; changing one byte creates another derivation rule");
    }
    Ok(0)
}
