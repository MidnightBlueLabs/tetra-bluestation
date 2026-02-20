use crossbeam_channel::{Receiver, Sender, unbounded};
use std::{marker::PhantomData, thread};

use tetra_config::SharedConfig;
use tetra_core::{TdmaTime, tetra_common::Sap, tetra_entities::TetraEntity};
use tetra_saps::SapMsg;

use crate::{MessageQueue, TetraEntityTrait, network::transports::NetworkError};

/// Trait that all network entity workers must implement.
/// Workers run in a separate thread and handle blocking network operations.
pub trait NetEntityWorker: Send + 'static {
    /// The configuration/parameters type needed to create this worker
    type Transport: Send + 'static;

    /// Create a new worker instance
    fn new(
        entity_self: TetraEntity,
        entity_dest: TetraEntity,
        sap: Sap,
        w2e_sender: Sender<SapMsg>,
        e2w_receiver: Receiver<SapMsg>,
        transport: Self::Transport,
    ) -> Self;

    /// Run the worker's main loop (blocking)
    fn run(&mut self);
}

/// Universal network entity for external communications.
/// Generic over the worker type W that handles the actual network operations.
/// Serves as base for specialized network entities.
pub struct NetEntity<W: NetEntityWorker> {
    /// TETRA entity type this network entity represents
    entity_self: TetraEntity,
    /// Destination entity, the other entity on our SAP
    entity_dest: TetraEntity,
    /// The SAP we're listening on
    sap: Sap,

    /// Configuration
    config: SharedConfig,

    /// Sender to worker thread
    e2w_sender: Sender<SapMsg>,
    /// Receiver from worker thread
    w2e_receiver: Receiver<SapMsg>,

    /// Phantom data for the worker type
    _worker: PhantomData<W>,
}

impl<W: NetEntityWorker> NetEntity<W> {
    /// Create a new network entity with the specified worker type.
    ///
    /// # Arguments
    /// * `config` - Shared TETRA configuration
    /// * `entity_self` - The TETRA entity type this network entity represents
    /// * `entity_dest` - The destination entity on our SAP
    /// * `sap` - The SAP we're listening on
    /// * `worker_config` - Configuration specific to the worker type
    pub fn new(
        config: SharedConfig,
        entity_self: TetraEntity,
        entity_dest: TetraEntity,
        sap: Sap,
        worker_config: W::Transport,
    ) -> Result<Self, NetworkError> {
        // Create channels for worker communication
        let (e2w_sender, e2w_receiver) = unbounded::<SapMsg>();
        let (w2e_sender, w2e_receiver) = unbounded::<SapMsg>();

        // Spawn worker thread
        thread::Builder::new()
            .name(format!("net-worker-{:?}", sap).to_lowercase())
            .spawn(move || {
                let mut worker = W::new(entity_self, entity_dest, sap, w2e_sender, e2w_receiver, worker_config);

                // Run the worker loop
                worker.run();
            })
            .map_err(|e| NetworkError::ConnectionFailed(format!("Failed to spawn worker thread: {}", e)))?;

        Ok(Self {
            config,
            entity_self,
            entity_dest,
            sap,
            e2w_sender,
            w2e_receiver,
            _worker: PhantomData,
        })
    }

    /// Handle responses from the worker thread
    fn handle_worker_response(&mut self, response: SapMsg, queue: &mut MessageQueue) {
        queue.push_back(response);
    }

    /// Clean up expired pending requests
    fn cleanup_expired_requests(&mut self) {
        // let now = std::time::Instant::now();
        // let timeout = Duration::from_secs(300); // 5 minute timeout

        // self.pending_requests.retain(|_, request| {
        //     now.duration_since(request.timestamp) < timeout
        // });
        // tracing::warn!("TnmmNetEntity cleanup_expired_requests not yet implemented");
    }
}

impl<W: NetEntityWorker> TetraEntityTrait for NetEntity<W> {
    fn entity(&self) -> TetraEntity {
        self.entity_self
    }

    fn set_config(&mut self, config: SharedConfig) {
        self.config = config;
    }

    fn rx_prim(&mut self, _queue: &mut MessageQueue, message: SapMsg) {
        // Determine destination based on message content and routing logic
        // This should be implemented by specialized network entities
        tracing::debug!("NetEntity{:?} received SAP message: {:?}", self.sap, message);
        self.e2w_sender
            .send(message)
            .expect(format!("NetEntity{:?} failed to send message to worker", self.sap).as_str());
    }

    fn tick_start(&mut self, queue: &mut MessageQueue, _ts: TdmaTime) {
        // Process responses from worker thread (non-blocking)
        while let Ok(response) = self.w2e_receiver.try_recv() {
            self.handle_worker_response(response, queue);
        }

        // Clean up expired requests
        self.cleanup_expired_requests();
    }
}
