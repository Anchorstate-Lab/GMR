use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use gmr::ProbeVersion;
use gmr_transport::shell::{Artifacts, publish};
use serde::{Deserialize, Serialize};

use crate::error::CliError;

pub const RECIPES_FILE: &str = "probes.toml";

pub const RECIPE_SCHEMA: &str = "gmr.probe-recipe.v1";

pub const PINNED_FILE: &str = "recipes.json";

pub const PINNED_SCHEMA: &str = "gmr.probe-recipes.v1";

/// Recipe versions computed at release time. A user machine has the artifacts
/// but not the sources, so it cannot earn these hashes itself; shipping them
/// is what lets `probe = "<name>"` resolve without a toolchain.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Pinned {
    pub schema: String,
    pub versions: BTreeMap<String, String>,
}

/// The obs vocabulary a probe emits. Deliberately outside the recipe version:
/// it does not change the derivation rule, only which shapes fit.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Obs {
    pub schema: String,
    #[serde(default)]
    pub at: Vec<String>,
    #[serde(default)]
    pub facts: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Recipe {
    /// A script probe has no build step; staging is enough.
    #[serde(default)]
    pub build: Vec<String>,
    /// Artifact-relative path -> repo-relative source path.
    pub stage: BTreeMap<String, String>,
    pub entrypoint: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Host variables pulled into the closure; using any downgrades verifiability.
    #[serde(default)]
    pub env_from_host: Vec<String>,
    pub sources: Vec<String>,
    pub obs: Obs,
    /// File extensions this probe can read. Routes `about:` to a probe without
    /// the CLI knowing any language names. Outside the version, like `obs`.
    #[serde(default)]
    pub handles: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct File {
    probe: BTreeMap<String, Recipe>,
}

#[derive(Debug, Default)]
pub struct Recipes {
    declared: BTreeMap<String, Recipe>,
    pinned: BTreeMap<String, String>,
}

/// What the recipe version hashes. Deliberately excludes platform, built-binary
/// hashes and captured host env: those are what make artifact versions local.
#[derive(Serialize)]
struct Record<'a> {
    schema: &'a str,
    name: &'a str,
    build: &'a [String],
    stage: &'a BTreeMap<String, String>,
    entrypoint: &'a str,
    args: &'a [String],
    env: &'a BTreeMap<String, String>,
    env_from_host: &'a [String],
    output_contract: &'a str,
    sources: Vec<(String, String)>,
}

impl Recipes {
    pub fn load(root: &Path) -> Result<Self, CliError> {
        let path = anchor_dir(root).join(RECIPES_FILE);
        let declared = match std::fs::read_to_string(&path) {
            Ok(text) => {
                toml::from_str::<File>(&text)
                    .map_err(|e| CliError(format!("cannot read {}: {e}", path.display())))?
                    .probe
            }
            Err(_) => BTreeMap::new(),
        };
        Ok(Self {
            declared,
            pinned: read_pinned(&store_dir(root).join(PINNED_FILE))?,
        })
    }

    pub fn get(&self, name: &str) -> Result<&Recipe, CliError> {
        self.declared.get(name).ok_or_else(|| {
            CliError(format!(
                "no probe recipe named `{name}`; {} declares {}",
                RECIPES_FILE,
                match self.declared.is_empty() {
                    true => "none".to_owned(),
                    false => self
                        .declared
                        .keys()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(" · "),
                }
            ))
        })
    }

    /// Pinned wins: on a user machine the sources are absent by design, and a
    /// version earned at release time is still earned.
    pub fn version_of(&self, name: &str, root: &Path) -> Result<ProbeVersion, CliError> {
        if let Some(v) = self.pinned.get(name) {
            return Ok(ProbeVersion::new(v.clone()));
        }
        self.get(name)?.version(name, root)
    }

