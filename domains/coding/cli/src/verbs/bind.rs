use gmr::{AnchorKey, Ref, Runtime, Source};

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

    if detach {
        let cleared = rt.revoke(&reference, Source::Adjudicated).await?;
        return detached(&path, &address, &cleared, json);
    }

    let version = rt.current_version(&reference).await.unwrap_or(None);
    let anchors: Vec<AnchorKey> = anchors.into_iter().map(AnchorKey::new).collect();

    let landed = rt
        .bind(
            reference,
            anchors.clone(),
            version.clone(),
            Source::Adjudicated,
        )
        .await?;
    let anchors = landed.anchors.clone();

    if json {
        println!(
            "{}",
            serde_json::json!({ "bound": address, "version": version, "anchors": anchors, "detached": detach })
        );
    } else {
        println!(
            "{path} → {}",
            anchors
                .iter()
                .map(|a| a.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        match &version {
            Some(v) => {
                let v = v.as_str();
                println!("  bound version {}", &v[..12.min(v.len())]);
            }
            None => println!(
                "  no version: the store could not answer for this record, so nothing has \
                 been verified about it yet"
            ),
        }
        for (named, living) in &landed.moved {
            println!("  {named} is closed and superseded; this landed on {living}");
        }
    }
    Ok(0)
}

fn detached(path: &str, address: &str, cleared: &[AnchorKey], json: bool) -> Result<i32, CliError> {
    let named: Vec<String> = cleared.iter().map(|a| a.to_string()).collect();
    if json {
        println!(
            "{}",
            serde_json::json!({ "detached": address, "revoked_on": named })
        );
        return Ok(0);
    }
    match named.is_empty() {
        true => println!("{path} was on no anchor; nothing to revoke"),
        false => println!("{path} revoked on {}", named.join(", ")),
    }
    println!("  the assertions stay in the table, and so does this revocation");
    Ok(0)
}
