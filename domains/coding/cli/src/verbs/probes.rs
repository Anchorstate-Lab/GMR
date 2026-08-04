use std::path::Path;

use gmr_transport::shell::Artifacts;

use crate::error::CliError;
use crate::probes::{PINNED_FILE, RECIPES_FILE, Recipes, anchor_dir, build_all, store_dir};

pub fn build(root: &Path, json: bool) -> Result<i32, CliError> {
    let built = build_all(root, &store_dir(root))?;
    if json {
        let rows: Vec<_> = built
            .iter()
            .map(|i| {
                serde_json::json!({
                    "probe": i.name, "recipe": i.recipe, "artifact": i.artifact,
                })
            })
            .collect();
        println!("{}", serde_json::json!({ "installed": rows }));
    } else {
        for i in &built {
            println!(
                "{}  {} → {}",
                i.name,
                &i.recipe.as_str()[..12],
                &i.artifact.as_str()[..12]
            );
        }
        println!("{} probes installed", built.len());
    }
    Ok(0)
}

pub fn list(root: &Path, verbose: bool, json: bool) -> Result<i32, CliError> {
    let recipes = Recipes::load(root)?;
    let artifacts = Artifacts::new(store_dir(root));

    let mut rows = Vec::new();
    for v in coding_extract::vocabularies() {
        rows.push(serde_json::json!({
            "probe": v.name,
            "kind": "builtin",
            "version": coding_extract::registry()[&gmr::ProbeName::new(v.name)].version,
            "handles": v.handles,
            "obs": { "schema": v.schema, "at": v.at, "facts": v.facts },
        }));
    }
    for (name, recipe) in recipes.iter() {
        let installed = artifacts
            .installed(&gmr::ProbeName::new(name))
            .map_err(|e| CliError(e.0))?;
        rows.push(serde_json::json!({
            "probe": name,
            "kind": "shell",
            "version": installed.as_ref().map(|v| v.as_str().to_owned()),
            "handles": recipe.handles,
            "obs": { "schema": recipe.obs.schema, "at": recipe.obs.at, "facts": recipe.obs.facts },
        }));
    }

    if json {
        println!("{}", serde_json::json!({ "probes": rows }));
        return Ok(0);
    }

    for row in &rows {
        let version = match row["version"].as_str() {
            Some(v) => v[..12].to_owned(),
            None => "not installed here".to_owned(),
        };
        println!(
            "{}  {}  {version}",
            row["probe"].as_str().unwrap_or(""),
            row["kind"].as_str().unwrap_or("")
        );
        if verbose {
            println!("    obs {}", row["obs"]["schema"].as_str().unwrap_or(""));
            println!("    at    {}", join(&row["obs"]["at"]));
            println!("    facts {}", join(&row["obs"]["facts"]));
        }
    }
    Ok(0)
}

fn join(v: &serde_json::Value) -> String {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str())
                .collect::<Vec<_>>()
                .join(" · ")
        })
        .unwrap_or_default()
}

/// Assemble what a release ships: the recipes, their pinned versions, and only
/// the artifacts currently installed for them. The store accumulates every
/// artifact ever built here; a tarball should carry none of that history.
pub fn bundle(root: &Path, out: &Path, json: bool) -> Result<i32, CliError> {
    let recipes = Recipes::load(root)?;
    let artifacts = Artifacts::new(store_dir(root));
    let probes = out.join("probes");
    std::fs::create_dir_all(&probes)
        .map_err(|e| CliError(format!("cannot create {probes:?}: {e}")))?;

    let mut shipped = Vec::new();
    for (name, _) in recipes.iter() {
        let probe = gmr::ProbeName::new(name);
        let artifact = artifacts
            .installed(&probe)
            .map_err(|e| CliError(e.0))?
            .ok_or_else(|| {
                CliError(format!(
                    "`{name}` has no installed artifact; run `probes build` before bundling"
                ))
            })?;
        artifacts
            .resolve(&probe)
            .map_err(|e| CliError(format!("`{name}` does not verify: {}", e.0)))?;
        copy_dir(
            &store_dir(root).join(artifact.as_str()),
            &probes.join(artifact.as_str()),
        )?;
        shipped.push((name.to_owned(), artifact.as_str().to_owned()));
    }

    for file in [RECIPES_FILE, PINNED_FILE] {
        let from = match file {
            RECIPES_FILE => anchor_dir(root).join(file),
            _ => store_dir(root).join(file),
        };
        std::fs::copy(&from, probes.join(file))
            .map_err(|e| CliError(format!("cannot copy {from:?}: {e}")))?;
    }
    write_install_index(&probes, &recipes, root, &shipped)?;

    if json {
        println!(
            "{}",
            serde_json::json!({ "out": out.to_string_lossy(), "probes": shipped.len() })
        );
    } else {
        for (name, artifact) in &shipped {
            println!("{name}  {}", &artifact[..12]);
        }
        println!("{} probes bundled into {}", shipped.len(), out.display());
    }
    Ok(0)
}

/// Rewritten rather than copied: the working store keeps entries for recipe
/// versions that are no longer declared, and a release should not carry them.
fn write_install_index(
    probes: &Path,
    recipes: &Recipes,
    root: &Path,
    shipped: &[(String, String)],
) -> Result<(), CliError> {
    let mut index = serde_json::Map::new();
    for (name, artifact) in shipped {
        let recipe = recipes.version_of(name, root)?;
        index.insert(
            recipe.as_str().to_owned(),
            serde_json::Value::String(artifact.clone()),
        );
    }
    let body = serde_json::json!({
        "schema": "gmr.probe-install.v1",
        "installed": index,
    });
    std::fs::write(
        probes.join("installed.json"),
        serde_json::to_vec_pretty(&body).expect("install index must serialize"),
    )
    .map_err(|e| CliError(format!("cannot write the install index: {e}")))
}

fn copy_dir(from: &Path, to: &Path) -> Result<(), CliError> {
    std::fs::create_dir_all(to).map_err(|e| CliError(format!("cannot create {to:?}: {e}")))?;
    let entries =
        std::fs::read_dir(from).map_err(|e| CliError(format!("cannot read {from:?}: {e}")))?;
    for entry in entries.flatten() {
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            copy_dir(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst)
                .map_err(|e| CliError(format!("cannot copy to {dst:?}: {e}")))?;
            copy_mode(&src, &dst)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn copy_mode(src: &Path, dst: &Path) -> Result<(), CliError> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(src)
        .map_err(|e| CliError(format!("cannot stat {src:?}: {e}")))?
        .permissions()
        .mode();
    std::fs::set_permissions(dst, std::fs::Permissions::from_mode(mode))
        .map_err(|e| CliError(format!("cannot set the mode on {dst:?}: {e}")))
}

#[cfg(not(unix))]
fn copy_mode(_src: &Path, _dst: &Path) -> Result<(), CliError> {
    Ok(())
}
