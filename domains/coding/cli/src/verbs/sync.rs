use std::path::Path;

use gmr::{
    Anchor, AnchorKey, OpenRequest, ProbeRef, Ref, Retain, RunSettings, Runtime, State, Transitions,
};
use serde::Deserialize;

use crate::error::CliError;
use crate::probes::Catalog;
use crate::rules;

pub const DEFAULT_FILE: &str = ".anchor/anchors.toml";

pub struct Context {
    pub catalog: Catalog,
}

#[derive(Debug, Default, Deserialize)]
pub struct Declared {
    #[serde(default)]
    pub anchor: Vec<AnchorDecl>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnchorDecl {
    pub key: String,
    pub probe: String,
    #[serde(default)]
    pub params: serde_json::Value,
    #[serde(default)]
    pub position: Option<serde_json::Value>,
    #[serde(default)]
    pub shape: Option<String>,
    #[serde(default)]
    pub rules: Vec<String>,
    #[serde(default)]
    pub terminal: Vec<String>,
    #[serde(default)]
    pub retain_full: bool,
    #[serde(default)]
    pub cadence_secs: Option<u64>,
}

impl AnchorDecl {
    pub fn to_probe(&self, ctx: &Context) -> Result<ProbeRef, CliError> {
        rules::probe(
            ctx.catalog.kind_of(&self.probe),
            &self.probe,
            &self.params.to_string(),
        )
    }

    fn check_contract(&self, ctx: &Context) -> Result<(), CliError> {
        let reads = crate::contract::reads_of(&self.to_transitions()?)
            .map_err(|e| CliError(format!("{}: {e}", self.key)))?;
        let missing = crate::contract::unmet(&reads, &ctx.catalog.obs_of(&self.probe)?);
        match missing.is_empty() {
            true => Ok(()),
            false => Err(CliError(format!(
                "{}: rules read {}, which probe `{}` does not emit",
                self.key,
                missing.join(" · "),
                self.probe
            ))),
        }
    }

    pub fn to_transitions(&self) -> Result<Transitions, CliError> {
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

pub fn read_declared(root: &Path, file: &str) -> Result<Declared, CliError> {
    let path = root.join(file);
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(toml::from_str(&text)?),
        Err(_) if !path.exists() && file == DEFAULT_FILE => Ok(Declared::default()),
        Err(e) => Err(CliError(format!("cannot read `{file}`: {e}"))),
    }
}

pub fn merged<'a>(
    declared: &'a Declared,
    notes: &'a [crate::memories::Note],
) -> Vec<&'a AnchorDecl> {
    let from_notes = notes
        .iter()
        .flat_map(|n| &n.wants)
        .filter_map(|w| match w {
            crate::memories::Want::Declared(d) => Some(d.as_ref()),
            crate::memories::Want::Existing(_) => None,
        })
        .filter(|d| !declared.anchor.iter().any(|t| t.key == d.key));

    let mut seen: Vec<&str> = Vec::new();
    let mut out = Vec::new();
    for decl in declared.anchor.iter().chain(from_notes) {
        if seen.contains(&decl.key.as_str()) {
            continue;
        }
        seen.push(&decl.key);
        out.push(decl);
    }
    out
}

pub async fn run(
    rt: &Runtime,
    root: &Path,
    file: String,
    dry_run: bool,
    json: bool,
) -> Result<i32, CliError> {
    let declared = read_declared(root, &file)?;
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

    let mut scheduled = 0;
    for decl in merged(&declared, &notes) {
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
            if let (Some(was), Ok(now)) = (&view.derivation, rt.instrument(&view.anchor.probe))
                && was.version != now.version
            {
                swapped.push(decl.key.clone());
            }
            if rt.settings_for(&key).await? != decl.settings() {
                if !dry_run {
                    rt.set_settings(&key, &decl.settings()).await?;
                }
                resettled.push(decl.key.clone());
            }
            continue;
        }
        decl.check_contract(&ctx)?;
        if dry_run {
            opened.push(decl.key.clone());
            continue;
        }
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

pub fn differs(
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
  'obs.exact == false => { position: state.position, n: 0, status: "coordinate-missed" }',
  'not exists(state.n) => { position: state.position, n: obs.candidates, status: "counted" }',
  'obs.candidates != state.n => { position: state.position, n: obs.candidates, status: "recounted" }',
  'true => { position: state.position, n: state.n, status: "settled" }',
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

    #[test]
    fn hand_written_rules_and_a_named_shape_both_become_transitions() {
        assert!(
            decl(&format!("{ART}\nshape = \"roster\""))
                .to_transitions()
                .is_ok()
        );
        assert!(decl(&format!("{ART}{RULES}")).to_transitions().is_ok());
        assert_ne!(
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

    #[test]
    fn a_shape_the_probe_cannot_feed_is_refused() {
        let (_d, c) = ctx(AST_LIKE);
        let e = decl("probe = \"ast-like\"\nshape = \"fingerprint\"")
            .check_contract(&c)
            .unwrap_err();
        assert!(e.to_string().contains("at.fingerprint"), "{e}");
    }

    #[test]
    fn roster_rides_the_same_probe_happily() {
        let (_d, c) = ctx(AST_LIKE);
        decl("probe = \"ast-like\"\nshape = \"roster\"")
            .check_contract(&c)
            .unwrap();
    }

    #[test]
    fn hand_written_rules_get_the_same_check_a_shape_gets() {
        let (_d, c) = ctx(AST_LIKE);
        let e = decl(
            "probe = \"ast-like\"\nrules = ['obs.facts.occurrences > 0 => { status: \"x\" }']",
        )
        .check_contract(&c)
        .unwrap_err();
        assert!(e.to_string().contains("facts.occurrences"), "{e}");
    }

    #[test]
    fn a_syntax_error_in_a_rule_is_caught_at_sync_time_not_observe_time() {
        let (_d, c) = ctx(AST_LIKE);
        let e = decl("probe = \"ast-like\"\nrules = ['obs.facts. => { status: \"x\" }']")
            .check_contract(&c)
            .unwrap_err();
        assert!(e.to_string().starts_with("k:"), "{e}");
    }
}