    pub fn is_pinned(&self, name: &str) -> bool {
        self.pinned.contains_key(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Recipe)> {
        self.declared.iter().map(|(k, v)| (k.as_str(), v))
    }

    pub fn for_extension(&self, ext: &str) -> Option<&str> {
        self.declared
            .iter()
            .find(|(_, r)| r.handles.iter().any(|h| h == ext))
            .map(|(name, _)| name.as_str())
    }
}

fn read_pinned(path: &Path) -> Result<BTreeMap<String, String>, CliError> {
    let Ok(bytes) = std::fs::read(path) else {
        return Ok(BTreeMap::new());
    };
    let pinned: Pinned = serde_json::from_slice(&bytes)
        .map_err(|e| CliError(format!("cannot read {}: {e}", path.display())))?;
    if pinned.schema != PINNED_SCHEMA {
        return Err(CliError(format!(
            "{} declares schema `{}`, but this build only accepts `{PINNED_SCHEMA}`",
            path.display(),
            pinned.schema
        )));
    }
    Ok(pinned.versions)
}

/// Emitted by `probes build` so a release can ship versions it earned here.
fn write_pinned(store: &Path, built: &[Installed]) -> Result<(), CliError> {
    let pinned = Pinned {
        schema: PINNED_SCHEMA.to_owned(),
        versions: built
            .iter()
            .map(|i| (i.name.clone(), i.recipe.as_str().to_owned()))
            .collect(),
    };
    let body = serde_json::to_vec_pretty(&pinned).expect("pinned recipes must serialize");
    std::fs::write(store.join(PINNED_FILE), body)
        .map_err(|e| CliError(format!("cannot write {PINNED_FILE}: {e}")))
}

impl Recipe {
    /// Earned from tracked source bytes, so it is the same on every platform.
    pub fn version(&self, name: &str, root: &Path) -> Result<ProbeVersion, CliError> {
        let record = Record {
            schema: RECIPE_SCHEMA,
            name,
            build: &self.build,
            stage: &self.stage,
            entrypoint: &self.entrypoint,
            args: &self.args,
            env: &self.env,
            env_from_host: &self.env_from_host,
            output_contract: gmr::OUTCOME_CONTRACT,
            sources: self.source_hashes(root)?,
        };
        let value = serde_json::to_value(&record)
            .map_err(|e| CliError(format!("cannot canonicalise the recipe for {name}: {e}")))?;
        Ok(ProbeVersion::new(
            gmr::core::content_hash_of(&value).into_inner(),
        ))
    }

    /// A missing declared source is fatal: silently hashing a smaller closure
    /// would let a real criteria change slip past the revision gate.
    fn source_hashes(&self, root: &Path) -> Result<Vec<(String, String)>, CliError> {
        let mut out = Vec::new();
        for declared in &self.sources {
            let path = root.join(declared);
            if !path.exists() {
                return Err(CliError(format!(
                    "recipe source `{declared}` does not exist; refusing to hash a smaller closure"
                )));
            }
            collect_hashes(root, &path, &mut out)?;
        }
        out.sort();
        Ok(out)
    }
}

fn collect_hashes(root: &Path, at: &Path, out: &mut Vec<(String, String)>) -> Result<(), CliError> {
    if at.is_dir() {
        let entries = std::fs::read_dir(at)
            .map_err(|e| CliError(format!("cannot read {}: {e}", at.display())))?;
        for entry in entries.flatten() {
            collect_hashes(root, &entry.path(), out)?;
        }
        return Ok(());
    }
    let bytes = std::fs::read(at).map_err(|e| CliError(format!("cannot read {at:?}: {e}")))?;
    let rel = at.strip_prefix(root).unwrap_or(at).to_string_lossy();
    out.push((
        rel.replace('\\', "/"),
        gmr::core::content_hash_of_bytes(&bytes).into_inner(),
    ));
    Ok(())
}

pub struct Installed {
    pub name: String,
    pub recipe: ProbeVersion,
    pub artifact: ProbeVersion,
}

/// The developer path. Users get artifacts built at release time instead.
pub fn build_all(root: &Path, store: &Path) -> Result<Vec<Installed>, CliError> {
    let recipes = Recipes::load(root)?;
    let artifacts = Artifacts::new(store);
    let mut out = Vec::new();
    for (name, recipe) in recipes.iter() {
        out.push(build_one(root, &artifacts, name, recipe)?);
    }
    write_pinned(store, &out)?;
    Ok(out)
}

pub fn build_one(
    root: &Path,
    artifacts: &Artifacts,
    name: &str,
    recipe: &Recipe,
) -> Result<Installed, CliError> {
    let version = recipe.version(name, root)?;

    if let Some((program, args)) = recipe.build.split_first() {
        let status = std::process::Command::new(program)
            .args(args)
            .current_dir(root)
            .status()
            .map_err(|e| CliError(format!("cannot run the build for `{name}`: {e}")))?;
        if !status.success() {
            return Err(CliError(format!(
                "the build for `{name}` exited with {status}"
            )));
        }
    }

    let staging = tempfile::tempdir()
        .map_err(|e| CliError(format!("cannot make a staging directory for `{name}`: {e}")))?;
    for (inside, from) in &recipe.stage {
        let src = root.join(from);
        let dst = staging.path().join(inside);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CliError(format!("cannot create {parent:?}: {e}")))?;
        }
        std::fs::copy(&src, &dst).map_err(|e| {
            CliError(format!(
                "cannot stage `{from}` for `{name}`: {e}; did the build produce it?"
            ))
        })?;
        copy_mode(&src, &dst)?;
    }

    let mut env = recipe.env.clone();
    for key in &recipe.env_from_host {
        let value = std::env::var(key).map_err(|_| {
            CliError(format!(
                "`{name}` declares env_from_host `{key}`, but it is not set here"
            ))
        })?;
        env.insert(key.clone(), value);
    }

    let artifact = publish(
        artifacts,
        staging.path(),
        gmr::Kind::new("shell"),
        &recipe.entrypoint,
        recipe.args.clone(),
        env,
    )
    .map_err(|e| CliError(format!("cannot publish `{name}`: {}", e.0)))?;

    artifacts
        .install(&version, &artifact)
        .map_err(|e| CliError(format!("cannot install `{name}`: {}", e.0)))?;

    Ok(Installed {
        name: name.to_owned(),
        recipe: version,
        artifact,
    })
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

