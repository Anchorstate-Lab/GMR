use std::path::Path;

use gmr_core::{Kind, ProbeVersion};

use crate::artifact::{Artifacts, publish};

/// Publish a shell script as a probe artifact for tests.
///
/// Tests must go through real publishing too; otherwise "earned versions" only
/// hold on the production path.
pub fn publish_script(store: impl AsRef<Path>, body: &str) -> ProbeVersion {
    let staging = tempfile::tempdir().expect("cannot create staging directory");
    let entry = staging.path().join("probe");
    std::fs::write(&entry, format!("#!/bin/sh\n{body}\n")).expect("cannot write script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&entry, std::fs::Permissions::from_mode(0o755))
            .expect("cannot set executable bit");
    }
    publish(
        &Artifacts::new(store.as_ref()),
        staging.path(),
        Kind::new("shell"),
        "probe",
        Vec::new(),
        Default::default(),
    )
    .expect("cannot publish")
}
