use gmr::{Before, Grounded, Grounding, Source};
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

pub fn anchor(g: &Grounded, names: &crate::memories::Names) -> String {
    let v = &g.view;
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

    if let Some(f) = &v.faltering {
        out.push_str(&format!("  ! {} consecutive failed attempts\n", f.attempts));
    }
    if matches!(v.sighting, gmr::Sighting::Absent) {
        out.push_str("  * last observation looked there and found nothing\n");
    }

    for m in &g.memories {
        let mark = if m.grounded { "*" } else { "?" };
        out.push_str(&format!("  {mark} {}", names.of(&m.reference)));
        out.push_str(&grounding(&m.grounding));
        if let Some(said) = vouching(&m.sources) {
            out.push_str(said);
        }
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

fn vouching(sources: &std::collections::BTreeSet<Source>) -> Option<&'static str> {
    if sources.iter().any(|s| s.independent()) {
        return None;
    }
    match sources.iter().all(|s| *s == Source::Unknown) {
        true => Some("  where this link came from was never recorded"),
        false => Some("  only the writer of this record says it is about this anchor"),
    }
}

fn grounding(g: &Grounding) -> String {
    match g {
        Grounding::Current { .. } => String::new(),
        Grounding::Unverified { .. } => {
            "  never verified: nothing has yet compared this record against what the store \
             holds"
                .to_owned()
        }
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