pub fn anchor_dir(root: &Path) -> PathBuf {
    root.join(".anchor")
}

pub fn store_dir(root: &Path) -> PathBuf {
    anchor_dir(root).join("probes")
}

pub fn state_dir(root: &Path) -> PathBuf {
    anchor_dir(root).join("state")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(sources: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".anchor")).unwrap();
        for (path, body) in sources {
            let p = dir.path().join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, body).unwrap();
        }
        dir
    }

    const TOML: &str = r#"
[probe.demo]
stage = { probe = "src/probe.sh" }
entrypoint = "probe"
sources = ["src"]
obs = { schema = "gmr.probe-coord.v1", at = ["file"], facts = ["line"] }
"#;

    #[test]
    fn the_same_sources_earn_the_same_version() {
        let a = world(&[(".anchor/probes.toml", TOML), ("src/probe.sh", "echo 1")]);
        let b = world(&[(".anchor/probes.toml", TOML), ("src/probe.sh", "echo 1")]);
        let va = Recipes::load(a.path())
            .unwrap()
            .get("demo")
            .unwrap()
            .version("demo", a.path())
            .unwrap();
        let vb = Recipes::load(b.path())
            .unwrap()
            .get("demo")
            .unwrap()
            .version("demo", b.path())
            .unwrap();
        assert_eq!(va, vb);
    }

    #[test]
    fn one_changed_byte_earns_a_different_version() {
        let a = world(&[(".anchor/probes.toml", TOML), ("src/probe.sh", "echo 1")]);
        let b = world(&[(".anchor/probes.toml", TOML), ("src/probe.sh", "echo 2")]);
        let va = Recipes::load(a.path())
            .unwrap()
            .get("demo")
            .unwrap()
            .version("demo", a.path())
            .unwrap();
        let vb = Recipes::load(b.path())
            .unwrap()
            .get("demo")
            .unwrap()
            .version("demo", b.path())
            .unwrap();
        assert_ne!(va, vb);
    }

    #[test]
    fn a_missing_source_is_refused_not_silently_dropped() {
        let d = world(&[(".anchor/probes.toml", TOML)]);
        let e = Recipes::load(d.path())
            .unwrap()
            .get("demo")
            .unwrap()
            .version("demo", d.path())
            .unwrap_err();
        assert!(e.to_string().contains("smaller closure"), "{e}");
    }

    #[test]
    fn an_unknown_recipe_names_what_is_declared() {
        let d = world(&[(".anchor/probes.toml", TOML), ("src/probe.sh", "echo 1")]);
        let e = Recipes::load(d.path()).unwrap().get("nope").unwrap_err();
        assert!(e.to_string().contains("demo"), "{e}");
    }

    #[test]
    fn a_script_probe_builds_stages_and_installs() {
        let d = world(&[(".anchor/probes.toml", TOML), ("src/probe.sh", "echo '{}'")]);
        let store = d.path().join(".anchor").join("probes");
        let built = build_all(d.path(), &store).unwrap();
        assert_eq!(built.len(), 1);

        let installed = Artifacts::new(&store).installed(&built[0].recipe).unwrap();
        assert_eq!(installed, built[0].artifact);
        assert_ne!(built[0].recipe, built[0].artifact);
    }
}
