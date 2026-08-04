use std::path::Path;

use gmr::{
    Anchor, AnchorKey, OpenRequest, ProbeRef, Retain, RunSettings, Runtime, State, Transitions,
};
use serde::Deserialize;

use crate::error::CliError;
use crate::probes::Recipes;
use crate::rules;

pub struct Context {
    pub root: std::path::PathBuf,
    pub recipes: Recipes,
}

#[derive(Debug, Deserialize)]
pub struct Declared {
    #[serde(default)]
    pub anchor: Vec<AnchorDecl>,
}

#[derive(Debug, Deserialize)]
pub struct AnchorDecl {
    pub key: String,
    /// A recipe name, portable across platforms. An artifact hash is not: it
    /// carries the platform and the built binary's hash.
    #[serde(default)]
    pub probe: Option<String>,
    /// Escape hatch pinning one artifact version. Exclusive with `probe`.
    #[serde(default)]
    pub artifact: Option<String>,
    #[serde(default)]
    pub params: serde_json::Value,
    #[serde(default)]
    pub position: Option<serde_json::Value>,
    /// A named transition preset, exclusive with `rules`. Expanded into literal
    /// rules at sync time, so the declaration hash still covers full criteria.
    #[serde(default)]
    pub shape: Option<String>,
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
    fn to_probe(&self, ctx: &Context) -> Result<ProbeRef, CliError> {
        let params = self.params.to_string();
        match (&self.probe, &self.artifact) {
            (Some(_), Some(_)) | (None, None) => Err(CliError(format!(
                "{}: declare either `probe` (a recipe name) or `artifact` (a version), not both",
                self.key
            ))),
            (Some(name), None) => {
                let recipe = ctx.recipes.get(name)?;
                let version = recipe.version(name, &ctx.root)?;
                rules::probe(version.as_str(), &params)
            }
            (None, Some(artifact)) => rules::probe(artifact, &params),
        }
    }

    /// A pinned artifact has no recipe and therefore no vocabulary to check.
    fn check_contract(&self, ctx: &Context) -> Result<(), CliError> {
        let (Some(shape), Some(name)) = (&self.shape, &self.probe) else {
            return Ok(());
        };
        let missing = crate::shapes::unmet(crate::shapes::get(shape)?, &ctx.recipes.get(name)?.obs);
        match missing.is_empty() {
            true => Ok(()),
            false => Err(CliError(format!(
                "{}: shape `{shape}` reads {}, which probe `{name}` does not emit",
                self.key,
                missing.join(" · ")
            ))),
        }
    }

    fn to_transitions(&self) -> Result<Transitions, CliError> {
        match (&self.shape, self.rules.is_empty()) {
            (Some(_), false) => Err(CliError(format!(
                "{}: declare either `shape` or `rules`, not both",
                self.key
            ))),
            (Some(name), true) => rules::transitions(&crate::shapes::rules_of(
                crate::shapes::get(name).map_err(|e| CliError(format!("{}: {e}", self.key)))?,
            )),
            (None, _) => rules::transitions(&self.rules),
        }
    }

