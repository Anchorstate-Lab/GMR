//! `ContentProvider` implementations. One module per backend, one feature
//! per module — adding a backend is adding a feature and a module, not a
//! new package (same convention as `crates/gmr-store`'s `sqlite` feature).

#[cfg(any(feature = "git", feature = "claude-code"))]
mod local_file;

#[cfg(feature = "git")]
pub mod git;

#[cfg(feature = "claude-code")]
pub mod claude_code;

#[cfg(feature = "mem0")]
pub mod mem0;

#[cfg(any(feature = "git", feature = "claude-code"))]
pub(crate) fn spend(budget: &gmr_probe::Budget) -> Result<(), gmr_content::ContentError> {
    budget
        .checkpoint()
        .map_err(|s| gmr_content::ContentError::spent(s.as_str()))
}
