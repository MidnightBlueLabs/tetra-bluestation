use crate::{MessageQueue, TetraEntityTrait};
use tetra_config::SharedConfig;
use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{Sap, unimplemented_log};
use tetra_saps::SapMsg;

pub struct Sndcp {
    // config: Option<SharedConfig>,
    config: SharedConfig,
}

impl Sndcp {
    pub fn new(config: SharedConfig) -> Self {
        Self { config }
    }
}

impl TetraEntityTrait for Sndcp {
    fn entity(&self) -> TetraEntity {
        TetraEntity::Sndcp
    }

    fn rx_prim(&mut self, _queue: &mut MessageQueue, message: SapMsg) {
        tracing::debug!("rx_prim: {:?}", message);
        // tracing::debug!(ts=%message.dltime, "rx_prim: {:?}", message);

        // There is only one SAP for SNDCP
        // OR.. SN-SAP? TODO FIXME check docs
        assert!(message.sap == Sap::TlpdSap);
        unimplemented_log!("sndcp not implemented");
    }
}
