use gmr::{Instructions, Runtime};

use crate::{error::CliError, render};

pub async fn run(
    rt: &Runtime,
    key: String,
    fresher_than_secs: Option<u64>,
    json: bool,
) -> Result<i32, CliError> {
    let how = Instructions {
        max_staleness: fresher_than_secs.map(std::time::Duration::from_secs),
        ..Instructions::default()
    };
    let mut out = Vec::new();
    for key in super::resolve(rt, &key).await? {
        out.push(rt.sample(&key, &how).await?);
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(0);
    }
    for reading in &out {
        print!("{}", render::reading(reading));
    }
    Ok(0)
}
