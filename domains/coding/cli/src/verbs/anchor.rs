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

pub async fn run(
    rt: &Runtime,
    root: &Path,
    coord: Option<String>,
    memory: Option<String>,
    json: bool,
) -> Result<i32, CliError> {
    let Some(coord) = coord else {
        return crate::verbs::sync::run(
            rt,
            root,
            crate::verbs::sync::DEFAULT_FILE.to_owned(),
            false,
            json,
        )
        .await;
    };

    let catalog = Catalog::load(root)?;
    let routed = crate::coord::route(&coord, None, &catalog)?;
    let path = note_path(root, &coord)?;
    let wrote = write_note(&path, &coord, memory.as_deref())?;
    let rel = path
        .strip_prefix(root)
        .unwrap_or(&path)
        .to_string_lossy()
        .into_owned();

    if json {
        println!(
            "{}",
            serde_json::json!({
                "coordinate": coord, "probe": routed.probe, "shape": routed.shape,
                "position": routed.position, "note": rel, "wrote": wrote,
                "unwritten": wrote && memory.is_none(),
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
        match (wrote, memory.is_some()) {
            (true, true) => println!("wrote     {rel}"),
            (true, false) => println!("wrote     {rel}   {UNWRITTEN}"),
            (false, _) => println!("note      {rel}   already yours; left alone"),
        }
    }

    crate::verbs::sync::run(
        rt,
        root,
        crate::verbs::sync::DEFAULT_FILE.to_owned(),
        false,
        json,
    )
    .await
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
