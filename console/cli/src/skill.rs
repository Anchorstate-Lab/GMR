pub const SKILL_MD: &str = include_str!("../assets/SKILL.md");

pub const PROJECT_PATH: &str = ".claude/skills/gmr/SKILL.md";

pub fn global_path() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(std::path::PathBuf::from(home).join(".claude/skills/gmr/SKILL.md"))
}

pub struct Stale {
    pub path: String,
    pub refresh: &'static str,
}

fn differs(path: &std::path::Path) -> bool {
    match std::fs::read_to_string(path) {
        Ok(body) => body != SKILL_MD,
        Err(e) => e.kind() != std::io::ErrorKind::NotFound,
    }
}

pub fn stale(root: &std::path::Path) -> Vec<Stale> {
    [
        (Some(root.join(PROJECT_PATH)), "gmr init"),
        (global_path(), "gmr init --global"),
    ]
    .into_iter()
    .filter_map(|(path, refresh)| path.map(|path| (path, refresh)))
    .filter(|(path, _)| differs(path))
    .map(|(path, refresh)| Stale {
        path: path.display().to_string(),
        refresh,
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_doc_that_was_never_installed_is_not_stale() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!differs(&dir.path().join("nothing-here.md")));
    }

    #[test]
    fn a_doc_that_cannot_be_read_is_as_stale_as_one_that_is_wrong() {
        let dir = tempfile::tempdir().unwrap();
        assert!(
            differs(dir.path()),
            "absence is an answer — nobody installed it. Any other failure to read is not: \
             an agent that cannot read this file is in exactly the position a stale one puts \
             it in, holding no contract from this build, and the fix is the same either way"
        );
    }

    #[test]
    fn each_copy_names_the_command_that_actually_rewrites_it() {
        let dir = tempfile::tempdir().unwrap();
        let at = dir.path().join(PROJECT_PATH);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, "an older build wrote this").unwrap();

        let found = stale(dir.path());
        let project = found
            .iter()
            .find(|s| s.path == at.display().to_string())
            .expect("the project copy differs, so it is stale");

        assert_eq!(
            project.refresh, "gmr init",
            "the global copy is refreshed by `gmr init --global`; plain `gmr init` writes \
             only the project one. Telling a reader to run the wrong one leaves the doc \
             stale and doctor red with the instruction already carried out"
        );
    }
}
