use gmr::{Instructions, Runtime};

use crate::{error::CliError, render};

pub async fn run(
    rt: &Runtime,
    names: &crate::memories::Names,
    key: Option<String>,
    fresher_than_secs: Option<u64>,
    json: bool,
) -> Result<i32, CliError> {
    let how = Instructions {
        max_staleness: fresher_than_secs.map(std::time::Duration::from_secs),
        budget: None,
    };
    let views = match key {
        Some(k) => {
            let mut out = Vec::new();
            for key in super::resolve(rt, &k).await? {
                out.push(rt.grounded_within(&key, &how).await?);
            }
            out
        }
        None => rt.grounded_all_within(&how).await?,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&views)?);
    } else if views.is_empty() {
        println!("no anchors.");
    } else {
        for v in &views {
            print!("{}", render::anchor(v, names));
        }
    }
    Ok(0)
}
