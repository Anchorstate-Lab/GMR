use std::path::Path;

use gmr::{Anchor, AnchorKey, OpenRequest, ProbeRef, Retain, Runtime, State, Transitions};
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
    /// Probe artifact version. It is earned, so rebuilding the probe changes
    /// the version and becomes a criteria revision.
    pub artifact: String,
    #[serde(default)]
    pub params: serde_json::Value,
    #[serde(default)]
    pub position: Option<serde_json::Value>,
    #[serde(default)]
    pub rules: Vec<String>,
    #[serde(default)]
    pub terminal: Vec<String>,
    /// Keep a full record when the world did not move, instead of only noting another sighting.
    #[serde(default)]
    pub retain_full: bool,
    /// This anchor's observation cadence; omit it to use the deployment default.
    #[serde(default)]
    pub cadence_secs: Option<u64>,
}

impl AnchorDecl {
    fn to_probe(&self) -> Result<ProbeRef, CliError> {
        rules::probe(&self.artifact, &self.params.to_string())
    }

    fn to_transitions(&self) -> Result<Transitions, CliError> {
        rules::transitions(&self.rules)
    }

    fn retain(&self) -> Retain {
        if self.retain_full {
            Retain::Full
        } else {
            Retain::Tick
        }
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
        .map_err(|e| CliError(format!("cannot read `{file}`: {e}")))?;
    let declared: Declared = toml::from_str(&text)?;

    let existing = rt.anchors().await?;
    let mut opened = Vec::new();
    let mut drifted_declaration = Vec::new();
    let mut warnings = Vec::new();

    let mut scheduled = 0;
    for decl in &declared.anchor {
        let key = AnchorKey::new(decl.key.clone());
        if !dry_run && rt.ensure_scheduled(&key).await? {
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
                probe: decl.to_probe()?,
                transitions: decl.to_transitions()?,
                terminal: rules::terminal(&decl.terminal),
                initial: decl.initial(),
                retain: decl.retain(),
                cadence_secs: decl.cadence_secs,
                supersedes: None,
            })
            .await?;
        for w in result.warnings {
            warnings.push(format!("{key}: {w}"));
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
        "{} anchors{}",
        opened.len(),
        if dry_run {
            " would be opened (--dry-run)"
        } else {
            " opened"
        }
    );
    for w in &warnings {
        println!("  ! {w}");
    }
    if !drifted_declaration.is_empty() {
        println!(
            "\n{} anchors have declarations that differ from their current criteria:",
            drifted_declaration.len()
        );
        for k in &drifted_declaration {
            println!("  != {k}");
        }
        println!(
            "\nChanging a probe or transition table is a criteria revision, not a refactor; sync will not do it for you.\n\
             Decide whether to accept it, then use revise so it leaves a sealed record."
        );
    }
    Ok(0)
}

fn differs(anchor: &Anchor, decl: &AnchorDecl) -> Result<bool, CliError> {
    Ok(anchor.probe != decl.to_probe()?
        || anchor.transitions != decl.to_transitions()?
        || anchor.terminal != rules::terminal(&decl.terminal)
        || anchor.retain != decl.retain()
        || anchor.cadence_secs != decl.cadence_secs)
}
