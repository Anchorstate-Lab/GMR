use std::path::Path;

use gmr_core::{Kind, ProbeName, ProbeRef, ProbeVersion};

use super::artifact::{Artifacts, publish};

/// Tests publish and install for real; otherwise "earned versions" would only
/// hold on the production path.
pub fn install_script(store: impl AsRef<Path>, name: &str, body: &str) -> ProbeRef {
    let name = ProbeName::new(name);
    let version = publish_script(store.as_ref(), body);
    Artifacts::new(store.as_ref())
        .install(&name, &version)
        .expect("cannot install");
    ProbeRef::new(Kind::new("shell"), name, serde_json::json!({}))
}

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
