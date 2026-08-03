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
    let mut drifted_criteria = Vec::new();
    let mut drifted_schedule = Vec::new();
    let mut warnings = Vec::new();

    let mut scheduled = 0;
    for decl in &declared.anchor {
        let key = AnchorKey::new(decl.key.clone());
        if !dry_run && rt.ensure_scheduled(&key).await? {
            scheduled += 1;
        }
        if existing.contains(&key) {
            let view = rt.read(&key).await?;
            let drift = differs(&view.anchor, decl)?;
            if drift.criteria {
                drifted_criteria.push(decl.key.clone());
            }
            if drift.schedule {
                drifted_schedule.push(decl.key.clone());
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
                "opened": opened,
                "criteria_drifted": drifted_criteria,
                "schedule_drifted": drifted_schedule,
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
    if !drifted_criteria.is_empty() {
        println!(
            "\n{} anchors have declarations that differ from their current criteria:",
            drifted_criteria.len()
        );
        for k in &drifted_criteria {
            println!("  != {k}");
        }
        println!(
            "\nChanging a probe or transition table is a criteria revision, not a refactor; sync will not do it for you.\n\
             Decide whether to accept it, then use revise so it leaves a sealed record."
        );
    }
    if !drifted_schedule.is_empty() {
        println!(
            "\n{} anchors declare a retain/cadence that is not the one they are running:",
            drifted_schedule.len()
        );
        for k in &drifted_schedule {
            println!("  ~= {k}");
        }
        // Do not send them to `revise`: Change has no variant for either field,
        // so there is currently no way to act on this at all. Say so plainly
        // rather than naming a verb that will turn them away.
        println!(
            "\nThese two say how an anchor is run, not what it judges. They are fixed when the\n\
             anchor is opened and there is no revision channel for them yet — revise cannot move\n\
             them either. The declaration on file is not in force; the anchor keeps what it was\n\
             opened with. To actually change one today, the anchor has to be superseded."
        );
    }
    Ok(0)
}

/// How the declaration on file differs from what the anchor is actually
/// running. The two halves are not the same kind of problem and must not be
/// reported as one: only the first has a way to act on it.
struct Drift {
    /// Probe, transition table, terminal set. Sealed criteria — `revise`
    /// moves these and leaves a record when it does.
    criteria: bool,
    /// `retain` and `cadence_secs`: how this anchor is *run*, not what it
    /// judges. They are fixed at open time and have **no revision channel at
    /// all** — `Change` has no variant for either — so pointing the reader at
    /// `revise` here would send them at a wall with no door in it.
    schedule: bool,
}

fn differs(anchor: &Anchor, decl: &AnchorDecl) -> Result<Drift, CliError> {
    Ok(Drift {
        criteria: anchor.probe != decl.to_probe()?
            || anchor.transitions != decl.to_transitions()?
            || anchor.terminal != rules::terminal(&decl.terminal),
        schedule: anchor.retain != decl.retain() || anchor.cadence_secs != decl.cadence_secs,
    })
}
