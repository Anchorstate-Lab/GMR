use gmr::{Before, Blind, Grounded, Grounding, Holding, Knowledge, Source, Warrant};
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

pub fn axes_line(state: &gmr::State) -> Option<String> {
    let v = state.as_value().get("v")?.as_object()?;
    Some(
        v.iter()
            .map(|(k, on)| format!("{k} {}", u8::from(on.as_bool().unwrap_or(false))))
            .collect::<Vec<_>>()
            .join("  "),
    )
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

    match axes_line(&v.state) {
        Some(axes) => out.push_str(&format!("  axes   {axes}\n")),
        None => {
            let now = v
                .state
                .as_value()
                .get("now")
                .map(|n| n.to_string())
                .unwrap_or_else(|| v.state.as_value().to_string());
            let short: String = now.chars().take(120).collect();
            let mark = match now.chars().count() > 120 {
                true => "…",
                false => "",
            };
            out.push_str(&format!("  now    {short}{mark}\n"));
        }
    }

    if let Some(f) = &v.faltering {
        out.push_str(&format!(
            "  ! {} consecutive failed attempts, latest {}: {}\n",
            f.attempts,
            blame(&f.reason),
            f.message
        ));
    }
    if matches!(v.sighting, gmr::Sighting::Absent) {
        out.push_str("  * last observation looked there and found nothing\n");
    }

    for m in &g.memories {
        let mark = if m.grounded { "*" } else { "?" };
        out.push_str(&format!("  {mark} {}", names.of(&m.reference)));
        out.push_str(&grounding(&m.grounding));
        out.push_str(&warranting(m.warrant.as_ref()));
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

fn blame(reason: &gmr::ReasonClass) -> &'static str {
    match reason {
        gmr::ReasonClass::Unreachable => "could not reach the world",
        gmr::ReasonClass::Unusable => "came back unusable",
        gmr::ReasonClass::Unevaluable => "could not be judged against the rules",
    }
}

fn warranting(w: Option<&Warrant>) -> String {
    let Some(w) = w else {
        return String::new();
    };
    [holding(&w.holding), knowledge(&w.knowledge)]
        .into_iter()
        .flatten()
        .map(|said| format!("  {said}"))
        .collect()
}

pub fn holding(h: &Holding) -> Option<String> {
    match h {
        Holding::Holds => None,
        Holding::Moved { axes, .. } => Some(format!(
            "the ground moved since this was bound: {}",
            axes.join(" · ")
        )),
        Holding::Incomparable { .. } => Some(
            "bound against a reading a different instrument took; whether the ground moved \
             cannot be told from here"
                .to_owned(),
        ),
        Holding::Absent => Some("the last look found nothing where this anchor points".to_owned()),
        Holding::NeverEstablished => {
            Some("bound before this anchor had read anything at all".to_owned())
        }
        Holding::Undated => {
            Some("this binding carries no date, so nothing can be compared against it".to_owned())
        }
    }
}

pub fn knowledge(k: &Knowledge) -> Option<String> {
    match k {
        Knowledge::Seen { verifiability, .. } if verifiability.is_closed() => None,
        Knowledge::Seen { verifiability, .. } => Some(format!(
            "read by a probe whose closure is open: {} can change the answer",
            verifiability
                .over()
                .iter()
                .map(|o| o.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
        Knowledge::Blind { why, .. } => Some(format!("unconfirmed: {}", unseen(why))),
    }
}

fn unseen(why: &Blind) -> &'static str {
    match why {
        Blind::NeverAsked => "nothing has looked yet, or the budget ran out before it could",
        Blind::Unreachable { .. } => "the last look could not reach the world — theirs to fix",
        Blind::Unusable { .. } => "the last look came back unusable — the probe's to fix",
        Blind::Unevaluable { .. } => "the last look could not be judged — the rules' to fix",
    }
}
