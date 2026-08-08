use std::path::Path;

use gmr::{AnchorKey, Ref, Runtime};

use crate::error::CliError;

pub async fn run(
    rt: &Runtime,
    root: &Path,
    path: String,
    anchors: Vec<String>,
    detach: bool,
    provider: String,
    json: bool,
) -> Result<i32, CliError> {
    if anchors.is_empty() && !detach {
        return Err(CliError("provide either --anchors or --detach".into()));
    }
    if provider == "git" && !root.join(&path).exists() {
        return Err(CliError(format!("`{path}` is not in this repository")));
    }
    let reference = Ref::new(provider, path.clone());
    let version = rt
        .memory()
        .current_version(&reference)
        .await?
        .ok_or_else(|| CliError(format!("no content provider could version `{path}`")))?;
    let anchors: Vec<AnchorKey> = anchors.into_iter().map(AnchorKey::new).collect();

    rt.bind(reference, anchors.clone(), version.clone()).await?;
    let version = version.into_inner();

    if json {
        println!(
            "{}",
            serde_json::json!({ "bound": path, "version": version, "anchors": anchors, "detached": detach })
        );
    } else if detach {
        println!("{path} detached; history remains in the table");
    } else {
        println!(
            "{path} → {}",
            anchors
                .iter()
                .map(|a| a.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!("  bound version {}", &version[..12.min(version.len())]);
    }
    Ok(0)
}
