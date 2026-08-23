use std::path::{Path, PathBuf};

use gmr::Runtime;

use crate::error::CliError;
use crate::memories::NOTES_DIR;
use crate::probes::Catalog;

pub const UNWRITTEN: &str = "Say what the code cannot say about itself.";

fn slug_of(coord: &str) -> String {
    let (whole, part) = match coord.split_once('#') {
        Some((w, p)) => (w, Some(p)),
        None => (coord, None),
    };
    let stem = whole
        .rsplit('/')
        .next()
        .unwrap_or(whole)
        .rsplit_once('.')
        .map(|(s, _)| s)
        .unwrap_or(whole);
    let raw = match part {
        Some(p) => format!("{stem}-{p}"),
        None => stem.to_owned(),
    };
    raw.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

fn short_hash(coord: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in coord.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:x}")[..8].to_owned()
}

fn names(text: &str, coord: &str) -> bool {
    let Some(rest) = text.strip_prefix("---\n") else {
        return false;
    };
    let Some(end) = rest.find("\n---") else {
        return false;
    };
    rest[..end].lines().any(|line| {
        let said = line
            .strip_prefix("about:")
            .or_else(|| line.trim_start().strip_prefix("- "))
            .map(|v| v.trim().trim_matches('"'));
        said == Some(coord)
    })
}

fn note_path(root: &Path, coord: &str) -> Result<PathBuf, CliError> {
    let dir = root.join(NOTES_DIR);
    let first = dir.join(format!("{}.md", slug_of(coord)));
    let existing = match std::fs::read_to_string(&first) {
        Ok(text) => text,
        Err(_) => return Ok(first),
    };
    match names(&existing, coord) {
        true => Ok(first),
        false => Ok(dir.join(format!("{}-{}.md", slug_of(coord), short_hash(coord)))),
    }
}

fn write_note(
    path: &Path,
    coord: &str,
    memory: Option<&str>,
    declared: bool,
) -> Result<bool, CliError> {
    if path.exists() {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| CliError(format!("cannot create {parent:?}: {e}")))?;
    }
    let head = match declared {
        false => format!("about: {coord}"),
        true => format!("anchors:\n  - \"{coord}\""),
    };
    let body = memory.unwrap_or(UNWRITTEN);
    std::fs::write(path, format!("---\n{head}\n---\n\n{body}\n"))
        .map_err(|e| CliError(format!("cannot write {path:?}: {e}")))?;
    Ok(true)
}

fn already_declared(root: &Path, catalog: &Catalog, coord: &str) -> Result<bool, CliError> {
    let declared = crate::verbs::sync::read_declared(root, crate::verbs::sync::DEFAULT_FILE)?;
    let scanned = crate::memories::scan(root, catalog)?;
    Ok(crate::verbs::sync::merged(&declared, &scanned.notes)
        .iter()
        .any(|d| d.key == coord))
}

fn declaration(coord: &str, routed: &crate::coord::Routed) -> crate::verbs::sync::AnchorDecl {
    crate::verbs::sync::AnchorDecl {
        key: coord.to_owned(),
        probe: routed.probe.clone(),
        params: routed.params.clone(),
        position: Some(routed.position.clone()),
        shape: Some(routed.shape.clone()),
        rules: Vec::new(),
        terminal: Vec::new(),
        watch: None,
        settings: crate::settings::Declared::default(),
    }
}

