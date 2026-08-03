use std::path::Path;

use gmr::{Ref, Runtime, Version};

use crate::error::CliError;

pub async fn run(rt: &Runtime, root: &Path, path: String, json: bool) -> Result<i32, CliError> {
    if !root.join(&path).exists() {
        return Err(CliError(format!("`{path}` is not in this repository")));
    }
    let version = gmr_provider_git::blob_version(root, &path).map_err(|e| CliError(e.message))?;

    rt.reaffirm(
        &Ref::new("git", path.clone()),
        Version::new(version.clone()),
    )
    .await?;

    if json {
        println!(
            "{}",
            serde_json::json!({ "reaffirmed": path, "version": version })
        );
    } else {
        println!("{path} reaffirmed");
        println!("  bound version {}", &version[..12.min(version.len())]);
    }
    Ok(0)
}
