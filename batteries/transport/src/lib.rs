//! `Transport` implementations. One module per backend, one feature per
//! module — adding a backend is adding a feature and a module, not a
//! new package (same convention as `crates/gmr-store`'s `sqlite` feature).

#[cfg(feature = "inproc")]
pub mod inproc;
#[cfg(feature = "shell")]
pub mod shell;

pub use gmr_probe::{PARAMS_ENV, POSITION_ENV};
