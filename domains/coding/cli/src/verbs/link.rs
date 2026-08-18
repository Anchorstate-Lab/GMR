use gmr::{LinkKind, Ref, Runtime};

use crate::error::CliError;

pub async fn run(
    rt: &Runtime,
    from: Ref,
    to: Ref,
    kind: String,
    json: bool,
) -> Result<i32, CliError> {
    let (from_ref, to_ref) = (from, to);
    let (from, to) = (
        from_ref.external_id.to_string(),
        to_ref.external_id.to_string(),
    );
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
