use serde_json::{Map, Value};

use crate::error::CliError;
use crate::probes::Catalog;

const WHOLE: [&str; 2] = coding_extract::WHOLE;
const PART: &[&str] = &["name", "heading"];
const PREFERENCE: &[&str] = &["contract", "fingerprint", "roster"];

#[derive(Debug, PartialEq, Eq)]
pub struct Routed {
    pub probe: String,
    pub shape: String,
    pub position: Value,
}

pub fn probe_for(coord: &str, catalog: &Catalog) -> Result<String, CliError> {
    let file = coord.split_once('#').map(|(f, _)| f).unwrap_or(coord);
    let ext = file.rsplit_once('.').map(|(_, e)| e).unwrap_or_default();
    catalog.for_extension(ext).ok_or_else(|| {
        CliError(format!(
            "`{coord}` needs a probe that reads `.{ext}`, and none does"
        ))
    })
}

fn slot(at: &[String], want: &[&'static str]) -> Option<&'static str> {
    want.iter().copied().find(|w| at.iter().any(|k| k == w))
}

fn position_for(coord: &str, at: &[String], probe: &str) -> Result<Value, CliError> {
    let (whole, part) = match coord.split_once('#') {
        Some((w, p)) => (w, Some(p)),
        None => (coord, None),
    };

    let mut out = Map::new();
    let whole_key = slot(at, &WHOLE).ok_or_else(|| {
        CliError(format!(
            "probe `{probe}` names no whole to point at; its coordinate is {}",
            at.join(" · ")
        ))
    })?;
    out.insert(whole_key.to_owned(), Value::String(whole.to_owned()));

    if let Some(part) = part {
        let part_key = slot(at, PART).ok_or_else(|| {
            CliError(format!(
                "`{coord}` names a part, but probe `{probe}` has no coordinate item for one; \
                 it has {}",
                at.join(" · ")
            ))
        })?;
        out.insert(part_key.to_owned(), Value::String(part.to_owned()));
    }
    Ok(Value::Object(out))
}

fn fits(shape: &str, obs: &crate::probes::Obs) -> Result<bool, CliError> {
    let transitions =
        crate::rules::transitions(&crate::shapes::rules_of(crate::shapes::get(shape)?))?;
    Ok(crate::contract::unmet(&crate::contract::reads_of(&transitions)?, obs).is_empty())
}

fn shape_for(coord: &str, obs: &crate::probes::Obs) -> Result<String, CliError> {
    if !coord.contains('#') {
        return Ok("roster".to_owned());
    }
    for name in PREFERENCE {
        if fits(name, obs)? {
            return Ok((*name).to_owned());
        }
    }
    Err(CliError(format!(
        "no shape this build ships can be fed by the probe that reads `{coord}`"
    )))
}

pub fn route(coord: &str, shape: Option<&str>, catalog: &Catalog) -> Result<Routed, CliError> {
    let probe = probe_for(coord, catalog)?;
    let obs = catalog.obs_of(&probe)?;
    let position = position_for(coord, &obs.at, &probe)?;
    let shape = match shape {
        Some(s) => {
            crate::shapes::get(s)?;
            s.to_owned()
        }
        None => shape_for(coord, &obs)?,
    };
    Ok(Routed {
        probe,
        shape,
        position,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECIPES: &str = r#"
[probe.prose-map]
stage = { probe = "x" }
entrypoint = "probe"
sources = ["x"]
handles = ["md"]
obs = { schema = "gmr.probe-coord.v1", at = ["file", "heading", "fingerprint"], facts = ["line", "lines"] }

[probe.blob-map]
stage = { probe = "x" }
entrypoint = "probe"
sources = ["x"]
handles = ["bin"]
obs = { schema = "gmr.probe-coord.v1", at = ["path", "fingerprint"], facts = ["bytes"] }
"#;

    fn catalog() -> (tempfile::TempDir, Catalog) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".anchor")).unwrap();
        std::fs::write(dir.path().join(".anchor/probes.toml"), RECIPES).unwrap();
        std::fs::write(dir.path().join("x"), "body").unwrap();
        let c = Catalog::load(dir.path()).unwrap();
        (dir, c)
    }

    fn routed(coord: &str) -> Routed {
        let (_d, c) = catalog();
        route(coord, None, &c).unwrap()
    }

    #[test]
    fn a_named_part_of_code_watches_its_contract() {
        assert_eq!(
            routed("src/auth.rs#create_session"),
            Routed {
                probe: "ast-map".to_owned(),
                shape: "contract".to_owned(),
                position: serde_json::json!({ "file": "src/auth.rs", "name": "create_session" }),
            }
        );
    }

    #[test]
    fn a_whole_file_watches_its_roster() {
        assert_eq!(
            routed("src/auth.rs"),
            Routed {
                probe: "ast-map".to_owned(),
                shape: "roster".to_owned(),
                position: serde_json::json!({ "file": "src/auth.rs" }),
            }
        );
    }

    #[test]
    fn a_part_lands_in_whichever_slot_the_probe_actually_has() {
        assert_eq!(
            routed("docs/design.md#Invariants"),
            Routed {
                probe: "prose-map".to_owned(),
                shape: "fingerprint".to_owned(),
                position: serde_json::json!({ "file": "docs/design.md", "heading": "Invariants" }),
            }
        );
    }

    #[test]
    fn a_probe_that_calls_its_whole_a_path_gets_a_path() {
        assert_eq!(
            routed("vendor/blob.bin").position,
            serde_json::json!({ "path": "vendor/blob.bin" })
        );
    }

    #[test]
    fn a_part_a_probe_has_no_slot_for_is_refused_by_name() {
        let at: Vec<String> = ["path", "fingerprint"].map(str::to_owned).into();
        let e = position_for("vendor/blob.bin#thing", &at, "blob-map").unwrap_err();
        assert!(e.to_string().contains("no coordinate item"), "{e}");
        assert!(e.to_string().contains("fingerprint"), "{e}");
    }

    #[test]
    fn an_extension_no_builtin_or_declared_probe_names_still_falls_to_the_derived_catchall() {
        let (_d, c) = catalog();
        let routed = route("schema/a.proto", None, &c).unwrap();
        assert_eq!(
            routed.probe, "addr-map",
            "no probe declares `.proto`, but addr-map's own eligible rule is `true` for \
             every path and it is the only addressable builtin that says so — so it is the \
             derived fallback rather than a refusal. This coordinate used to be refused; \
             see coding-extract's catchall for where the fallback is derived",
        );
        assert_eq!(
            routed.position,
            serde_json::json!({ "path": "schema/a.proto" })
        );
    }

    #[test]
    fn a_probe_that_names_an_extension_outranks_the_catchall_whoever_declared_it() {
        let (_d, c) = catalog();
        let routed = route("vendor/blob.bin", None, &c).unwrap();
        assert_eq!(
            routed.probe, "blob-map",
            "`blob-map` declares `handles = [\"bin\"]` and the catchall declares nothing in \
             particular, so the specific one answers. Asking the builtin roster first as a \
             whole put a declared probe behind a fallback that claims every extension — the \
             repository would install an instrument, get the fingerprint of the whole file \
             instead, and be told nothing"
        );
    }

    #[test]
    fn a_named_shape_overrides_the_inference() {
        let (_d, c) = catalog();
        assert_eq!(
            route("src/auth.rs#f", Some("roster"), &c).unwrap().shape,
            "roster"
        );
        assert!(route("src/auth.rs#f", Some("nope"), &c).is_err());
    }
}
