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

#[derive(Debug, PartialEq, Eq)]
pub enum Reached {
    Over(String),
    In(String),
    Through(String),
}

impl Reached {
    pub fn at(&self) -> &str {
        match self {
            Self::Over(at) | Self::In(at) | Self::Through(at) => at,
        }
    }
}

pub fn fetched(coord: &str) -> Option<(Reached, Option<String>)> {
    let whole = match coord.strip_prefix("file://") {
        Some(path) => Reached::In(path.to_owned()),
        None => match coord.strip_prefix("sql://") {
            Some(db) => Reached::Through(db.to_owned()),
            None => match coord.starts_with("http://") || coord.starts_with("https://") {
                true => Reached::Over(coord.to_owned()),
                false => return None,
            },
        },
    };
    let (at, select) = match whole.at().split_once('#') {
        Some((at, select)) => (at.to_owned(), Some(select.to_owned())),
        None => (whole.at().to_owned(), None),
    };
    Some((
        match whole {
            Reached::Over(_) => Reached::Over(at),
            Reached::In(_) => Reached::In(at),
            Reached::Through(_) => Reached::Through(at),
        },
        select,
    ))
}

pub fn derive_name(url: &str, select: Option<&str>) -> String {
    let path = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let last = path
        .split(['?', '#'])
        .next()
        .unwrap_or(path)
        .rsplit('/')
        .find(|s| !s.is_empty())
        .unwrap_or(path);
    let last = match crate::probes::stem_of(last) {
        Some(stem) => stem,
        None => last,
    };
    let tail = select.and_then(|s| s.rsplit(['.', '/']).find(|x| !x.is_empty() && *x != "$"));
    let raw = match tail {
        Some(t) => format!("{last}-{t}"),
        None => last.to_owned(),
    };
    slug(&raw)
}

fn slug(raw: &str) -> String {
    let mut out = String::new();
    for c in raw.chars() {
        match c.is_ascii_alphanumeric() {
            true => out.push(c.to_ascii_lowercase()),
            false => {
                if !out.ends_with('-') && !out.is_empty() {
                    out.push('-');
                }
            }
        }
    }
    let trimmed = out.trim_matches('-');
    match trimmed.len() <= 64 {
        true => trimmed.to_owned(),
        false => trimmed[..64].trim_matches('-').to_owned(),
    }
}

fn taken(name: &str, at: &str, select: Option<&str>) -> CliError {
    CliError(format!(
        "`{name}` already names a fetched fact, and it points somewhere else (`{at}`{}). \
         Re-routing a name is a criteria change and goes through `revise`/`accept \
         --criteria`; to declare this one alongside it, give it its own name with --as",
        select
            .map(|s| format!(" selecting `{s}`"))
            .unwrap_or_default()
    ))
}

fn fetch_declared(
    root: &Path,
    catalog: &Catalog,
    name: &str,
    where_: &Reached,
    select: Option<&str>,
) -> Result<bool, CliError> {
    match where_ {
        Reached::Over(url) => match catalog.https().find(|(n, _)| *n == name) {
            Some((_, held)) if held.url == *url && held.select.as_deref() == select => Ok(false),
            Some((_, held)) => Err(taken(name, &held.url, held.select.as_deref())),
            None => {
                crate::probes::declare_http(
                    root,
                    name,
                    &crate::probes::HttpDecl {
                        url: url.clone(),
                        select: select.map(str::to_owned),
                        headers: Default::default(),
                    },
                )?;
                Ok(true)
            }
        },
        Reached::Through(db) => match catalog.sqls().find(|(n, _)| *n == name) {
            Some((_, held)) if held.url.as_deref() == Some(db.as_str()) => Ok(false),
            Some((_, held)) => Err(taken(
                name,
                held.url
                    .as_deref()
                    .unwrap_or("a database from the environment"),
                Some(&held.query),
            )),
            None => {
                let query = select.ok_or_else(|| {
                    CliError(
                        "a sql coordinate is `sql://<database>#<query>`, and this one names \
                         no query. A sql probe reports whatever its query returns, so there \
                         is nothing to report without one"
                            .to_owned(),
                    )
                })?;
                crate::probes::declare_sql(
                    root,
                    name,
                    &crate::probes::SqlDecl {
                        url: Some(db.clone()),
                        url_from_env: None,
                        query: query.to_owned(),
                        column: None,
                    },
                )?;
                Ok(true)
            }
        },
        Reached::In(path) => match catalog.files().find(|(n, _)| *n == name) {
            Some((_, held)) if held.path == *path && held.select.as_deref() == select => Ok(false),
            Some((_, held)) => Err(taken(name, &held.path, held.select.as_deref())),
            None => {
                crate::probes::declare_file(
                    root,
                    name,
                    &crate::probes::FileDecl {
                        path: path.clone(),
                        select: select.map(str::to_owned),
                        shaped: None,
                    },
                )?;
                Ok(true)
            }
        },
    }
}

