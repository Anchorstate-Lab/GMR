use gmr::{AnchorKey, Runtime};

use crate::{error::CliError, render};

pub async fn run(
    rt: &Runtime,
    key: Option<String>,
    moved_only: bool,
    json: bool,
) -> Result<i32, CliError> {
    let views = match key {
        Some(k) => vec![rt.read(&AnchorKey::new(k)).await?],
        None => rt.read_all().await?,
    };

    let shown: Vec<_> = views
        .iter()
        .filter(|v| !moved_only || v.attempts > 0 || v.status.is_some())
        .collect();

    if json {
        println!("{}", serde_json::to_string_pretty(&shown)?);
    } else if shown.is_empty() {
        println!("no anchors are moving.");
    } else {
        for v in &shown {
            print!("{}", render::anchor(v));
        }
    }
    Ok(0)
}
