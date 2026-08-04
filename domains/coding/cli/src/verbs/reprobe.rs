use std::path::Path;

use gmr::{AnchorKey, Change, Runtime};

use crate::error::CliError;
use crate::probes::Recipes;
use crate::verbs::sealed;

#[allow(clippy::too_many_arguments)]
pub async fn run(
    rt: &Runtime,
    root: &Path,
    key: String,
    probe_name: Option<String>,
    artifact: Option<String>,
    params: String,
    why: String,
    json: bool,
) -> Result<i32, CliError> {
    let key = AnchorKey::new(key);
    // Mirrors the declaration: a recipe name is what anchors.toml carries, so
    // revising to one must not force the author back to a raw hash.
    let version = match (probe_name, artifact) {
        (Some(_), Some(_)) | (None, None) => {
            return Err(CliError(
                "give either --probe (a recipe name) or --artifact (a version), not both".into(),
            ));
        }
        (Some(name), None) => Recipes::load(root)?
            .version_of(&name, root)?
            .as_str()
            .to_owned(),
        (None, Some(v)) => v,
    };
    let probe = crate::rules::probe(&version, &params)?;
    let revised = rt
        .revise(&key, Change::Reprobe { probe }, why.as_bytes())
        .await?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "reprobed": key,
                "context": revised.context,
                "rationale": revised.rationale,
                "incomparable_state": revised.incomparable_state,
            })
        );
    } else {
        println!("{key} changed probe");
        if revised.incomparable_state {
            println!(
                "  ! The state was derived by another rule and is not comparable to the new observation.\n    \
                 Either restate to recapture it, or explicitly accept cross-rule comparability; that is your assertion, the substrate only records it."
            );
        }
        sealed(&revised.context, &revised.rationale);
    }
    Ok(0)
}
