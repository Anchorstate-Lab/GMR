use std::sync::Arc;

use gmr_budget::Budget;
use gmr_core::{Anchor, Derivation, Outcome, ProbeRef};
use gmr_probe::{ProbeCall, ProbeError, ProbeErrorCode, Transport};

pub struct Observer {
    transports: Vec<Arc<dyn Transport>>,
}

impl Observer {
    pub(crate) fn new(transports: Vec<Arc<dyn Transport>>) -> Self {
        Self { transports }
    }

    fn transport(&self, probe: &ProbeRef) -> Result<&dyn Transport, ProbeError> {
        self.transports
            .iter()
            .find(|t| t.kind() == &probe.kind)
            .map(Arc::as_ref)
            .ok_or_else(|| {
                ProbeError::unreachable(format!("no transport recognises a `{}` probe", probe.kind))
            })
    }

    pub(crate) fn resolve(&self, probe: &ProbeRef) -> Result<Derivation, ProbeError> {
        let transport = self.transport(probe)?;
        transport.resolve(&probe.name).ok_or_else(|| {
            ProbeError::with_code(
                gmr_core::ReasonClass::Unusable,
                ProbeErrorCode::ArtifactInvalid,
                format!(
                    "no `{}` probe named `{}` is available here",
                    probe.kind, probe.name
                ),
            )
        })
    }

    pub(crate) async fn invoke(
        &self,
        anchor: &Anchor,
        position: &serde_json::Value,
        budget: &Budget,
    ) -> Result<Outcome, ProbeError> {
        self.sample(&anchor.probe, position, budget).await
    }

    pub(crate) async fn sample(
        &self,
        probe: &ProbeRef,
        position: &serde_json::Value,
        budget: &Budget,
    ) -> Result<Outcome, ProbeError> {
        self.transport(probe)?
            .invoke(&ProbeCall {
                probe,
                position,
                budget,
            })
            .await
    }
}
