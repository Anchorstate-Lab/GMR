pub const SKILL_MD: &str = include_str!("../assets/SKILL.md");

pub const PROJECT_PATH: &str = ".claude/skills/gmr/SKILL.md";

pub fn global_path() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(std::path::PathBuf::from(home).join(".claude/skills/gmr/SKILL.md"))
}

pub fn stale(root: &std::path::Path) -> Vec<String> {
    let installed = std::iter::once(root.join(PROJECT_PATH)).chain(global_path());
    installed
        .filter(|p| std::fs::read_to_string(p).is_ok_and(|body| body != SKILL_MD))
        .map(|p| p.display().to_string())
        .collect()
}
