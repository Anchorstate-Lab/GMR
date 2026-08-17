use gmr::{Ref, Runtime};

use crate::error::CliError;

pub async fn run(
    rt: &Runtime,
    path: String,
    provider: String,
    json: bool,
) -> Result<i32, CliError> {
    let reference = Ref::new(provider.clone(), path.clone());
    let version = rt
        .current_version(&reference)
        .await?
        .ok_or_else(|| CliError(format!("`{provider}` has no record `{path}`")))?;

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
