use gmr::AnchorView;

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
