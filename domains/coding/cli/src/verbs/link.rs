use gmr::{LinkKind, Ref, Runtime};

use crate::error::CliError;

pub async fn run(
    rt: &Runtime,
    from: String,
    to: String,
    kind: String,
    from_provider: String,
    to_provider: String,
    json: bool,
) -> Result<i32, CliError> {
    let from_ref = Ref::new(from_provider, from.clone());
    let to_ref = Ref::new(to_provider, to.clone());
    rt.link(&from_ref, &to_ref, LinkKind(kind.clone())).await?;

    if json {
        println!(
            "{}",
            serde_json::json!({ "from": from, "to": to, "kind": kind })
        );
    } else {
        println!("{from} --{kind}--> {to}");
    }
    Ok(0)
}