pub async fn run(
    rt: &Runtime,
    root: &Path,
    stores: &crate::stores::Stores,
    coord: Option<String>,
    memory: Option<String>,
    record: Option<String>,
    json: bool,
) -> Result<i32, CliError> {
    let Some(coord) = coord else {
        return crate::verbs::sync::run(
            rt,
            root,
            &stores.names,
            crate::verbs::sync::DEFAULT_FILE.to_owned(),
            false,
            json,
        )
        .await;
    };

    let catalog = Catalog::load(root)?;
    let mut routed = crate::coord::route(&coord, None, &catalog)?;
    routed.position = crate::coord::resolve(rt, &routed, &catalog).await?;

    let declared = already_declared(root, &catalog, &coord)?;
    let mut note = None;
    let mut fresh = false;
    match &memory {
        Some(_) => {
            let path = note_path(root, &coord)?;
            fresh = write_note(&path, &coord, memory.as_deref(), declared)?;
            note = Some(
                path.strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        None => {
            if !declared {
                crate::verbs::sync::declare(
                    root,
                    crate::verbs::sync::DEFAULT_FILE,
                    &declaration(&coord, &routed),
                )?;
                fresh = true;
            }
        }
    }

    if json {
        println!(
            "{}",
            serde_json::json!({
                "coordinate": coord, "probe": routed.probe, "shape": routed.shape,
                "position": routed.position, "declared_in": declared_in(note.as_deref()),
                "note": note, "wrote": fresh, "record": record,
            })
        );
    } else {
        let axes = crate::shapes::get(&routed.shape)
            .map(crate::shapes::axes_of)
            .unwrap_or_default();
        match axes.is_empty() {
            true => println!("watching  {coord}   {}", routed.shape),
            false => println!("watching  {coord}   {}", axes.join(" · ")),
        }
        match (&note, fresh) {
            (Some(rel), true) if declared => println!(
                "wrote     {rel}   binding only; {} already declares this coordinate",
                crate::verbs::sync::DEFAULT_FILE
            ),
            (Some(rel), true) => println!("wrote     {rel}"),
            (Some(rel), false) => println!("note      {rel}   already yours; left alone"),
            (None, true) => println!("declared  {}", crate::verbs::sync::DEFAULT_FILE),
            (None, false) => println!("declared  already declared elsewhere; left alone"),
        }
    }

    let code = crate::verbs::sync::run(
        rt,
        root,
        &stores.names,
        crate::verbs::sync::DEFAULT_FILE.to_owned(),
        false,
        json,
    )
    .await?;

    let key = gmr::AnchorKey::new(coord.clone());
    let Some(address) = record else {
        if !json && rt.bindings_on(&key).await?.is_empty() {
            println!("memory    {UNWRITTEN}   nothing is bound here yet");
        }
        return Ok(code);
    };
    let reference = stores.locate(&address, None)?;
    let (version, landed) =
        crate::verbs::bind::assert_on(rt, reference.clone(), vec![key], gmr::Source::Adjudicated)
            .await?;
    if !json {
        println!(
            "bound     {} → {}",
            crate::memories::addressed(&reference),
            landed
                .anchors
                .iter()
                .map(|a| a.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        if version.is_none() {
            println!(
                "          no version yet: the store could not answer for this record, so \
                 nothing about it has been verified"
            );
        }
    }
    Ok(code)
}

fn declared_in(note: Option<&str>) -> &str {
    match note {
        Some(_) => "note",
        None => crate::verbs::sync::DEFAULT_FILE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slug_reads_like_the_thing_it_names() {
        assert_eq!(slug_of("src/auth.rs#create_session"), "auth-create_session");
        assert_eq!(slug_of("src/auth.rs"), "auth");
        assert_eq!(slug_of("docs/design.md#Two Anchors"), "design-Two-Anchors");
    }

    #[test]
    fn two_coordinates_that_share_a_slug_get_different_notes() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        std::fs::create_dir_all(root.join(NOTES_DIR)).unwrap();

        let a = note_path(root, "src/a/auth.rs#f").unwrap();
        write_note(&a, "src/a/auth.rs#f", None, false).unwrap();

        let b = note_path(root, "src/b/auth.rs#f").unwrap();
        assert_ne!(
            a, b,
            "the second coordinate must not land on the first note"
        );

        assert_eq!(note_path(root, "src/a/auth.rs#f").unwrap(), a);
    }

    #[test]
    fn a_note_written_under_an_existing_declaration_only_binds() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("memories/x.md");

        write_note(&path, "a.rs#f", Some("mine"), true).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();

        assert!(
            !text.contains("about:"),
            "the coordinate is already declared in anchors.toml. A second declaration \
             here gives one anchor two of them, and `merged` prefers the file — so every \
             later edit to this note's frontmatter would silently do nothing: {text}"
        );
        assert!(text.contains("anchors:"), "{text}");
        assert!(text.contains("a.rs#f"), "{text}");
        assert!(text.contains("mine"), "{text}");
    }

    #[test]
    fn a_note_that_only_binds_still_owns_its_slug() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        std::fs::create_dir_all(root.join(NOTES_DIR)).unwrap();

        let first = note_path(root, "src/a.rs#f").unwrap();
        write_note(&first, "src/a.rs#f", Some("mine"), true).unwrap();

        assert_eq!(
            note_path(root, "src/a.rs#f").unwrap(),
            first,
            "a note that binds without declaring names its coordinate under `anchors:` \
             rather than `about:`. Reading only `about:` makes the slug look like it \
             belongs to a different coordinate, and the next run writes a second file"
        );
    }

    #[test]
    fn an_existing_note_keeps_its_body() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("memories/x.md");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "---\nabout: a.rs#f\n---\n\nmine\n").unwrap();

        assert!(!write_note(&path, "a.rs#f", Some("theirs"), false).unwrap());
        assert!(std::fs::read_to_string(&path).unwrap().contains("mine"));
    }

    #[test]
    fn a_memory_given_on_the_command_line_lands_in_the_note() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("memories/x.md");
        write_note(&path, "a.rs#f", Some("auth must precede creation"), false).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("about: a.rs#f"), "{text}");
        assert!(text.contains("auth must precede creation"), "{text}");
        assert!(!text.contains(UNWRITTEN), "{text}");
    }
}
