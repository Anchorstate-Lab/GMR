use std::path::Path;

use gmr::{AnchorKey, Ref, Runtime, Version};

use crate::error::CliError;

pub async fn run(
    rt: &Runtime,
    root: &Path,
    path: String,
    anchors: Vec<String>,
    detach: bool,
    json: bool,
) -> Result<i32, CliError> {
    if anchors.is_empty() && !detach {
        return Err(CliError("要么给 --anchors，要么 --detach".into()));
    }
    if !root.join(&path).exists() {
        return Err(CliError(format!("`{path}` 不在这个仓库里")));
    }
    let version = gmr_provider_git::blob_version(root, &path).map_err(|e| CliError(e.message))?;
    let anchors: Vec<AnchorKey> = anchors.into_iter().map(AnchorKey::new).collect();

    rt.bind(
        Ref::new("git", path.clone()),
        anchors.clone(),
        Version::new(version.clone()),
        vec![],
    )
    .await?;

    if json {
        println!(
            "{}",
            serde_json::json!({ "bound": path, "version": version, "anchors": anchors, "detached": detach })
        );
    } else if detach {
        println!("{path} 已摘走（历史留在表里）");
    } else {
        println!(
            "{path} → {}",
            anchors
                .iter()
                .map(|a| a.to_string())
                .collect::<Vec<_>>()
                .join(" · ")
        );
        println!("  绑定时那一版 {}", &version[..12.min(version.len())]);
    }
    Ok(0)
}
