use gmr::Runtime;

use crate::error::CliError;

pub async fn run(rt: &Runtime, key: String, why: String) -> Result<i32, CliError> {
    let key = super::resolve_one(rt, &key).await?;
    rt.close(&key, why.as_bytes()).await?;
    println!("{key} closed");
    Ok(0)
}
