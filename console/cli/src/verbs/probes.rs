use std::path::Path;

use gmr_transport::shell::Artifacts;

use crate::error::CliError;
use gmr::Transport;
use gmr_transport::script::Script;

use crate::probes::{Catalog, RECIPES_FILE, Recipes, anchor_dir, build_all, store_dir};

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
            println!("{}  {} → {}", i.name, i.recipe.short(), i.artifact.short());
        }
        println!("{} probes installed", built.len());
    }
    Ok(0)
}

#[derive(serde::Serialize)]
struct ObsRow {
    schema: String,
    at: Vec<String>,
    identity: Vec<String>,
    facts: Vec<String>,
}

impl ObsRow {
    fn of(schema: &str, at: &[String], identity: &[String], facts: &[String]) -> Self {
        Self {
            schema: schema.to_owned(),
            at: at.to_vec(),
            identity: identity.to_vec(),
            facts: facts.to_vec(),
        }
    }
}

#[derive(serde::Serialize)]
struct Row {
    probe: String,
    kind: &'static str,
    version: Option<gmr::ProbeVersion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    address: Option<gmr::ProbeVersion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    run: Option<String>,
    handles: serde_json::Value,
    obs: ObsRow,
}

pub fn rows(root: &Path) -> Result<Vec<(String, &'static str)>, CliError> {
    Ok(built(root)?
        .into_iter()
        .map(|r| (r.probe, r.kind))
        .collect())
}

fn built(root: &Path) -> Result<Vec<Row>, CliError> {
    let recipes = Recipes::load(root)?;
    let artifacts = Artifacts::new(store_dir(root));

    let mut rows = Vec::new();
    let builtin = gmr_coding_pack::registry_uncached();
    for v in gmr_coding_pack::vocabularies() {
        rows.push(Row {
            probe: v.name.to_owned(),
            kind: "builtin",
            version: Some(builtin[&gmr::ProbeName::new(v.name)].version.clone()),
            address: None,
            run: None,
            handles: reads_json(v.reads),
            obs: ObsRow::of(
                v.schema,
                &owned(v.at),
                &gmr_coding_pack::recipe(v.name)
                    .map(|r| owned(r.identity))
                    .unwrap_or_default(),
                &owned(v.facts),
            ),
        });
    }
    let catalog = Catalog::load(root)?;
    for (name, decl) in catalog.scripts() {
        rows.push(Row {
            probe: name.to_owned(),
            kind: "script",
            version: Script::new(root, catalog.script_paths())
                .resolve(&gmr::ProbeName::new(name))
                .map(|d| d.version),
            address: None,
            run: Some(decl.run.clone()),
            handles: serde_json::json!(decl.handles),
            obs: ObsRow::of(
                &decl.obs.schema,
                &decl.obs.at,
                &decl.obs.identity,
                &decl.obs.facts,
            ),
        });
    }
    for (name, ask) in catalog.https() {
        let obs = crate::probes::http_obs();
        rows.push(Row {
            probe: name.to_owned(),
            kind: "http",
            version: Some(ask.version()),
            address: None,
            run: Some(ask.url.clone()),
            handles: serde_json::json!([]),
            obs: ObsRow::of(&obs.schema, &obs.at, &obs.identity, &obs.facts),
        });
    }
    for (name, ask) in catalog.files() {
        let obs = crate::probes::file_obs();
        rows.push(Row {
            probe: name.to_owned(),
            kind: "file",
            version: Some(ask.version()),
            address: None,
            run: Some(ask.path.clone()),
            handles: serde_json::json!([]),
            obs: ObsRow::of(&obs.schema, &obs.at, &obs.identity, &obs.facts),
        });
    }
    for (name, ask) in catalog.sqls() {
        let obs = crate::probes::sql_obs();
        rows.push(Row {
            probe: name.to_owned(),
            kind: "sql",
            version: Some(ask.version()),
            address: None,
            run: Some(ask.query.clone()),
            handles: serde_json::json!([]),
            obs: ObsRow::of(&obs.schema, &obs.at, &obs.identity, &obs.facts),
        });
    }
    let shell = gmr_transport::shell::Shell::new(root, store_dir(root));
    for (name, recipe) in recipes.iter() {
        let probe = gmr::ProbeName::new(name);
        rows.push(Row {
            probe: name.to_owned(),
            kind: "shell",
            version: shell.resolve(&probe).map(|d| d.version),
            address: artifacts.installed(&probe).map_err(|e| CliError(e.0))?,
            run: None,
            handles: serde_json::json!(recipe.handles),
            obs: ObsRow::of(
                &recipe.obs.schema,
                &recipe.obs.at,
                &recipe.obs.identity,
                &recipe.obs.facts,
            ),
        });
    }

    Ok(rows)
}

pub fn list(root: &Path, verbose: bool, json: bool) -> Result<i32, CliError> {
    let rows = built(root)?;

    if json {
        println!("{}", serde_json::json!({ "probes": rows }));
        return Ok(0);
    }

    for row in &rows {
        let version = match &row.version {
            Some(v) => v.short().to_owned(),
            None => "not installed here".to_owned(),
        };
        println!("{}  {}  {version}", row.probe, row.kind);
        if verbose {
            println!("    obs {}", row.obs.schema);
            println!("    at    {}", row.obs.at.join(" · "));
            if !row.obs.identity.is_empty() {
                println!("    is    {}", row.obs.identity.join(" · "));
            }
            println!("    facts {}", row.obs.facts.join(" · "));
        }
    }
    Ok(0)
}

fn owned(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

fn reads_json(reads: gmr_coding_pack::Reads) -> serde_json::Value {
    match reads {
        gmr_coding_pack::Reads::Extensions(exts) => serde_json::json!(exts),
        gmr_coding_pack::Reads::Anything => serde_json::json!("*"),
    }
}

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
        shipped.push((probe, artifact));
    }

    std::fs::copy(
        anchor_dir(root).join(RECIPES_FILE),
        probes.join(RECIPES_FILE),
    )
    .map_err(|e| CliError(format!("cannot copy {RECIPES_FILE}: {e}")))?;
    let bundled = Artifacts::new(&probes);
    for (probe, artifact) in &shipped {
        bundled
            .install(probe, artifact)
            .map_err(|e| CliError(format!("cannot record `{probe}` in the bundle: {}", e.0)))?;
    }

    if json {
        println!(
            "{}",
            serde_json::json!({ "out": out.to_string_lossy(), "probes": shipped.len() })
        );
    } else {
        for (probe, artifact) in &shipped {
            println!("{probe}  {}", artifact.short());
        }
        println!("{} probes bundled into {}", shipped.len(), out.display());
    }
    Ok(0)
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
