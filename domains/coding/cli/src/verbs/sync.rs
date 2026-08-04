use std::path::Path;

use gmr::{
    Anchor, AnchorKey, OpenRequest, ProbeRef, Ref, Retain, RunSettings, Runtime, State, Transitions,
};
use serde::Deserialize;

use crate::error::CliError;
use crate::probes::Recipes;
use crate::rules;

/// init writes no anchors, so a repo whose anchors all come from notes has no
/// such file. Missing at the default path means "none declared here"; missing
/// at a path the user named is a typo worth stopping for.
pub const DEFAULT_FILE: &str = ".anchor/anchors.toml";

pub struct Context {
    pub root: std::path::PathBuf,
    pub recipes: Recipes,
}

#[derive(Debug, Default, Deserialize)]
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
                let version = ctx.recipes.version_of(name, &ctx.root)?;
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
    let path = root.join(&file);
    let declared: Declared = match std::fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text)?,
        Err(_) if !path.exists() && file == DEFAULT_FILE => Declared::default(),
        Err(e) => return Err(CliError(format!("cannot read `{file}`: {e}"))),
    };
    let ctx = Context {
        root: root.to_path_buf(),
        recipes: Recipes::load(root)?,
    };

    let notes = crate::memories::scan(root, &ctx.recipes)?;

    let existing = rt.anchors().await?;
    let mut opened = Vec::new();
    let mut drifted_criteria = Vec::new();
    let mut resettled = Vec::new();
    let mut warnings = Vec::new();

    // A note declaring an anchor is the same declaration as one in the toml;
    // the toml wins a duplicate key because it is the more explicit statement.
    let from_notes: Vec<&AnchorDecl> = notes
        .iter()
        .flat_map(|n| &n.wants)
        .filter_map(|w| match w {
            crate::memories::Want::Declared(d) => Some(d.as_ref()),
            crate::memories::Want::Existing(_) => None,
        })
        .filter(|d| !declared.anchor.iter().any(|t| t.key == d.key))
        .collect();

    let mut scheduled = 0;
    let mut seen: Vec<String> = Vec::new();
    for decl in declared.anchor.iter().chain(from_notes.into_iter()) {
        if seen.contains(&decl.key) {
            continue;
        }
        seen.push(decl.key.clone());
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

    let (bound, renamed) = align_bindings(rt, &notes, dry_run).await?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "opened": opened,
                "criteria_drifted": drifted_criteria,
                "resettled": resettled,
                "bound": bound, "renamed": renamed,
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
    if !bound.is_empty() {
        println!(
            "\n{} notes {} their anchors:",
            bound.len(),
            if dry_run {
                "would be bound to"
            } else {
                "bound to"
            }
        );
        for b in &bound {
            println!("  + {b}");
        }
    }
    if !renamed.is_empty() {
        println!(
            "\n{} notes dropped a key and gained an unseen one. That is either a\n\
             rename or a mistake, and sync will not guess which:",
            renamed.len()
        );
        for r in &renamed {
            println!("  ? {r}");
        }
        println!("\nClose the old anchor with a reason, or put the old key back.");
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

/// Binding is append-only, so this writes only when the relation actually
/// changed; otherwise every sync would add a row saying the same thing.
async fn align_bindings(
    rt: &Runtime,
    notes: &[crate::memories::Note],
    dry_run: bool,
) -> Result<(Vec<String>, Vec<String>), CliError> {
    let mut bound = Vec::new();
    let mut renamed = Vec::new();

    for note in notes {
        let reference = Ref::new("git", note.path.clone());
        let mut want: Vec<AnchorKey> = note
            .wants
            .iter()
            .map(|w| AnchorKey::new(w.key().to_owned()))
            .collect();
        want.sort();
        want.dedup();

        let current = rt.memory().binding_of(&reference).await?;
        if let Some(record) = &current {
            let mut had = record.binding.anchors.clone();
            had.sort();
            let dropped: Vec<_> = had.iter().filter(|k| !want.contains(k)).collect();
            let added: Vec<_> = want.iter().filter(|k| !had.contains(k)).collect();
            if !dropped.is_empty() && !added.is_empty() {
                renamed.push(format!(
                    "{}: dropped {}, gained {}",
                    note.path,
                    dropped
                        .iter()
                        .map(|k| k.to_string())
                        .collect::<Vec<_>>()
                        .join(", "),
                    added
                        .iter()
                        .map(|k| k.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
                continue;
            }
        }

        let version = rt
            .memory()
            .current_version(&reference)
            .await?
            .ok_or_else(|| {
                CliError(format!("no content provider could version `{}`", note.path))
            })?;
        let settled = current.is_some_and(|r| {
            let mut had = r.binding.anchors.clone();
            had.sort();
            had == want && r.bound_version == version
        });
        if settled {
            continue;
        }
        if !dry_run {
            rt.bind(reference, want, version).await?;
        }
        bound.push(note.path.clone());
    }
    Ok((bound, renamed))
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
