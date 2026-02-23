use std::collections::HashMap;

use tetra_core::{EndpointId, LinkId, MleHandle, TdmaTime, TetraAddress};

pub struct MleConnState {
    handle: MleHandle,
    addr: TetraAddress,
    link_id: LinkId,
    endpoint_id: EndpointId,

    ts_created: TdmaTime,
    ts_last_used: TdmaTime,
}

pub struct MleRouter {
    states: HashMap<u32, MleConnState>,
    next_handle: u32,
}

impl MleRouter {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
            next_handle: 1,
        }
    }

    pub fn create_handle(&mut self, _addr: TetraAddress, _link_id: LinkId, _endpoint_id: EndpointId, _ts: TdmaTime) -> u32 {
        let handle = self.next_handle;
        // TODO FIXME: Re-enable handle insertion once we start using this
        // let conn = MleConnState {
        //     handle,
        //     addr,
        //     link_id,
        //     endpoint_id,
        //     ts_created: ts,
        //     ts_last_used: ts,
        // };
        // self.states.insert(handle, conn);
        self.next_handle += 1;
        handle
    }

    /// Resolve a handle, returning its associated address, link ID and endpoint ID
    /// Internally, the router updates the element's last_used timestamp.
    pub fn use_handle(&mut self, handle: u32, ts: TdmaTime) -> (TetraAddress, u32, u32) {
        if let Some(conn) = self.states.get_mut(&handle) {
            conn.ts_last_used = ts; // Update last used timestamp
            (conn.addr.clone(), conn.link_id, conn.endpoint_id)
        } else {
            // self.dump_mappings();
            tracing::warn!("Unknown MLE handle: {}", handle);
            (TetraAddress::issi(0), 0, 0)
        }
    }

    pub fn delete_handle(&mut self, handle: u32) -> Option<MleConnState> {
        self.states.remove(&handle)
    }

    pub fn dump_mappings(&self) {
        tracing::info!("MLE Router mappings:");
        for (handle, conn) in &self.states {
            tracing::info!(
                "Handle {} -> Addr: {}, Link ID: {}, Endpoint ID: {}, Created: {}, Last Used: {}",
                handle,
                conn.addr,
                conn.link_id,
                conn.endpoint_id,
                conn.ts_created,
                conn.ts_last_used
            );
        }
    }
}
