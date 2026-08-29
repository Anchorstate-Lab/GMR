use gmr::{Ref, Runtime};

use crate::error::CliError;

pub async fn run(
    rt: &Runtime,
    names: &crate::memories::Names,
    reference: Ref,
    json: bool,
) -> Result<i32, CliError> {
    let path = names.of(&reference);
    let address = crate::memories::addressed(&reference);
    let version = rt.current_version(&reference).await?.ok_or_else(|| {
        CliError(format!(
            "`{}` has no record `{}`",
            reference.provider, reference.external_id
        ))
    })?;

    rt.reaffirm(&reference.clone().into(), Some(version.clone())).await?;
    let version = version.into_inner();

    if json {
        println!(
            "{}",
            serde_json::json!({ "reaffirmed": address, "version": version })
        );
    } else {
        println!("{path} reaffirmed");
        println!("  bound version {}", &version[..12.min(version.len())]);
    }
    Ok(0)
}
