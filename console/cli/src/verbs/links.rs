use gmr::{Ref, Runtime};

use crate::error::CliError;

pub async fn run(
    rt: &Runtime,
    names: &crate::memories::Names,
    reference: Ref,
    json: bool,
) -> Result<i32, CliError> {
    let held = rt.links(&reference).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&held)?);
        return Ok(0);
    }

    let path = names.of(&reference);
    if held.out.is_empty() && held.incoming.is_empty() {
        println!("{path} touches no live edge");
        return Ok(0);
    }
    for edge in &held.out {
        println!(
            "{path} --{}--> {}  [{}]",
            edge.kind.0,
            names.of(&edge.to),
            edge.source
        );
    }
    for edge in &held.incoming {
        println!(
            "{} --{}--> {path}  [{}]",
            names.of(&edge.from),
            edge.kind.0,
            edge.source
        );
    }
    Ok(0)
}
