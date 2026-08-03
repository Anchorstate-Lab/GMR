use std::sync::Arc;

use gmr_core::Anchor;
use gmr_probe::{ProbeError, Sighted, Transport};

/// The transports this deployment wired up. No journal, no bindings, no queue.
pub struct Observer {
    transports: Vec<Arc<dyn Transport>>,
}

impl Observer {
    pub(crate) fn new(transports: Vec<Arc<dyn Transport>>) -> Self {
        Self { transports }
    }

    pub(crate) async fn invoke(
        &self,
        anchor: &Anchor,
        position: &serde_json::Value,
    ) -> Result<Sighted, ProbeError> {
        let transport = self
            .transports
            .iter()
            .find(|t| t.kind() == &anchor.probe.kind)
            .ok_or_else(|| {
                ProbeError::unreachable(format!(
                    "no transport recognises a `{}` probe",
                    anchor.probe.kind
                ))
            })?;
        transport.invoke(&anchor.probe, position).await
    }
}