#[derive(Debug, Default)]
pub struct Asked {
    pub coordinate: Option<String>,
    pub named: Option<String>,
    pub memory: Option<String>,
    pub record: Option<String>,
}

pub async fn run(
    rt: &Runtime,
    root: &Path,
    stores: &crate::stores::Stores,
    asked: Asked,
    json: bool,
) -> Result<i32, CliError> {
    let Asked {
        coordinate: coord,
        named,
        memory,
        record,
    } = asked;
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

    let mut catalog = Catalog::load(root)?;
    let mut wrote_probe = None;
    let mut wrote_anchor = false;
    let mut minted = false;
    let (coord, routed) = match fetched(&coord) {
        None => {
            let mut routed = crate::coord::route(&coord, None, &catalog)?;
            routed.position = crate::coord::resolve(rt, &routed, &catalog).await?;
            (coord, routed)
        }
        Some((where_, select)) => {
            let at = where_.at().to_owned();
            let name = match named {
                Some(given) => slug(&given),
                None => match where_ {
                    Reached::Through(_) => derive_name(&at, None),
                    _ => derive_name(&at, select.as_deref()),
                },
            };
            if name.is_empty() {
                return Err(CliError(format!(
                    "no name could be derived from `{at}`; give one with --as"
                )));
            }
            if fetch_declared(root, &catalog, &name, &where_, select.as_deref())? {
                wrote_probe = Some((
                    name.clone(),
                    match where_ {
                        Reached::Over(_) => "http",
                        Reached::In(_) => "file",
                        Reached::Through(_) => "sql",
                    },
                ));
                catalog = Catalog::load(root)?;
            }
            minted = true;
            (
                name.clone(),
                crate::coord::Routed {
                    probe: name,
                    shape: "value".to_owned(),
                    position: serde_json::json!({}),
                    params: serde_json::Value::Null,
                },
            )
        }
    };
    let named = record
        .as_deref()
        .map(|address| stores.locate(address, None))
        .transpose()?;

    let mut declared = already_declared(root, &catalog, &coord)?;
    if minted && !declared {
        crate::verbs::sync::declare(
            root,
            crate::verbs::sync::DEFAULT_FILE,
            &declaration(&coord, &routed),
        )?;
        declared = true;
        wrote_anchor = true;
    }
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

    if !json {
        if let Some((name, table)) = &wrote_probe {
            println!(
                "declared  .anchor/{}   [{table}.{name}]",
                crate::probes::RECIPES_FILE
            );
        }
        let axes = crate::shapes::get(&routed.shape)
            .map(crate::shapes::axes_of)
            .unwrap_or_default();
        match axes.is_empty() {
            true => println!("watching  {coord}   {}", routed.shape),
            false => println!("watching  {coord}   {}", axes.join(" · ")),
        }
        match (&note, fresh) {
            (Some(rel), true) if wrote_anchor => {
                println!("wrote     {rel}   and {}", crate::verbs::sync::DEFAULT_FILE)
            }
            (Some(rel), true) if declared => println!(
                "wrote     {rel}   binding only; {} already declares this coordinate",
                crate::verbs::sync::DEFAULT_FILE
            ),
            (Some(rel), true) => println!("wrote     {rel}"),
            (Some(rel), false) => println!("note      {rel}   already yours; left alone"),
            (None, true) => println!("declared  {}", crate::verbs::sync::DEFAULT_FILE),
            (None, false) if wrote_anchor => {
                println!("declared  {}", crate::verbs::sync::DEFAULT_FILE)
            }
            (None, false) => println!("declared  already declared elsewhere; left alone"),
        }
    }

    let synced = crate::verbs::sync::synced(
        rt,
        root,
        &stores.names,
        crate::verbs::sync::DEFAULT_FILE.to_owned(),
        false,
    )
    .await?;
    if !json {
        crate::verbs::sync::tell(&synced, false);
    }

    let key = gmr::AnchorKey::new(coord.clone());
    let mut attached = None;
    if let Some(reference) = named {
        let (version, landed) = crate::verbs::bind::assert_on(
            rt,
            reference.clone(),
            vec![key.clone()],
            gmr::Source::Adjudicated,
        )
        .await?;
        let address = crate::memories::addressed(&reference);
        let anchors: Vec<String> = landed.anchors.iter().map(ToString::to_string).collect();
        if !json {
            println!("bound     {address} → {}", anchors.join(", "));
            if version.is_none() {
                println!(
                    "          no version yet: the store could not answer for this record, \
                     so nothing about it has been verified"
                );
            }
        }
        attached = Some(serde_json::json!({
            "record": address, "anchors": anchors,
            "version": version.map(gmr::Version::into_inner),
        }));
    }

    let barren = rt.bindings_on(&key).await?.is_empty();
    match json {
        true => println!(
            "{}",
            serde_json::json!({
                "coordinate": coord, "probe": routed.probe, "shape": routed.shape,
                "position": routed.position, "declared_in": declared_in(note.as_deref()),
                "note": note, "wrote": fresh, "bound": attached, "barren": barren,
                "sync": synced,
            })
        ),
        false if barren => println!("memory    {UNWRITTEN}   nothing is bound here yet"),
        false => {}
    }
    Ok(synced.code())
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

#[cfg(test)]
mod fetched_coordinates {
    use super::*;

    #[test]
    fn a_url_is_reached_over_and_a_file_url_is_reached_in() {
        assert_eq!(
            fetched("https://x/a#$.last"),
            Some((
                Reached::Over("https://x/a".to_owned()),
                Some("$.last".to_owned())
            ))
        );
        assert_eq!(
            fetched("http://x/a"),
            Some((Reached::Over("http://x/a".to_owned()), None))
        );
        assert_eq!(
            fetched("file://deploy.yaml#$.service.replicas"),
            Some((
                Reached::In("deploy.yaml".to_owned()),
                Some("$.service.replicas".to_owned())
            )),
            "a file:// coordinate reaches into the tree; the scheme is what distinguishes it \
             from a bare path, which must keep routing to an extractor"
        );
        assert_eq!(
            fetched("src/lib.rs#run"),
            None,
            "a path coordinate must keep routing by extension; `#` alone does not make one \
             of these, and neither does having a dot in it"
        );
        assert_eq!(
            fetched("sql://sqlite://app.db#SELECT version FROM migrations"),
            Some((
                Reached::Through("sqlite://app.db".to_owned()),
                Some("SELECT version FROM migrations".to_owned())
            )),
            "one rule for all three: `scheme://<where>#<what>`. For sql the what is the \
             query, which is why a coordinate without one is refused rather than guessed at"
        );
        assert_eq!(
            fetched("sql://sqlite://app.db"),
            Some((Reached::Through("sqlite://app.db".to_owned()), None)),
            "parsing still succeeds; it is declaring that refuses, so the error can say why"
        );
        assert_eq!(
            fetched("deploy.yaml#replicas"),
            None,
            "and a bare config path is still the extractor's, however much it looks like \
             something this could read. Opting in is the whole point of the scheme"
        );
    }

    #[test]
    fn the_name_is_a_name_and_the_url_is_not() {
        assert_eq!(
            derive_name(
                "https://crates.io/api/v1/crates/serde",
                Some("$.crate.max_stable_version")
            ),
            "serde-max-stable-version"
        );
        assert_eq!(derive_name("https://x/quote", Some("$.last")), "quote-last");
        assert_eq!(derive_name("https://x/quote/", None), "quote");

        assert_eq!(
            derive_name("deploy.yaml", Some("$.service.replicas")),
            "deploy-replicas",
            "a config file drops the extension, because `deploy-yaml-replicas` names the \
             format and not the fact"
        );
        assert_eq!(
            derive_name("config/prod.toml", Some("$.limits.rps")),
            "prod-rps"
        );
        assert_eq!(
            derive_name(
                "https://crates.io/api/v1/crates/serde",
                Some("$.crate.downloads")
            ),
            "serde-downloads",
            "and the dot in `crates.io` is not an extension. Stripping at the last dot of \
             the whole path -- rather than a known format suffix on the last segment -- \
             renamed every crates.io anchor to `crates-...` the moment file:// arrived"
        );

        let long = derive_name(
            "https://x/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            Some("$.bbbbbbbbbbbbbbbbbbbbbbbbbb"),
        );
        assert!(
            long.len() <= 64,
            "D-3: an AnchorKey is at most 64 characters, and a real URL with a query string \
             goes past that easily. The name is derived and cut to fit; the URL lives in the \
             declaration, where nothing bounds it. Got {} chars: {long}",
            long.len()
        );
        assert!(
            !long.ends_with('-'),
            "and cutting must not leave a trailing separator: {long}"
        );
    }

    #[test]
    fn a_derived_name_is_a_legal_probe_name() {
        for (url, select) in [
            ("https://API.Example.COM/v1/Foo_Bar", Some("$.a.b")),
            ("https://x/a?q=1&r=2", Some("$.last")),
            ("https://x/weird...name", None),
        ] {
            let name = derive_name(url, select);
            assert!(
                !name.is_empty()
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                    && !name.starts_with('-')
                    && !name.contains("--"),
                "`{url}` derived `{name}`, which ProbeName would refuse. The name is minted \
                 for the user, so it has to be legal without them thinking about it"
            );
        }
    }
}
