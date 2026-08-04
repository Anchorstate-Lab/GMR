//! Two roles that used to share one file, now two: `env` is the protocol
//! every probe must speak (env vars in, exit code out); `matching` is one
//! opinionated fuzzy-coordinate algorithm probes may use to answer it.
//! Flat re-exports keep `coord::report(...)`-style call sites unchanged.

pub mod env;
pub mod matching;

pub use env::*;
pub use matching::*;
