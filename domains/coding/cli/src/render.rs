use gmr::AnchorView;
use serde_json::Value;

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
        out.push_str(&format!("  {mark} {}", m.reference.external_id));
        if m.rewritten {
            out.push_str("  (rewritten since binding)");
            if m.retrievable == Some(false) {
                out.push_str(" bound version is no longer retrievable");
            }
        }
        if !m.grounded {
            out.push_str("  ungrounded");
        }
        if let Some(why) = &m.unavailable {
            out.push_str(&format!("  unavailable: {why}"));
        }
        out.push('\n');
    }
    out
}
