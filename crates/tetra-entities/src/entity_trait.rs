use crate::MessageQueue;
use as_any::AsAny;
use tetra_config::SharedConfig;
use tetra_core::{TdmaTime, tetra_entities::TetraEntity};
use tetra_saps::SapMsg;

/// Trait for TETRA entities
/// Used by MessageRouter for passing messages between entities
pub trait TetraEntityTrait: Send + AsAny {
    /// Returns the entity type identifier
    fn entity(&self) -> TetraEntity;

    /// Handle incoming SAP primitive
    fn rx_prim(&mut self, queue: &mut MessageQueue, message: SapMsg);

    /// Update configuration (optional)
    #[allow(dead_code)]
    fn set_config(&mut self, _config: SharedConfig) {}

    /// Called at the start of each TDMA tick
    fn tick_start(&mut self, _queue: &mut MessageQueue, _ts: TdmaTime) {}

    /// Called at the end of each TDMA tick
    fn tick_end(&mut self, _queue: &mut MessageQueue, _ts: TdmaTime) -> bool {
        false
    }
}
