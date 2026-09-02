use gmr::{Claim, Ref, Runtime, Source};

use crate::error::CliError;

pub async fn run(
    rt: &Runtime,
    names: &crate::memories::Names,
    said: String,
    reference: Ref,
    source: String,
    json: bool,
) -> Result<i32, CliError> {
    let Some(Claim::Said { id, .. }) = Claim::parse(&said) else {
        return Err(CliError(format!(
            "`{said}` is not an utterance -- condense runs from `said:<id>` into the record \
             it became"
        )));
    };
    let source = Source::parse(&source).ok_or_else(|| {
        CliError(format!(
            "`{source}` is not a provenance: derived, self_attested, adjudicated, configured, \
             or unknown"
        ))
    })?;
    let landed = rt.condense(&id, reference.clone(), source).await?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "said": id.as_str(),
                "into": crate::memories::addressed(&reference),
                "landed": landed,
            })
        );
        return Ok(0);
    }
    println!(
        "said:{} condensed into {} on {} anchor(s); the utterance is revoked and the \
         binding carries its origin",
        id.as_str(),
        names.of(&reference),
        landed.anchors.len()
    );
    Ok(0)
}
