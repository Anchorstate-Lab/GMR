use gmr::{AnchorKey, Change, Runtime};

use crate::error::CliError;
use crate::rules;
use crate::verbs::sealed;

pub async fn run(
    rt: &Runtime,
    key: String,
    rules_text: Vec<String>,
    why: String,
    json: bool,
) -> Result<i32, CliError> {
    let key = AnchorKey::new(key);
    let revised = rt
        .revise(
            &key,
            Change::Retransition {
                transitions: rules::transitions(&rules_text)?,
            },
            why.as_bytes(),
        )
        .await?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "retransitioned": key,
                "context": revised.context,
                "rationale": revised.rationale,
                "warnings": revised.warnings,
            })
        );
    } else {
        println!("{key} 改了转换表");
        for w in &revised.warnings {
            println!("  ! {w}");
        }
        sealed(&revised.context, &revised.rationale);
    }
    Ok(0)
}
