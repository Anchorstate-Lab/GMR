use std::path::Path;

use gmr::{Kind, ProbeName};
use gmr_transport::closure;
use gmr_transport::shell::{Artifacts, publish};

use crate::error::CliError;

/// Install a directory as the probe called `name`. An artifact nothing can name
/// is unreachable, so publishing and naming are one step.
pub fn run(
    root: &Path,
    from: String,
    name: String,
    entrypoint: String,
    args: Vec<String>,
    env: Vec<String>,
    json: bool,
) -> Result<i32, CliError> {
    let name = ProbeName::try_new(&name).map_err(CliError)?;
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

    let derivation = closure::of_path(&from)
        .ok_or_else(|| CliError(format!("cannot read {}", from.display())))?;
    let artifacts = Artifacts::new(crate::probes_dir(root));
    let address = publish(
        &artifacts,
        &from,
        Kind::new("shell"),
        derivation.clone(),
        &entrypoint,
        args,
        env,
    )
    .map_err(|e| CliError(e.0))?;
    artifacts
        .install(&name, &address)
        .map_err(|e| CliError(e.0))?;

    if json {
        println!(
            "{}",
            serde_json::json!({ "probe": name, "derivation": derivation, "address": address })
        );
    } else {
        println!("{name}  {derivation}");
        println!("  anchors name it `{name}`; the journal records the derivation above.");
        println!("  {address} is where it lives here, and what verification checks.");
    }
    Ok(0)
}
