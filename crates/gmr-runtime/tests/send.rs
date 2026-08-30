use std::time::Duration;

use gmr_core::{AnchorKey, Ref, Source};
use gmr_runtime::{Instructions, Runtime};

fn moved_between_threads<F: Send>(_: F) {}

#[test]
fn every_verb_a_host_can_call_is_a_future_a_host_can_spawn() {
    fn over(rt: &Runtime) {
        let refs = vec![gmr_core::Claim::from(Ref::new("git", "memories/x.md"))];
        let how = Instructions::fresher_than(Duration::from_secs(60));
        let key = AnchorKey::new("a");

        moved_between_threads(rt.ground(&refs, &how));
        moved_between_threads(rt.sample(&key, &how));
        moved_between_threads(rt.changed_since(0, None));
        moved_between_threads(rt.bind(
            gmr_core::Binding::on(refs[0].clone(), vec![key.clone()]),
            None,
            None,
            Source::Derived,
        ));
        moved_between_threads(rt.revoke(&refs[0], Source::Derived));
        moved_between_threads(rt.close(&key, b"why"));
    }
    let _ = over;
}
