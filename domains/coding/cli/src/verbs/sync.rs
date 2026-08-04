use std::path::Path;

use gmr::{
    Anchor, AnchorKey, OpenRequest, ProbeRef, Ref, Retain, RunSettings, Runtime, State, Transitions,
};
use serde::Deserialize;

use crate::error::CliError;
use crate::probes::Catalog;
use crate::rules;

/// init writes no anchors, so a repo whose anchors all come from notes has no
/// such file. Missing at the default path means "none declared here"; missing
/// at a path the user named is a typo worth stopping for.
pub const DEFAULT_FILE: &str = ".anchor/anchors.toml";

pub struct Context {
    pub catalog: Catalog,
}

#[derive(Debug, Default, Deserialize)]
pub struct Declared {
    #[serde(default)]
    pub anchor: Vec<AnchorDecl>,
}

#[derive(Debug, Deserialize)]
pub struct AnchorDecl {
    pub key: String,
    /// A name. A declaration has to survive an engine upgrade unchanged.
    pub probe: String,
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
        rules::probe(
            ctx.catalog.kind_of(&self.probe),
            &self.probe,
            &self.params.to_string(),
        )
    }

    fn check_contract(&self, ctx: &Context) -> Result<(), CliError> {
        let Some(shape) = &self.shape else {
            return Ok(());
        };
        let missing = crate::shapes::unmet(
            crate::shapes::get(shape)?,
            &ctx.catalog.obs_of(&self.probe)?,
        );
        match missing.is_empty() {
            true => Ok(()),
            false => Err(CliError(format!(
                "{}: shape `{shape}` reads {}, which probe `{}` does not emit",
                self.key,
                self.probe,
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
        catalog: Catalog::load(root)?,
    };

    let notes = crate::memories::scan(root, &ctx.catalog)?;

    let existing = rt.anchors().await?;
    let mut opened = Vec::new();
    let mut drifted_criteria = Vec::new();
    let mut swapped = Vec::new();
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
            let facets = differs(&view.anchor, decl, &ctx)?;
            if !facets.is_empty() {
                drifted_criteria.push(format!("{} ({})", decl.key, facets.join(" · ")));
            }
            // The declaration can be identical and the instrument still be a
            // different one. That is not drift; it is a baseline taken with
            // another ruler, and only a person can say whether it still counts.
            if let (Some(was), Ok(now)) = (&view.derivation, rt.instrument(&view.anchor.probe))
                && was.version != now.version
            {
                swapped.push(decl.key.clone());
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
                "instrument_swapped": swapped,
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
    if !swapped.is_empty() {
        println!(
            "\n{} anchors last read with an instrument this build no longer has:",
            swapped.len()
        );
        for k in &swapped {
            println!("  ~= {k}");
        }
        println!(
            "\nThe declarations are unchanged; what moved is the rule behind the name.\n\
             Whatever those baselines are compared against next was measured differently,\n\
             and only you can say whether that still counts as the same reading.\n\
             \n    gmr rebase --all --why '...'\n\
             \nObserve will keep running either way — it just cannot tell you which of\n\
             the two things moved."
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

/// Naming the facet matters: "the probe was renamed" and "the transition table
/// was rewritten" are different judgments, and the sealed reason should say
/// which one it is.
fn differs(
    anchor: &Anchor,
    decl: &AnchorDecl,
    ctx: &Context,
) -> Result<Vec<&'static str>, CliError> {
    let mut facets = Vec::new();
    if anchor.probe != decl.to_probe(ctx)? {
        facets.push("probe");
    }
    if anchor.transitions != decl.to_transitions()? {
        facets.push("rules");
    }
    if anchor.terminal != rules::terminal(&decl.terminal) {
        facets.push("terminal");
    }
    Ok(facets)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ART: &str = "probe = \"ast-map\"";

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
        let catalog = Catalog::load(dir.path()).unwrap();
        (dir, Context { catalog })
    }

    const AST_LIKE: &str = r#"
[probe.ast-like]
stage = { probe = "src/probe.sh" }
entrypoint = "probe"
sources = ["src"]
obs = { schema = "gmr.probe-coord.v1", at = ["file", "name"], facts = ["body", "line"] }
"#;

    #[test]
    fn the_declaration_carries_the_name_verbatim() {
        let (_d, c) = ctx(AST_LIKE);
        let probe = decl("probe = \"ast-like\"\nshape = \"roster\"")
            .to_probe(&c)
            .unwrap();
        assert_eq!(probe.name.as_str(), "ast-like");
    }

    #[test]
    fn a_version_where_a_name_belongs_is_refused() {
        let (_d, c) = ctx(AST_LIKE);
        let e = decl(&format!("probe = \"{}\"", "d9".repeat(32)))
            .to_probe(&c)
            .unwrap_err();
        assert!(e.to_string().contains("probe name"), "{e}");
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
