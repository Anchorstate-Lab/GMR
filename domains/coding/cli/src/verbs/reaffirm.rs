use std::path::Path;

use gmr::{Ref, Runtime};

use crate::error::CliError;

pub async fn run(rt: &Runtime, root: &Path, path: String, json: bool) -> Result<i32, CliError> {
    if !root.join(&path).exists() {
        return Err(CliError(format!("`{path}` is not in this repository")));
    }
    let reference = Ref::new("git", path.clone());
    let version = rt
        .memory()
        .current_version(&reference)
        .await?
        .ok_or_else(|| CliError(format!("no content provider could version `{path}`")))?;

    rt.reaffirm(&reference, version.clone()).await?;
    let version = version.into_inner();

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
