use crate::MessageQueue;
use tetra_saps::SapMsg;

/// Clause 12 Supplementary Services CMCE sub-entity
pub struct SsBsSubentity {}

impl SsBsSubentity {
    pub fn new() -> Self {
        SsBsSubentity {}
    }

    pub fn route_re_deliver(&mut self, _queue: &mut MessageQueue, mut _message: SapMsg) {
        tracing::trace!("route_re_deliver");

        // Handle the incoming unit data indication
        unimplemented!();
    }
}
