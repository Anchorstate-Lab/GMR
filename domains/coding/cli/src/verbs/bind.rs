use gmr::{AnchorKey, Ref, Runtime};

use crate::error::CliError;

pub async fn run(
    rt: &Runtime,
    names: &crate::memories::Names,
    reference: Ref,
    anchors: Vec<String>,
    detach: bool,
    json: bool,
) -> Result<i32, CliError> {
    if anchors.is_empty() && !detach {
        return Err(CliError("provide either --anchors or --detach".into()));
    }
    let path = names.of(&reference);
    let address = crate::memories::addressed(&reference);
    let version = match detach {
        false => rt.current_version(&reference).await?.ok_or_else(|| {
            CliError(format!(
                "`{}` has no record `{}`",
                reference.provider, reference.external_id
            ))
        })?,
        true => rt
            .memory()
            .binding_of(&reference)
            .await?
            .map(|record| record.bound_version)
            .ok_or_else(|| {
                CliError(format!(
                    "`{address}` is not bound to anything — nothing to detach"
                ))
            })?,
    };
    let anchors: Vec<AnchorKey> = anchors.into_iter().map(AnchorKey::new).collect();

    rt.bind(reference, anchors.clone(), version.clone()).await?;
    let version = version.into_inner();

    if json {
        println!(
            "{}",
            serde_json::json!({ "bound": address, "version": version, "anchors": anchors, "detached": detach })
        );
    } else if detach {
        println!("{path} detached; history remains in the table");
    } else {
        println!(
            "{path} → {}",
            anchors
                .iter()
                .map(|a| a.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!("  bound version {}", &version[..12.min(version.len())]);
    }
    Ok(0)
}
