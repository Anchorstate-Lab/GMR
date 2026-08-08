pub const SKILL_MD: &str = include_str!("../assets/SKILL.md");

pub const PROJECT_PATH: &str = ".claude/skills/gmr/SKILL.md";

pub fn global_path() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(std::path::PathBuf::from(home).join(".claude/skills/gmr/SKILL.md"))
}
