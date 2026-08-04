use std::path::Path;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::error::CliError;
use crate::probes::Recipes;
use crate::verbs::sync::AnchorDecl;

pub const NOTES_DIR: &str = "memories";

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

impl OneOrMany {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::One(s) => vec![s],
            Self::Many(v) => v,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Entry {
    /// A bare key names an anchor that must already exist.
    Existing(String),
    Declared(Box<Spec>),
}

#[derive(Debug, Deserialize)]
struct Spec {
    key: String,
    probe: Option<String>,
    #[serde(default)]
    params: Option<Value>,
    #[serde(default)]
    position: Option<Value>,
    #[serde(default)]
    shape: Option<String>,
    #[serde(default)]
    rules: Vec<String>,
    #[serde(default)]
    terminal: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct Frontmatter {
    #[serde(default)]
    about: Option<OneOrMany>,
    #[serde(default)]
    anchors: Vec<Entry>,
}

/// What a note says it is about.
#[derive(Debug)]
pub enum Want {
    Existing(String),
    Declared(Box<AnchorDecl>),
}

impl Want {
    pub fn key(&self) -> &str {
        match self {
            Self::Existing(k) => k,
            Self::Declared(d) => &d.key,
        }
    }
}

#[derive(Debug)]
pub struct Note {
    pub path: String,
    pub wants: Vec<Want>,
}

/// `src/auth.ts#createSession` -> the position a coord probe understands.
fn position_of(about: &str) -> Value {
    match about.split_once('#') {
        Some((file, name)) => json!({ "file": file, "name": name }),
        None => json!({ "file": about }),
    }
}

fn probe_for(about: &str, recipes: &Recipes) -> Result<String, CliError> {
    let file = about.split_once('#').map(|(f, _)| f).unwrap_or(about);
    let ext = file.rsplit_once('.').map(|(_, e)| e).unwrap_or_default();
    recipes
        .for_extension(ext)
        .map(str::to_owned)
        .ok_or_else(|| {
            CliError(format!(
                "`about: {about}` needs a probe that reads `.{ext}`, and no recipe declares one"
            ))
        })
}

/// The minimal form: the key is the coordinate itself, so nobody has to invent
/// a permanent identity on their first note.
fn from_about(about: &str, recipes: &Recipes) -> Result<AnchorDecl, CliError> {
    Ok(AnchorDecl {
        key: about.to_owned(),
        probe: Some(probe_for(about, recipes)?),
        artifact: None,
        params: json!({ "root": "." }),
        position: Some(position_of(about)),
        shape: Some("roster".to_owned()),
        rules: Vec::new(),
        terminal: Vec::new(),
        retain_full: false,
        cadence_secs: None,
    })
}

fn from_spec(spec: Spec) -> AnchorDecl {
    AnchorDecl {
        key: spec.key,
        probe: spec.probe,
        artifact: None,
        params: spec.params.unwrap_or_else(|| json!({ "root": "." })),
        position: spec.position,
        shape: spec.shape,
        rules: spec.rules,
        terminal: spec.terminal,
        retain_full: false,
        cadence_secs: None,
    }
}

fn frontmatter_of(text: &str) -> Result<Option<Frontmatter>, CliError> {
    let rest = match text.strip_prefix("---\n") {
        Some(r) => r,
        None => return Ok(None),
    };
    let Some(end) = rest.find("\n---") else {
        return Err(CliError("frontmatter is never closed by `---`".into()));
    };
    let body = &rest[..end];
    if body.trim().is_empty() {
        return Ok(None);
    }
    serde_yaml_ng::from_str(body)
        .map(Some)
        .map_err(|e| CliError(format!("frontmatter is not valid YAML: {e}")))
}

fn note_of(root: &Path, rel: &str, recipes: &Recipes) -> Result<Option<Note>, CliError> {
    let text = std::fs::read_to_string(root.join(rel))
        .map_err(|e| CliError(format!("cannot read `{rel}`: {e}")))?;
    let Some(fm) = frontmatter_of(&text).map_err(|e| CliError(format!("{rel}: {e}")))? else {
        return Ok(None);
    };

    let mut wants = Vec::new();
    for about in fm.about.map(OneOrMany::into_vec).unwrap_or_default() {
        let decl = from_about(&about, recipes).map_err(|e| CliError(format!("{rel}: {e}")))?;
        wants.push(Want::Declared(Box::new(decl)));
    }
    for entry in fm.anchors {
        wants.push(match entry {
            Entry::Existing(key) => Want::Existing(key),
            Entry::Declared(spec) => Want::Declared(Box::new(from_spec(*spec))),
        });
    }

    match wants.is_empty() {
        true => Ok(None),
        false => Ok(Some(Note {
            path: rel.to_owned(),
            wants,
        })),
    }
}

pub fn scan(root: &Path, recipes: &Recipes) -> Result<Vec<Note>, CliError> {
    let mut rels = Vec::new();
    walk(root, &root.join(NOTES_DIR), &mut rels)?;
    rels.sort();

    let mut notes = Vec::new();
    for rel in rels {
        if let Some(note) = note_of(root, &rel, recipes)? {
            notes.push(note);
        }
    }
    Ok(notes)
}

fn walk(root: &Path, at: &Path, out: &mut Vec<String>) -> Result<(), CliError> {
    let Ok(entries) = std::fs::read_dir(at) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, out)?;
        } else if path.extension().is_some_and(|e| e == "md") {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECIPES: &str = r#"
[probe.ast-map]
stage = { probe = "x" }
entrypoint = "probe"
sources = ["x"]
handles = ["rs", "ts", "tsx", "js", "py", "go"]
obs = { schema = "gmr.probe-coord.v1", at = ["file", "name"], facts = ["body", "line"] }
"#;

    fn world(notes: &[(&str, &str)]) -> (tempfile::TempDir, Recipes) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".anchor")).unwrap();
        std::fs::write(dir.path().join(".anchor/probes.toml"), RECIPES).unwrap();
        std::fs::write(dir.path().join("x"), "body").unwrap();
        for (path, body) in notes {
            let p = dir.path().join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, body).unwrap();
        }
        let recipes = Recipes::load(dir.path()).unwrap();
        (dir, recipes)
    }

    #[test]
    fn one_line_of_frontmatter_becomes_a_whole_anchor() {
        let (d, r) = world(&[(
            "memories/auth.md",
            "---\nabout: src/auth.ts#createSession\n---\n\n# note\n",
        )]);
        let notes = scan(d.path(), &r).unwrap();
        assert_eq!(notes.len(), 1);

        let Want::Declared(decl) = &notes[0].wants[0] else {
            panic!("expected a declared anchor");
        };
        assert_eq!(decl.key, "src/auth.ts#createSession");
        assert_eq!(decl.probe.as_deref(), Some("ast-map"));
        assert_eq!(decl.shape.as_deref(), Some("roster"));
        assert_eq!(
            decl.position,
            Some(json!({"file": "src/auth.ts", "name": "createSession"}))
        );
    }

    #[test]
    fn a_bare_key_binds_without_declaring_anything() {
        let (d, r) = world(&[(
            "memories/x.md",
            "---\nanchors:\n  - surface::gmr-core\n  - modules::gmr-core\n---\n",
        )]);
        let notes = scan(d.path(), &r).unwrap();
        let keys: Vec<_> = notes[0].wants.iter().map(Want::key).collect();
        assert_eq!(keys, vec!["surface::gmr-core", "modules::gmr-core"]);
        assert!(matches!(notes[0].wants[0], Want::Existing(_)));
    }

    #[test]
    fn an_extension_no_probe_reads_is_refused_by_name() {
        let (d, r) = world(&[("memories/x.md", "---\nabout: schema/a.proto\n---\n")]);
        let e = scan(d.path(), &r).unwrap_err();
        assert!(e.to_string().contains(".proto"), "{e}");
    }

    #[test]
    fn a_note_without_frontmatter_is_not_a_note() {
        let (d, r) = world(&[("memories/plain.md", "# just prose\n")]);
        assert!(scan(d.path(), &r).unwrap().is_empty());
    }

    #[test]
    fn unclosed_frontmatter_names_the_file() {
        let (d, r) = world(&[("memories/bad.md", "---\nabout: a.rs\n")]);
        let e = scan(d.path(), &r).unwrap_err();
        assert!(e.to_string().contains("bad.md"), "{e}");
    }

    #[test]
    fn the_explicit_form_still_reaches_every_knob() {
        let (d, r) = world(&[(
            "memories/x.md",
            "---\nanchors:\n  - key: surface::auth\n    probe: ast-map\n    shape: roster\n    position: { file: src/auth.ts, kind: function }\n---\n",
        )]);
        let notes = scan(d.path(), &r).unwrap();
        let Want::Declared(decl) = &notes[0].wants[0] else {
            panic!("expected a declared anchor");
        };
        assert_eq!(decl.key, "surface::auth");
        assert_eq!(
            decl.position,
            Some(json!({"file": "src/auth.ts", "kind": "function"}))
        );
    }
}
