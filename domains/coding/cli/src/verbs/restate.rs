use gmr::{AnchorKey, Change, Runtime, State};

use crate::error::CliError;
use crate::verbs::sealed;

pub async fn run(
    rt: &Runtime,
    key: String,
    state: String,
    why: String,
    json: bool,
) -> Result<i32, CliError> {
    let key = AnchorKey::new(key);
    let value: serde_json::Value =
        serde_json::from_str(&state).map_err(|e| CliError(format!("new state is not valid JSON: {e}")))?;
    if !value.is_object() {
        return Err(CliError("new state must be an object".into()));
    }

    let before = rt.read(&key).await?;
    let revised = rt
        .revise(
            &key,
            Change::Restate {
                state: State::new(value.clone()),
            },
            why.as_bytes(),
        )
        .await?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "restated": key, "from": before.state, "to": value,
                "context": revised.context, "rationale": revised.rationale,
            })
        );
    } else {
        println!("{key} state changed");
        println!("  from  {}", before.state.as_value());
        println!("  to    {value}");
        sealed(&revised.context, &revised.rationale);
    }
    Ok(0)
}
