include!(concat!(env!("OUT_DIR"), "/version.rs"));

#[cfg(test)]
mod tests {
    #[test]
    fn the_evaluator_version_is_earned_not_declared() {
        let v = super::EVALUATOR_VERSION;
        assert_eq!(v.len(), 64, "it should be a content hash, got `{v}`");
        assert!(
            v.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );
    }
}
