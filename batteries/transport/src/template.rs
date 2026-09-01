use gmr_core::ReasonClass;
use gmr_probe::{ProbeError, ProbeErrorCode};
use serde_json::Value;

pub fn url(template: &str, position: &Value) -> Result<String, ProbeError> {
    fill(template, position, encoded)
}

pub fn path(template: &str, position: &Value) -> Result<String, ProbeError> {
    fill(template, position, verbatim)
}

fn fill(
    template: &str,
    position: &Value,
    escape: fn(&str) -> String,
) -> Result<String, ProbeError> {
    if !template.contains('{') {
        return Ok(template.to_owned());
    }
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let close = after.find('}').ok_or_else(|| {
            invalid(format!(
                "`{template}` opens a name it never closes. A `{{` here means \"put the \
                 position's field of this name in\", so an unclosed one is a declaration \
                 nobody can read the same way twice"
            ))
        })?;
        out.push_str(&escape(&held(&after[..close], position, template)?));
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

fn held(name: &str, position: &Value, template: &str) -> Result<String, ProbeError> {
    let found = position.get(name).ok_or_else(|| {
        invalid(format!(
            "`{template}` is pointed by `{name}`, and the position it was called with does \
             not carry one: {position}. A template is how a probe says which part of a \
             coordinate it observes; a name the position cannot fill means the declaration \
             and the anchor disagree about what is being watched"
        ))
    })?;
    match found {
        Value::String(s) => Ok(s.clone()),
        Value::Number(n) => Ok(n.to_string()),
        Value::Bool(b) => Ok(b.to_string()),
        _ => Err(invalid(format!(
            "`{template}` is pointed by `{name}`, and the position holds {found} there. A \
             position names one place; a list or an object is several, and picking one of \
             them here would be this transport inventing which"
        ))),
    }
}

fn encoded(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for byte in raw.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

fn verbatim(raw: &str) -> String {
    raw.to_owned()
}

fn invalid(message: String) -> ProbeError {
    ProbeError::with_code(
        ReasonClass::Unusable,
        ProbeErrorCode::ArtifactInvalid,
        message,
    )
}
