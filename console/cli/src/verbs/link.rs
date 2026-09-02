use gmr::{LinkKind, LinkRevocation, Ref, Runtime, Source};

use crate::error::CliError;

pub async fn run(
    rt: &Runtime,
    from: Ref,
    to: Ref,
    kind: String,
    detach: bool,
    source: String,
    json: bool,
) -> Result<i32, CliError> {
    let source = Source::parse(&source).ok_or_else(|| {
        CliError(format!(
            "`{source}` is not a provenance: derived, self_attested, adjudicated, configured, \
             or unknown"
        ))
    })?;
    let (from_ref, to_ref) = (from, to);
    let (from, to) = (
        from_ref.external_id.to_string(),
        to_ref.external_id.to_string(),
    );

    if detach {
        let revoked = rt
            .unlink(&LinkRevocation {
                from: from_ref,
                to: to_ref,
                kind: LinkKind(kind.clone()),
                asserted_as: None,
                source,
                when: chrono::Utc::now(),
            })
            .await?;
        if json {
            println!(
                "{}",
                serde_json::json!({ "detached": { "from": from, "to": to, "kind": kind }, "revoked": revoked })
            );
        } else {
            match revoked {
                0 => println!("no live edge {from} --{kind}--> {to}; nothing to revoke"),
                n => println!("{from} --{kind}--> {to} revoked ({n} assertion(s))"),
            }
        }
        return Ok(0);
    }

    rt.link(&from_ref, &to_ref, LinkKind(kind.clone()), source)
        .await?;

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
