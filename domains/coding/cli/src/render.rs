use gmr::{AnchorView, Before, Grounding, Ref};
use serde_json::Value;

pub fn shown(reference: &Ref) -> String {
    reference.external_id.to_string()
}

pub fn diagnosis(facts: Option<&gmr::Facts>) -> Option<String> {
    let facts = facts?.as_value();
    if facts.get("schema")?.as_str()? != crate::contract::COORD_SCHEMA {
        return None;
    }
    if facts.get("exact")?.as_bool()? {
        return None;
    }
    let list = |key: &str| {
        facts
            .get(key)
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(" · ")
            })
            .unwrap_or_default()
    };
    Some(match facts.get("found").and_then(Value::as_bool) {
        Some(true) => format!(
            "{} matched, {} did not — this reading is about whichever of {} others was closest",
            list("matched"),
            list("missed"),
            facts.get("candidates").and_then(Value::as_u64).unwrap_or(0)
        ),
        _ => format!("nothing there answered to any of {}", list("priority")),
    })
}

pub fn anchor(v: &AnchorView) -> String {
    let mut out = String::new();
    let head = match v.status.as_ref() {
        Some(s) => format!("{}  [{s}]", v.key),
        None => format!("{}", v.key),
    };
    out.push_str(&head);
    if v.closed {
        out.push_str("  closed");
    }
    out.push('\n');

    out.push_str(&format!("  state  {}\n", v.state.as_value()));

    if v.attempts > 0 {
        out.push_str(&format!("  ! {} consecutive failed attempts\n", v.attempts));
    }
    if matches!(v.sighting, gmr::Sighting::Absent) {
        out.push_str("  * last observation looked there and found nothing\n");
    }

    for m in &v.memories {
        let mark = if m.grounded { "*" } else { "?" };
        out.push_str(&format!("  {mark} {}", shown(&m.reference)));
        out.push_str(&grounding(&m.grounding));
        if !m.grounded {
            out.push_str("  ungrounded");
        }
        if m.content().is_some_and(|b| std::str::from_utf8(b).is_err()) {
            out.push_str("  (not text; shown with replacement characters)");
        }
        out.push('\n');
    }
    out
}

fn grounding(g: &Grounding) -> String {
    match g {
        Grounding::Current { .. } => String::new(),
        Grounding::Rewritten { before, .. } => {
            format!("  (rewritten since binding){}", was(before))
        }
        Grounding::Gone => "  the provider says this record is gone".to_owned(),
        Grounding::NoProvider { provider } => {
            format!("  no provider named `{provider}` is registered in this binary")
        }
        Grounding::Unreachable { why, .. } => format!("  could not be reached: {why}"),
    }
}

fn was(before: &Before) -> &'static str {
    match before {
        Before::Retrieved { .. } => "",
        Before::NotRetained => " the bound version was not kept",
        Before::NoHistory => " this provider keeps no history, so there is nothing to diff against",
        Before::Unreachable { .. } => " and the bound version could not be reached",
    }
}
