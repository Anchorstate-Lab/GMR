use std::path::Path;

use gmr_transport::shell::Artifacts;

use crate::error::CliError;
use crate::probes::{Recipes, build_all, store_dir};

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
    for (name, recipe) in recipes.iter() {
        let version = recipe.version(name, root)?;
        let installed = artifacts
            .installed(&version)
            .map_err(|e| CliError(e.0))?
            .clone();
        let built = installed != version;
        rows.push(serde_json::json!({
            "probe": name,
            "recipe": version,
            "artifact": built.then(|| installed.as_str().to_owned()),
            "obs": { "schema": recipe.obs.schema, "at": recipe.obs.at, "facts": recipe.obs.facts },
        }));
    }

    if json {
        println!("{}", serde_json::json!({ "probes": rows }));
        return Ok(0);
    }

    for row in &rows {
        let built = match row["artifact"].as_str() {
            Some(a) => format!("→ {}", &a[..12]),
            None => "not built here — run `probes build`".to_owned(),
        };
        println!(
            "{}  {}  {built}",
            row["probe"].as_str().unwrap_or(""),
            &row["recipe"].as_str().unwrap_or("")[..12]
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
