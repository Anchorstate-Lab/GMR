use gmr::{LinkKind, LinkRevocation, Ref, Runtime, Source};

use crate::error::CliError;

pub async fn run(
    rt: &Runtime,
    from: Ref,
    to: Ref,
    kind: String,
    detach: bool,
    json: bool,
) -> Result<i32, CliError> {
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
                source: Source::Adjudicated,
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

    rt.link(
        &from_ref,
        &to_ref,
        LinkKind(kind.clone()),
        Source::Adjudicated,
    )
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
