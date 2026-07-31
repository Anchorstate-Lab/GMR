use std::path::Path;

use gmr::{Anchor, AnchorKey, Kind, OpenRequest, Probe, Retain, Runtime, State, Transitions};
use serde::Deserialize;

use crate::error::CliError;
use crate::rules;

#[derive(Debug, Deserialize)]
pub struct Declared {
    #[serde(default)]
    pub anchor: Vec<AnchorDecl>,
}

#[derive(Debug, Deserialize)]
pub struct AnchorDecl {
    pub key: String,
    pub probe: String,
    #[serde(default)]
    pub position: Option<serde_json::Value>,
    #[serde(default)]
    pub rules: Vec<String>,
    #[serde(default)]
    pub terminal: Vec<String>,
}

impl AnchorDecl {
    fn to_probe(&self) -> Probe {
        Probe::new(Kind::new("shell"), serde_json::json!({ "run": self.probe }))
    }

    fn to_transitions(&self) -> Result<Transitions, CliError> {
        rules::transitions(&self.rules)
    }

    fn initial(&self) -> Option<State> {
        self.position
            .clone()
            .map(|p| State::new(serde_json::json!({ "position": p })))
    }
}

pub async fn run(
    rt: &Runtime,
    root: &Path,
    file: String,
    dry_run: bool,
    json: bool,
) -> Result<i32, CliError> {
    let text = std::fs::read_to_string(root.join(&file))
        .map_err(|e| CliError(format!("读不到 `{file}`：{e}")))?;
    let declared: Declared = toml::from_str(&text)?;

    let existing = rt.anchors().await?;
    let mut opened = Vec::new();
    let mut drifted_declaration = Vec::new();
    let mut warnings = Vec::new();

    let mut scheduled = 0;
    for decl in &declared.anchor {
        let key = AnchorKey::new(decl.key.clone());
        if !dry_run && rt.schedule(&key).await? {
            scheduled += 1;
        }
        if existing.contains(&key) {
            let view = rt.read(&key).await?;
            if differs(&view.anchor, decl)? {
                drifted_declaration.push(decl.key.clone());
            }
            continue;
        }
        if dry_run {
            opened.push(decl.key.clone());
            continue;
        }
        let result = rt
            .open(OpenRequest {
                key: key.clone(),
                probe: decl.to_probe(),
                transitions: decl.to_transitions()?,
                terminal: rules::terminal(&decl.terminal),
                initial: decl.initial(),
                retain: Retain::Tick,
                cadence_secs: None,
            })
            .await?;
        for w in result.warnings {
            warnings.push(format!("{key}：{w}"));
        }
        opened.push(decl.key.clone());
    }

    if json {
        println!(
            "{}",
            serde_json::json!({
                "opened": opened, "declaration_drifted": drifted_declaration,
                "warnings": warnings, "dry_run": dry_run, "scheduled": scheduled,
            })
        );
        return Ok(0);
    }

    println!(
        "{} 个锚{}",
        opened.len(),
        if dry_run {
            " 会被开（--dry-run）"
        } else {
            " 已开"
        }
    );
    for w in &warnings {
        println!("  ! {w}");
    }
    if !drifted_declaration.is_empty() {
        println!(
            "\n{} 个锚的声明跟它当前的判据不一致：",
            drifted_declaration.len()
        );
        for k in &drifted_declaration {
            println!("  ≠ {k}");
        }
        println!(
            "\n改探针或改转换表是一次**判据修订**，不是重构 —— sync 不替你做。\n\
             想清楚要不要接受，然后走 revise，它会留下一条密封的记录。"
        );
    }
    Ok(0)
}

fn differs(anchor: &Anchor, decl: &AnchorDecl) -> Result<bool, CliError> {
    Ok(anchor.probe != decl.to_probe()
        || anchor.transitions != decl.to_transitions()?
        || anchor.terminal != rules::terminal(&decl.terminal))
}
