use gmr_core::ReasonClass;
use gmr_probe::{ProbeError, ProbeErrorCode};

pub fn without_credentials(url: &str) -> Result<(), ProbeError> {
    let authority = match url.split_once("://") {
        Some((_, rest)) => rest.split(['/', '?', '#']).next().unwrap_or(rest),
        None => return Ok(()),
    };
    if !authority.contains('@') {
        return Ok(());
    }
    Err(ProbeError::with_code(
        ReasonClass::Unusable,
        ProbeErrorCode::ArtifactInvalid,
        "this url carries a credential before its host. A declaration is written down, \
         committed, copied between machines and sent over wires, and every failure it \
         produces is a line in an append-only log that nothing can edit afterwards; a \
         password put here is a password in all of those. Name an environment variable \
         instead -- the declaration then carries the name, and the value is read at the \
         moment of the call and never stored"
            .to_owned(),
    ))
}
