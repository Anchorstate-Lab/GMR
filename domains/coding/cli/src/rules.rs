use std::collections::BTreeSet;

use gmr::{Expr, Kind, ProbeRef, ProbeVersion, Rule, StatusId, Transitions};

use crate::error::CliError;

/// 锚上写的探针：指向哪个 artifact，带什么参数。
pub fn probe(artifact: &str, params: &str) -> Result<ProbeRef, CliError> {
    let artifact = ProbeVersion::try_new(artifact).map_err(|e| {
        CliError(format!(
            "`{artifact}` 不是一个 artifact 版本号（{e}）——\n\
             用 `anchor publish <目录>` 发布一个，它会打印这个号"
        ))
    })?;
    let params: serde_json::Value =
        serde_json::from_str(params).map_err(|e| CliError(format!("params 不是合法 JSON：{e}")))?;
    Ok(ProbeRef::new(Kind::new("shell"), artifact, params))
}

pub fn rule(text: &str) -> Result<Rule, CliError> {
    let (when, to) = text.split_once("=>").ok_or_else(|| {
        CliError(format!(
            "转换规则要写成 `守卫 => 新状态`，收到 `{text}`\n\
             例：changed(\"shape\") => {{ shape: obs.shape, status: \"drifted\" }}"
        ))
    })?;
    let (when, to) = (when.trim(), to.trim());
    if when.is_empty() || to.is_empty() {
        return Err(CliError(format!("`{text}` 的守卫或新状态是空的")));
    }
    Ok(Rule {
        when: Expr::text(when),
        to: Expr::text(to),
    })
}

pub fn transitions(texts: &[String]) -> Result<Transitions, CliError> {
    texts
        .iter()
        .map(|t| rule(t))
        .collect::<Result<_, _>>()
        .map(Transitions)
}

pub fn terminal(names: &[String]) -> BTreeSet<StatusId> {
    names
        .iter()
        .map(|s| StatusId::new(s.trim()))
        .filter(|s| !s.as_str().is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rule_splits_on_the_arrow() {
        let r = rule("changed(\"shape\") => { status: \"drifted\" }").unwrap();
        assert_eq!(r.when.source.as_str().unwrap(), "changed(\"shape\")");
        assert_eq!(r.to.source.as_str().unwrap(), "{ status: \"drifted\" }");
    }

    #[test]
    fn a_rule_without_an_arrow_says_what_it_wanted() {
        let e = rule("changed(\"shape\")").unwrap_err();
        assert!(e.0.contains("守卫 => 新状态"));
    }

    #[test]
    fn the_arrow_inside_an_expression_still_splits_at_the_first_one() {
        assert!(rule("a => b => c").unwrap().to.source.as_str().unwrap() == "b => c");
    }
}
