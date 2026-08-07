#[derive(Debug)]
pub struct CliError(pub String);

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

macro_rules! from_display {
    ($($t:ty),* $(,)?) => {
        $(impl From<$t> for CliError {
            fn from(e: $t) -> Self { CliError(e.to_string()) }
        })*
    };
}

from_display!(
    gmr::RuntimeError,
    gmr::StoreError,
    gmr::CanonicalizeError,
    serde_json::Error,
    toml::de::Error,
    std::io::Error,
);
