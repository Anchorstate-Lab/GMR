use gmr::AnchorView;

pub fn anchor(v: &AnchorView) -> String {
    let mut out = String::new();
    let head = match v.status.as_ref() {
        Some(s) => format!("{}  [{s}]", v.key),
        None => format!("{}", v.key),
    };
    out.push_str(&head);
    if v.closed {
        out.push_str("  已终结");
    }
    out.push('\n');

    out.push_str(&format!("  状态  {}\n", v.state.as_value()));

    if v.attempts > 0 {
        out.push_str(&format!("  ! 连续 {} 次没看成\n", v.attempts));
    }
    if matches!(v.sighting, gmr::Sighting::Absent) {
        out.push_str("  · 上次去看了，那儿什么都没有\n");
    }

    for m in &v.memories {
        let mark = if m.grounded { "·" } else { "?" };
        out.push_str(&format!("  {mark} {}", m.reference.external_id));
        if m.rewritten {
            out.push_str("  （绑定后被改写过）");
            if m.retrievable == Some(false) {
                out.push_str(" 当初那一版已取不回");
            }
        }
        if !m.grounded {
            out.push_str("  未接地");
        }
        if let Some(why) = &m.unavailable {
            out.push_str(&format!("  取不到：{why}"));
        }
        out.push('\n');
    }
    out
}
