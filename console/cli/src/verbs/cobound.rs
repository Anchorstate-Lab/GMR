use gmr::{Claim, Ref, Runtime};

use crate::error::CliError;

pub async fn run(
    rt: &Runtime,
    names: &crate::memories::Names,
    reference: Ref,
    json: bool,
) -> Result<i32, CliError> {
    let path = names.of(&reference);
    let others: Vec<Claim> = rt.cobound(&reference.clone().into()).await?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "path": path,
                "cobound": others.iter().map(Claim::to_string).collect::<Vec<_>>(),
            })
        );
        return Ok(0);
    }

    if others.is_empty() {
        println!("{path} shares no anchor with any other bound claim");
    } else {
        println!("{path} is co-bound with:");
        for other in &others {
            match other {
                Claim::Stored(reference) => println!("  {}", names.of(reference)),
                said => println!("  {said}"),
            }
        }
    }
    Ok(0)
}
