use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use gmr::{Kind, ProbeVersion};
use gmr_transport::shell::{Artifacts, publish};
use serde::{Deserialize, Serialize};

use crate::error::CliError;

pub const RECIPES_FILE: &str = "probes.toml";

pub const RECIPE_SCHEMA: &str = "gmr.probe-recipe.v1";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Obs {
    pub schema: String,
    #[serde(default)]
    pub at: Vec<String>,
    #[serde(default)]
    pub identity: Vec<String>,
    #[serde(default)]
    pub facts: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Recipe {
    #[serde(default)]
    pub build: Vec<String>,
    pub stage: BTreeMap<String, String>,
    pub entrypoint: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub env_from_host: Vec<String>,
    pub sources: Vec<String>,
    pub obs: Obs,
    #[serde(default)]
    pub handles: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HeaderDecl {
    Given(String),
    FromEnv(String),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HttpDecl {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub select: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, HeaderDecl>,
}

impl HttpDecl {
    pub fn ask(&self) -> gmr_transport::http::Ask {
        gmr_transport::http::Ask {
            url: self.url.clone(),
            select: self.select.clone(),
            headers: self
                .headers
                .iter()
                .map(|(name, value)| {
                    let value = match value {
                        HeaderDecl::Given(v) => gmr_transport::http::Header::Given(v.clone()),
                        HeaderDecl::FromEnv(v) => gmr_transport::http::Header::FromEnv(v.clone()),
                    };
                    (name.clone(), value)
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FileDecl {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub select: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shaped: Option<String>,
}

impl FileDecl {
    pub fn ask(&self) -> gmr_transport::file::Ask {
        gmr_transport::file::Ask {
            path: self.path.clone(),
            select: self.select.clone(),
            shaped: self
                .shaped
                .as_deref()
                .and_then(gmr_transport::file::Shaped::of_extension),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SqlDecl {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url_from_env: Option<String>,
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<String>,
}

impl SqlDecl {
    pub fn source(&self) -> Result<gmr_transport::sql::Source, CliError> {
        match (&self.url, &self.url_from_env) {
            (Some(url), None) => Ok(gmr_transport::sql::Source::Given(url.clone())),
            (None, Some(var)) => Ok(gmr_transport::sql::Source::FromEnv(var.clone())),
            (None, None) => Err(CliError(
                "a sql probe needs either `url` or `url_from_env`".to_owned(),
            )),
            (Some(_), Some(_)) => Err(CliError(
                "a sql probe names both `url` and `url_from_env`; say which one it is".to_owned(),
            )),
        }
    }

    pub fn ask(&self) -> Option<gmr_transport::sql::Ask> {
        Some(gmr_transport::sql::Ask {
            source: self.source().ok()?,
            query: self.query.clone(),
            column: self.column.clone(),
        })
    }
}

pub fn sql_obs() -> Obs {
    Obs {
        schema: gmr_transport::sql::SCHEMA.to_owned(),
        at: Vec::new(),
        identity: Vec::new(),
        facts: vec![gmr_transport::sql::VALUE.to_owned()],
    }
}

pub fn file_obs() -> Obs {
    Obs {
        schema: gmr_transport::file::SCHEMA.to_owned(),
        at: Vec::new(),
        identity: Vec::new(),
        facts: vec![gmr_transport::file::VALUE.to_owned()],
    }
}

pub fn http_obs() -> Obs {
    Obs {
        schema: gmr_transport::http::SCHEMA.to_owned(),
        at: Vec::new(),
        identity: Vec::new(),
        facts: vec![gmr_transport::http::VALUE.to_owned()],
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScriptDecl {
    pub run: String,
    pub obs: Obs,
    #[serde(default)]
    pub handles: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct File {
    #[serde(default)]
    probe: BTreeMap<String, Recipe>,
    #[serde(default)]
    script: BTreeMap<String, ScriptDecl>,
    #[serde(default)]
    http: BTreeMap<String, HttpDecl>,
    #[serde(default)]
    file: BTreeMap<String, FileDecl>,
    #[serde(default)]
    sql: BTreeMap<String, SqlDecl>,
}

#[derive(Debug, Default)]
pub struct Recipes {
    declared: BTreeMap<String, Recipe>,
}

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
        Ok(Self { declared })
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

fn read_file(root: &Path) -> Result<File, CliError> {
    let path = anchor_dir(root).join(RECIPES_FILE);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(File::default());
    };
    toml::from_str::<File>(&text)
        .map_err(|e| CliError(format!("cannot read {}: {e}", path.display())))
}

impl Recipe {
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
        Ok(ProbeVersion::of(gmr::core::content_hash_of(&value)?))
    }

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

pub fn build_all(root: &Path, store: &Path) -> Result<Vec<Installed>, CliError> {
    let recipes = Recipes::load(root)?;
    let artifacts = Artifacts::new(store);
    let mut out = Vec::new();
    for (name, recipe) in recipes.iter() {
        out.push(build_one(root, &artifacts, name, recipe)?);
    }
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
        version.clone(),
        &recipe.entrypoint,
        recipe.args.clone(),
        env,
    )
    .map_err(|e| CliError(format!("cannot publish `{name}`: {}", e.0)))?;

    artifacts
        .install(&gmr::ProbeName::new(name), &artifact)
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

pub struct Catalog {
    recipes: Recipes,
    scripts: BTreeMap<String, ScriptDecl>,
    https: BTreeMap<String, HttpDecl>,
    files: BTreeMap<String, FileDecl>,
    sqls: BTreeMap<String, SqlDecl>,
}

impl Catalog {
    pub fn load(root: &Path) -> Result<Self, CliError> {
        let file = read_file(root)?;
        Ok(Self {
            recipes: Recipes::load(root)?,
            scripts: file.script,
            https: file.http,
            files: file.file,
            sqls: file.sql,
        })
    }

    pub fn script_paths(&self) -> BTreeMap<gmr::ProbeName, PathBuf> {
        self.scripts
            .iter()
            .map(|(n, d)| (gmr::ProbeName::new(n.clone()), PathBuf::from(&d.run)))
            .collect()
    }

    fn builtin(name: &str) -> Option<&'static coding_extract::Vocabulary> {
        coding_extract::vocabularies().find(|v| v.name == name)
    }

    pub fn kind_of(&self, name: &str) -> Kind {
        if Self::builtin(name).is_some() {
            return Kind::new("builtin");
        }
        if self.https.contains_key(name) {
            return Kind::new("http");
        }
        if self.files.contains_key(name) {
            return Kind::new("file");
        }
        if self.sqls.contains_key(name) {
            return Kind::new("sql");
        }
        match self.scripts.contains_key(name) {
            true => Kind::new("script"),
            false => Kind::new("shell"),
        }
    }

    pub fn obs_of(&self, name: &str) -> Result<Obs, CliError> {
        if let Some(v) = Self::builtin(name) {
            return Ok(Obs {
                schema: v.schema.to_owned(),
                at: v.at.iter().map(|s| (*s).to_owned()).collect(),
                identity: coding_extract::recipe(name)
                    .map(|r| r.identity.iter().map(|s| (*s).to_owned()).collect())
                    .unwrap_or_default(),
                facts: v.facts.iter().map(|s| (*s).to_owned()).collect(),
            });
        }
        if self.https.contains_key(name) {
            return Ok(http_obs());
        }
        if self.files.contains_key(name) {
            return Ok(file_obs());
        }
        if self.sqls.contains_key(name) {
            return Ok(sql_obs());
        }
        if let Some(d) = self.scripts.get(name) {
            return Ok(d.obs.clone());
        }
        Ok(self.recipes.get(name)?.obs.clone())
    }

    pub fn for_extension(&self, ext: &str) -> Option<String> {
        coding_extract::declares(ext)
            .map(str::to_owned)
            .or_else(|| {
                self.scripts
                    .iter()
                    .find(|(_, d)| d.handles.iter().any(|h| h == ext))
                    .map(|(n, _)| n.clone())
            })
            .or_else(|| self.recipes.for_extension(ext).map(str::to_owned))
            .or_else(|| coding_extract::catchall().map(str::to_owned))
    }

    pub fn scripts(&self) -> impl Iterator<Item = (&str, &ScriptDecl)> {
        self.scripts.iter().map(|(n, d)| (n.as_str(), d))
    }

    pub fn https(&self) -> impl Iterator<Item = (&str, &HttpDecl)> {
        self.https.iter().map(|(n, d)| (n.as_str(), d))
    }

    pub fn files(&self) -> impl Iterator<Item = (&str, &FileDecl)> {
        self.files.iter().map(|(n, d)| (n.as_str(), d))
    }

    pub fn sqls(&self) -> impl Iterator<Item = (&str, &SqlDecl)> {
        self.sqls.iter().map(|(n, d)| (n.as_str(), d))
    }

    pub fn asks(&self) -> BTreeMap<gmr::ProbeName, gmr_transport::http::Ask> {
        self.https
            .iter()
            .map(|(n, d)| (gmr::ProbeName::new(n.clone()), d.ask()))
            .collect()
    }
}

pub struct Declared {
    root: PathBuf,
}

impl Declared {
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl gmr_transport::http::Asks for Declared {
    fn ask(&self, name: &gmr::ProbeName) -> Option<gmr_transport::http::Ask> {
        Catalog::load(&self.root)
            .ok()?
            .https()
            .find(|(n, _)| *n == name.as_str())
            .map(|(_, d)| d.ask())
    }
}

impl gmr_transport::sql::Asks for Declared {
    fn ask(&self, name: &gmr::ProbeName) -> Option<gmr_transport::sql::Ask> {
        Catalog::load(&self.root)
            .ok()?
            .sqls()
            .find(|(n, _)| *n == name.as_str())
            .and_then(|(_, d)| d.ask())
    }
}

impl gmr_transport::file::Asks for Declared {
    fn ask(&self, name: &gmr::ProbeName) -> Option<gmr_transport::file::Ask> {
        Catalog::load(&self.root)
            .ok()?
            .files()
            .find(|(n, _)| *n == name.as_str())
            .map(|(_, d)| d.ask())
    }
}

pub fn declare_http(root: &Path, name: &str, decl: &HttpDecl) -> Result<(), CliError> {
    declare_probe(root, "http", name, decl)
}

pub fn declare_file(root: &Path, name: &str, decl: &FileDecl) -> Result<(), CliError> {
    declare_probe(root, "file", name, decl)
}

pub fn declare_sql(root: &Path, name: &str, decl: &SqlDecl) -> Result<(), CliError> {
    declare_probe(root, "sql", name, decl)
}

fn declare_probe<T: Serialize>(
    root: &Path,
    table: &str,
    name: &str,
    decl: &T,
) -> Result<(), CliError> {
    let mut block = BTreeMap::new();
    block.insert(name.to_owned(), decl);
    let mut outer = BTreeMap::new();
    outer.insert(table.to_owned(), block);
    let written = toml::to_string(&outer)
        .map_err(|e| CliError(format!("cannot write a probe for `{name}`: {e}")))?;

    let dir = anchor_dir(root);
    std::fs::create_dir_all(&dir)
        .map_err(|e| CliError(format!("cannot create {}: {e}", dir.display())))?;
    let path = dir.join(RECIPES_FILE);
    let mut held = std::fs::read_to_string(&path).unwrap_or_default();
    if !held.is_empty() && !held.ends_with('\n') {
        held.push('\n');
    }
    if !held.is_empty() {
        held.push('\n');
    }
    std::fs::write(&path, format!("{held}{written}"))
        .map_err(|e| CliError(format!("cannot write {}: {e}", path.display())))
}

pub fn stem_of(segment: &str) -> Option<&str> {
    let (stem, ext) = segment.rsplit_once('.')?;
    if stem.is_empty() {
        return None;
    }
    gmr_transport::file::Shaped::of_extension(ext).map(|_| stem)
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

        let installed = Artifacts::new(&store)
            .installed(&gmr::ProbeName::new("demo"))
            .unwrap();
        assert_eq!(installed, Some(built[0].artifact.clone()));
        assert_ne!(built[0].recipe, built[0].artifact);
    }
}
