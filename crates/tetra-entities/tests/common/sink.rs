use tetra_core::tetra_entities::TetraEntity;
use tetra_entities::{MessageQueue, TetraEntityTrait};
use tetra_saps::sapmsg::SapMsg;

/// A TETRA component sink for testing purposes
/// Collects all received SapMsg messages for later inspection
pub struct Sink {
    component: TetraEntity,
    msgqueue: Vec<SapMsg>,
}

impl Sink {
    pub fn new(component: TetraEntity) -> Self {
        Self {
            component,
            msgqueue: vec![],
        }
    }

    pub fn take_msgqueue(&mut self) -> Vec<SapMsg> {
        std::mem::take(&mut self.msgqueue)
    }
}

impl TetraEntityTrait for Sink {
    fn entity(&self) -> TetraEntity {
        self.component
    }

    fn rx_prim(&mut self, _queue: &mut MessageQueue, message: SapMsg) {
        tracing::debug!("rx_prim: {:?}", message);
        // tracing::debug!(ts=%message.dltime, "rx_prim: {:?}", message);

        self.msgqueue.push(message);
    }
}
