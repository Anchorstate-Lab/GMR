use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::CliError;
use crate::memories::NOTES_DIR;
use crate::probes::{Catalog, RECIPES_FILE, Recipes, anchor_dir, state_dir, store_dir};

const GITIGNORE: &str = "\
# Declarations belong in git; the journal and the artifacts do not.
#
# The journal is append-only: every observe adds an entry, which is exactly what
# the design wants and exactly what a committed file should not look like. sync
# can rebuild it from the declarations, but a rebuild captures *now*, not the
# baseline it originally caught.
#
# Artifacts are built here, and carry this platform and this binary's hash.
*
!.gitignore
!anchors.toml
!probes.toml
";

fn bundled() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
    let dir = exe.parent()?;
    [dir.join("probes"), dir.parent()?.join("probes")]
        .into_iter()
        .find(|p| p.join(RECIPES_FILE).is_file())
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), CliError> {
    std::fs::create_dir_all(to).map_err(|e| CliError(format!("cannot create {to:?}: {e}")))?;
    let entries =
        std::fs::read_dir(from).map_err(|e| CliError(format!("cannot read {from:?}: {e}")))?;
    for entry in entries.flatten() {
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            copy_tree(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst)
                .map_err(|e| CliError(format!("cannot copy to {dst:?}: {e}")))?;
        }
    }
    Ok(())
}

fn write_new(path: &Path, body: &str) -> Result<bool, CliError> {
    if path.exists() {
        return Ok(false);
    }
    std::fs::write(path, body).map_err(|e| CliError(format!("cannot write {path:?}: {e}")))?;
    Ok(true)
}

fn extensions(at: &Path, out: &mut BTreeMap<String, usize>) {
    let Ok(entries) = std::fs::read_dir(at) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || name == "target" || name == "node_modules" {
            continue;
        }
        if path.is_dir() {
            extensions(&path, out);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            *out.entry(ext.to_owned()).or_default() += 1;
        }
    }
}

fn skill_target(root: &Path, global: bool) -> Result<PathBuf, CliError> {
    if global {
        return crate::skill::global_path()
            .ok_or_else(|| CliError("cannot find the skill directory: $HOME is not set".into()));
    }
    Ok(root.join(crate::skill::PROJECT_PATH))
}

pub fn run(root: &Path, json: bool, global: bool) -> Result<i32, CliError> {
    for dir in [anchor_dir(root), state_dir(root), store_dir(root)] {
        std::fs::create_dir_all(&dir)
            .map_err(|e| CliError(format!("cannot create {dir:?}: {e}")))?;
    }
    std::fs::create_dir_all(root.join(NOTES_DIR))
        .map_err(|e| CliError(format!("cannot create {NOTES_DIR}: {e}")))?;
    write_new(&anchor_dir(root).join(".gitignore"), GITIGNORE)?;

    let skill_target = skill_target(root, global)?;
    if let Some(parent) = skill_target.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| CliError(format!("cannot create {parent:?}: {e}")))?;
    }
    let skill_written = write_new(&skill_target, crate::skill::SKILL_MD)?;

    let mut installed: Vec<String> = coding_extract::vocabularies()
        .map(|v| v.name.to_owned())
        .collect();
    if let Some(from) = bundled() {
        write_new(
            &anchor_dir(root).join(RECIPES_FILE),
            &std::fs::read_to_string(from.join(RECIPES_FILE))
                .map_err(|e| CliError(format!("cannot read the bundled recipes: {e}")))?,
        )?;
        copy_tree(&from, &store_dir(root))?;
        let _ = std::fs::remove_file(store_dir(root).join(RECIPES_FILE));
        installed.extend(Recipes::load(root)?.iter().map(|(name, _)| name.to_owned()));
    }

    let catalog = Catalog::load(root)?;
    let mut counts = BTreeMap::new();
    extensions(root, &mut counts);
    let mut readable: Vec<(String, usize)> = Vec::new();
    let mut opaque: Vec<(String, usize)> = Vec::new();
    for (ext, n) in counts {
        match catalog.for_extension(&ext).is_some() {
            true => readable.push((ext, n)),
            false => opaque.push((ext, n)),
        }
    }
    readable.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    opaque.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    opaque.truncate(5);

    if json {
        println!(
            "{}",
            serde_json::json!({
                "probes": installed,
                "readable": readable.iter().map(|(e, n)| serde_json::json!({"ext": e, "files": n})).collect::<Vec<_>>(),
                "opaque": opaque.iter().map(|(e, n)| serde_json::json!({"ext": e, "files": n})).collect::<Vec<_>>(),
                "skill": { "path": skill_target.display().to_string(), "written": skill_written },
            })
        );
        return Ok(0);
    }

    if installed.is_empty() {
        println!("no probes are bundled with this build — run `probes build` to build them here");
    } else {
        println!("probes installed: {}", installed.join(" · "));
    }

    match readable.first() {
        None => println!("\nno file here can be read by any installed probe"),
        Some(_) => {
            println!("\nreadable:");
            for (ext, n) in &readable {
                println!("  .{ext}  {n} files");
            }
        }
    }
    if !opaque.is_empty() {
        println!(
            "\nno probe reads: {}",
            opaque
                .iter()
                .map(|(e, _)| format!(".{e}"))
                .collect::<Vec<_>>()
                .join(" · ")
        );
    }

    if skill_written {
        println!("\nskill doc written to {}", skill_target.display());
    }

    println!("\nNo anchors were opened. What is worth watching is yours to say:\n");
    println!("  gmr anchor {} -m '...'", example(&readable));
    println!("\nthen `gmr check` to ask whether it still holds.");
    Ok(0)
}

fn example(readable: &[(String, usize)]) -> String {
    match readable.first() {
        Some((ext, _)) => format!("src/<file>.{ext}#<functionName>"),
        None => "src/<file>#<name>".to_owned(),
    }
}
