use gmr::{Ref, Runtime};

use crate::error::CliError;

pub async fn run(
    rt: &Runtime,
    path: String,
    provider: String,
    json: bool,
) -> Result<i32, CliError> {
    let reference = Ref::new(provider, path.clone());
    let others = rt.cobound(&reference).await?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "path": path,
                "cobound": others.iter().map(|r| &r.external_id).collect::<Vec<_>>(),
            })
        );
        return Ok(0);
    }

    if others.is_empty() {
        println!("{path} shares no anchor with any other bound reference");
    } else {
        println!("{path} is co-bound with:");
        for other in &others {
            println!("  {}", other.external_id);
        }
    }
    Ok(0)
}