    fn settings(&self) -> RunSettings {
        RunSettings {
            retain: if self.retain_full {
                Retain::Full
            } else {
                Retain::Tick
            },
            cadence_secs: self.cadence_secs,
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
    let ctx = Context {
        root: root.to_path_buf(),
        recipes: Recipes::load(root)?,
    };

    let existing = rt.anchors().await?;
    let mut opened = Vec::new();
    let mut drifted_criteria = Vec::new();
    let mut resettled = Vec::new();
    let mut warnings = Vec::new();

    let mut scheduled = 0;
    for decl in &declared.anchor {
        let key = AnchorKey::new(decl.key.clone());
        if !dry_run && rt.ensure_scheduled(&key).await? {
            scheduled += 1;
        }
        if existing.contains(&key) {
            let view = rt.read(&key).await?;
            if differs(&view.anchor, decl, &ctx)? {
                drifted_criteria.push(decl.key.clone());
            }
            // Retain and cadence are not criteria, so sync just applies them.
            if rt.settings_for(&key).await? != decl.settings() {
                if !dry_run {
                    rt.set_settings(&key, &decl.settings()).await?;
                }
                resettled.push(decl.key.clone());
            }
            continue;
        }
        if dry_run {
            opened.push(decl.key.clone());
            continue;
        }
        decl.check_contract(&ctx)?;
        let result = rt
            .open(OpenRequest {
                key: key.clone(),
                probe: decl.to_probe(&ctx)?,
                transitions: decl.to_transitions()?,
                terminal: rules::terminal(&decl.terminal),
                initial: decl.initial(),
                settings: decl.settings(),
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
                "resettled": resettled,
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
    if !resettled.is_empty() {
        println!(
            "\n{} anchors {} a new retain/cadence from the declaration:",
            resettled.len(),
            if dry_run { "would take" } else { "took" }
        );
        for k in &resettled {
            println!("  ~= {k}");
        }
    }
    Ok(0)
}

fn differs(anchor: &Anchor, decl: &AnchorDecl, ctx: &Context) -> Result<bool, CliError> {
    Ok(anchor.probe != decl.to_probe(ctx)?
        || anchor.transitions != decl.to_transitions()?
        || anchor.terminal != rules::terminal(&decl.terminal))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ART: &str =
        "artifact = \"d9fe5d540d44ba9c97a351323396c3028d0281a213e21c69bb55b89da4f9ba62\"";

    const RULES: &str = r#"
rules = [
  'obs.exact == false => { position: state.position, n: 0, matches: [], status: "coordinate-missed" }',
  'not exists(state.n) => { position: state.position, n: obs.candidates, matches: obs.matches, status: "captured" }',
  'obs.candidates > state.n => { position: state.position, n: obs.candidates, matches: obs.matches, was: state.matches, status: "added" }',
  'obs.candidates < state.n => { position: state.position, n: obs.candidates, matches: obs.matches, was: state.matches, status: "removed" }',
  'changed("matches") => { position: state.position, n: obs.candidates, matches: obs.matches, was: state.matches, status: "moved" }',
]
"#;

    fn decl(body: &str) -> AnchorDecl {
        let text = format!("[[anchor]]\nkey = \"k\"\n{body}");
        toml::from_str::<Declared>(&text)
            .unwrap()
            .anchor
            .into_iter()
            .next()
            .unwrap()
    }

    /// Migration guarantee: swapping literal rules for a shape moves no criteria.
    #[test]
    fn a_shape_expands_to_the_table_it_replaces() {
        assert_eq!(
            decl(&format!("{ART}\nshape = \"roster\""))
                .to_transitions()
                .unwrap(),
            decl(&format!("{ART}{RULES}")).to_transitions().unwrap()
        );
    }

    #[test]
    fn shape_and_rules_together_are_refused() {
        let e = decl(&format!("{ART}\nshape = \"roster\"{RULES}"))
            .to_transitions()
            .unwrap_err();
        assert!(e.to_string().contains("not both"), "{e}");
    }

    #[test]
    fn an_unknown_shape_names_the_anchor_that_asked_for_it() {
        let e = decl(&format!("{ART}\nshape = \"nope\""))
            .to_transitions()
            .unwrap_err();
        assert!(e.to_string().contains('k'), "{e}");
    }

    fn ctx(toml_body: &str) -> (tempfile::TempDir, Context) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".anchor")).unwrap();
        std::fs::write(dir.path().join(".anchor/probes.toml"), toml_body).unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/probe.sh"), "echo '{}'").unwrap();
        let recipes = Recipes::load(dir.path()).unwrap();
        let root = dir.path().to_path_buf();
        (dir, Context { root, recipes })
    }

    const AST_LIKE: &str = r#"
[probe.ast-like]
stage = { probe = "src/probe.sh" }
entrypoint = "probe"
sources = ["src"]
obs = { schema = "gmr.probe-coord.v1", at = ["file", "name"], facts = ["body", "line"] }
"#;

    #[test]
    fn a_probe_name_resolves_to_a_recipe_version() {
        let (_d, c) = ctx(AST_LIKE);
        let probe = decl("probe = \"ast-like\"\nshape = \"roster\"")
            .to_probe(&c)
            .unwrap();
        assert_eq!(probe.artifact.as_str().len(), 64);
    }

    #[test]
    fn naming_both_a_probe_and_an_artifact_is_refused() {
        let (_d, c) = ctx(AST_LIKE);
        let e = decl(&format!("probe = \"ast-like\"\n{ART}"))
            .to_probe(&c)
            .unwrap_err();
        assert!(e.to_string().contains("not both"), "{e}");
    }

    /// A mismatch must be caught before opening, not after observation.
    #[test]
    fn a_shape_the_probe_cannot_feed_is_refused() {
        let (_d, c) = ctx(AST_LIKE);
        let e = decl("probe = \"ast-like\"\nshape = \"occurrence\"")
            .check_contract(&c)
            .unwrap_err();
        assert!(e.to_string().contains("facts.occurrences"), "{e}");
    }

    #[test]
    fn roster_rides_the_same_probe_happily() {
        let (_d, c) = ctx(AST_LIKE);
        decl("probe = \"ast-like\"\nshape = \"roster\"")
            .check_contract(&c)
            .unwrap();
    }
}
