use gmr::{AnchorKey, Runtime};

use crate::{error::CliError, render};

pub async fn run(rt: &Runtime, key: Option<String>, json: bool) -> Result<i32, CliError> {
    let views = match key {
        Some(k) => vec![rt.read(&AnchorKey::new(k)).await?],
        None => rt.read_all().await?,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&views)?);
    } else if views.is_empty() {
        println!("no anchors.");
    } else {
        for v in &views {
            print!("{}", render::anchor(v));
        }
    }
    Ok(0)
}
