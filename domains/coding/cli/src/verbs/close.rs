use gmr::{AnchorKey, Runtime};

use crate::error::CliError;

pub async fn run(rt: &Runtime, key: String, why: String) -> Result<i32, CliError> {
    let key = AnchorKey::new(key);
    rt.close(&key, why.as_bytes()).await?;
    println!("{key} closed");
    Ok(0)
}
