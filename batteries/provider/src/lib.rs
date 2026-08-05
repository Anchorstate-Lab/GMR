//! `ContentProvider` implementations. One module per backend, one feature
//! per module — adding a backend is adding a feature and a module, not a
//! new package (same convention as `crates/gmr-store`'s `sqlite` feature).

#[cfg(feature = "git")]
pub mod git;

#[cfg(feature = "claude-code")]
pub mod claude_code;
