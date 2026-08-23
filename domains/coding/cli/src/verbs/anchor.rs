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

fn about_of(text: &str) -> Option<String> {
    let rest = text.strip_prefix("---\n")?;
    let end = rest.find("\n---")?;
    rest[..end]
        .lines()
        .find_map(|l| l.strip_prefix("about:"))
        .map(|v| v.trim().to_owned())
}

fn note_path(root: &Path, coord: &str) -> Result<PathBuf, CliError> {
    let dir = root.join(NOTES_DIR);
    let first = dir.join(format!("{}.md", slug_of(coord)));
    let existing = match std::fs::read_to_string(&first) {
        Ok(text) => text,
        Err(_) => return Ok(first),
    };
    match about_of(&existing).as_deref() {
        Some(a) if a == coord => Ok(first),
        _ => Ok(dir.join(format!("{}-{}.md", slug_of(coord), short_hash(coord)))),
    }
}

fn write_note(path: &Path, coord: &str, memory: Option<&str>) -> Result<bool, CliError> {
    if path.exists() {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| CliError(format!("cannot create {parent:?}: {e}")))?;
    }
    let body = memory.unwrap_or(UNWRITTEN);
    std::fs::write(path, format!("---\nabout: {coord}\n---\n\n{body}\n"))
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
    let routed = crate::coord::route(&coord, None, &catalog)?;

    let mut note = None;
    let mut fresh = false;
    match &memory {
        Some(_) => {
            let path = note_path(root, &coord)?;
            fresh = write_note(&path, &coord, memory.as_deref())?;
            note = Some(
                path.strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        None => {
            if !already_declared(root, &catalog, &coord)? {
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
                "unwritten": fresh && memory.is_none() && record.is_none(),
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
            (Some(rel), true) => println!("wrote     {rel}"),
            (Some(rel), false) => println!("note      {rel}   already yours; left alone"),
            (None, true) => println!("declared  {}", crate::verbs::sync::DEFAULT_FILE),
            (None, false) => println!("declared  already declared elsewhere; left alone"),
        }
        if fresh && note.is_none() && record.is_none() {
            println!("memory    {UNWRITTEN}   nothing is bound here yet");
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

    let Some(address) = record else {
        return Ok(code);
    };
    let reference = stores.locate(&address, None)?;
    let (version, landed) = crate::verbs::bind::assert_on(
        rt,
        reference.clone(),
        vec![gmr::AnchorKey::new(coord)],
        gmr::Source::Adjudicated,
    )
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
        write_note(&a, "src/a/auth.rs#f", None).unwrap();

        let b = note_path(root, "src/b/auth.rs#f").unwrap();
        assert_ne!(
            a, b,
            "the second coordinate must not land on the first note"
        );

        assert_eq!(note_path(root, "src/a/auth.rs#f").unwrap(), a);
    }

    #[test]
    fn an_existing_note_keeps_its_body() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("memories/x.md");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "---\nabout: a.rs#f\n---\n\nmine\n").unwrap();

        assert!(!write_note(&path, "a.rs#f", Some("theirs")).unwrap());
        assert!(std::fs::read_to_string(&path).unwrap().contains("mine"));
    }

    #[test]
    fn a_memory_given_on_the_command_line_lands_in_the_note() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("memories/x.md");
        write_note(&path, "a.rs#f", Some("auth must precede creation")).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("about: a.rs#f"), "{text}");
        assert!(text.contains("auth must precede creation"), "{text}");
        assert!(!text.contains(UNWRITTEN), "{text}");
    }
}
