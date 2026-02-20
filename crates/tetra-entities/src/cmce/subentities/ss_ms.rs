use tetra_saps::SapMsg;

use crate::MessageQueue;

/// Clause 12 Supplementary Services CMCE sub-entity
pub struct SsMsSubentity {}

impl SsMsSubentity {
    pub fn new() -> Self {
        SsMsSubentity {}
    }

    pub fn route_re_deliver(&mut self, _queue: &mut MessageQueue, mut _message: SapMsg) {
        tracing::trace!("route_re_deliver");

        // Handle the incoming unit data indication
        unimplemented!();
    }
}
