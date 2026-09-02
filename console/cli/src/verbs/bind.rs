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
    if detach {
        let path = names.of(&reference);
        let address = crate::memories::addressed(&reference);
        let cleared = rt
            .revoke(&reference.clone().into(), Source::Adjudicated)
            .await?;
        return detached(&path, &address, &cleared, json);
    }
    asserted(rt, names, reference, anchors, Source::Adjudicated, json).await
}

pub async fn attest(
    rt: &Runtime,
    names: &crate::memories::Names,
    reference: Ref,
    anchors: Vec<String>,
    json: bool,
) -> Result<i32, CliError> {
    asserted(rt, names, reference, anchors, Source::SelfAttested, json).await
}

pub async fn assert_on(
    rt: &Runtime,
    reference: Ref,
    anchors: Vec<AnchorKey>,
    source: Source,
) -> Result<(Option<gmr::Version>, gmr::Landed), CliError> {
    let version = rt.current_version(&reference).await.unwrap_or(None);
    let landed = rt
        .bind(
            gmr::Binding::on(reference, anchors),
            version.clone(),
            Default::default(),
            source,
        )
        .await?;
    Ok((version, landed))
}

async fn asserted(
    rt: &Runtime,
    names: &crate::memories::Names,
    reference: Ref,
    anchors: Vec<String>,
    source: Source,
    json: bool,
) -> Result<i32, CliError> {
    let path = names.of(&reference);
    let address = crate::memories::addressed(&reference);

    let anchors: Vec<AnchorKey> = anchors.into_iter().map(AnchorKey::new).collect();
    let (version, landed) = assert_on(rt, reference, anchors, source).await?;
    let anchors = landed.anchors.clone();

    if json {
        println!(
            "{}",
            serde_json::json!({
                "bound": address, "version": version, "anchors": anchors,
                "source": source.as_str(), "vouched": source.independent(),
                "recorded": landed.recorded,
            })
        );
        return Ok(0);
    }

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
    if !source.independent() {
        println!(
            "  only the writer of this record says it is about these anchors; nothing \
             else has read it"
        );
    }
    for (named, living) in &landed.moved {
        println!("  {named} is closed and superseded; this landed on {living}");
    }
    if !landed.recorded {
        println!("  nothing written: these anchors, this version and this reading already stand");
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
