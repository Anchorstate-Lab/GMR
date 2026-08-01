use std::path::Path;

use gmr_core::{Kind, ProbeVersion};

use crate::artifact::{Artifacts, publish};

/// 把一段 shell 脚本发布成探针 artifact，给测试用。
///
/// 测试也得走真的发布 —— 否则「版本是挣来的」这条只在生产路径上成立。
pub fn publish_script(store: impl AsRef<Path>, body: &str) -> ProbeVersion {
    let staging = tempfile::tempdir().expect("建不了暂存目录");
    let entry = staging.path().join("probe");
    std::fs::write(&entry, format!("#!/bin/sh\n{body}\n")).expect("写不了脚本");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&entry, std::fs::Permissions::from_mode(0o755))
            .expect("设不了可执行位");
    }
    publish(
        &Artifacts::new(store.as_ref()),
        staging.path(),
        Kind::new("shell"),
        "probe",
        Vec::new(),
        Default::default(),
    )
    .expect("发布不了")
}
