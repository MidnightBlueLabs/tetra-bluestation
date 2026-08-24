//! Call signalling between CMCE and the network bridge (Brew).
//!
//! A signal names a call and what happened to it, nothing more. Everything else lives in the
//! global circuit view, which both sides read: [`BrewEvent`] carries what the backend did to a
//! call, [`CmceEvent`] what the cell did. Call parameters that exist before a circuit does are
//! deposited in the network call request store, keyed by the Brew session uuid.
//!
//! The call id is the reference throughout. A Brew session uuid appears only where no circuit
//! can exist yet: a call the backend announces, and the rejection that may answer it.

/// Brew to CMCE: what the backend did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrewEvent {
    /// A network speaker took the floor, under a fresh session. This starts a group call or
    /// changes its speaker, so there may be no circuit yet; the speaker and group are in the
    /// network call request store, keyed by this session.
    TxStart { brew_uuid: uuid::Uuid },
    /// The network transmission feeding this call ended.
    TxEnd { call_id: u16 },
    /// The backend offers a call to a local MS. No circuit exists yet; the call parameters are
    /// in the network call request store, keyed by this session.
    SetupRequest { brew_uuid: uuid::Uuid },
    /// The backend accepted the call the cell set up towards it.
    SetupAccept { call_id: u16 },
    /// The backend refused the call the cell set up towards it.
    SetupReject { call_id: u16, cause: u8 },
    /// The far party is ringing.
    Alert { call_id: u16 },
    /// The far party answered; its parameters are in the network call request store.
    ConnectRequest { call_id: u16 },
    /// The backend through-connected the call the cell answered.
    ConnectConfirm { call_id: u16 },
    /// The far party took the floor of a simplex call.
    SimplexGranted { call_id: u16 },
    /// The floor of a simplex call is free again.
    SimplexIdle { call_id: u16 },
    /// The backend cleared the call.
    Release { call_id: u16, cause: u8 },
}

impl BrewEvent {
    /// The call this event names, for the events that name one.
    pub fn get_call_id(&self) -> Option<u16> {
        match *self {
            BrewEvent::TxEnd { call_id }
            | BrewEvent::SetupAccept { call_id }
            | BrewEvent::SetupReject { call_id, .. }
            | BrewEvent::Alert { call_id }
            | BrewEvent::ConnectRequest { call_id }
            | BrewEvent::ConnectConfirm { call_id }
            | BrewEvent::SimplexGranted { call_id }
            | BrewEvent::SimplexIdle { call_id }
            | BrewEvent::Release { call_id, .. } => Some(call_id),
            BrewEvent::TxStart { .. } | BrewEvent::SetupRequest { .. } => None,
        }
    }

    /// The Brew session this event names, for the events that predate the circuit.
    pub fn get_brew_uuid(&self) -> Option<uuid::Uuid> {
        match *self {
            BrewEvent::TxStart { brew_uuid } | BrewEvent::SetupRequest { brew_uuid } => Some(brew_uuid),
            _ => None,
        }
    }
}

/// CMCE to Brew: what the cell did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmceEvent {
    /// A local party took the floor: open, or reuse, an upstream session to carry it.
    TxStart { call_id: u16 },
    /// The transmission carried upstream ended. The call may already be torn down, in which
    /// case its session waits in the closing session store.
    TxEnd { call_id: u16 },
    /// A local MS calls a party the backend has to reach; the call parameters are in the
    /// network call request store.
    SetupRequest { call_id: u16 },
    /// The call the backend offered is being delivered to a local MS.
    SetupAccept { call_id: u16 },
    /// The call the backend offered cannot be delivered. No circuit was ever created for it,
    /// so it is answered by session.
    SetupReject { brew_uuid: uuid::Uuid, cause: u8 },
    /// The called MS is ringing.
    Alert { call_id: u16 },
    /// The called MS answered; its parameters are in the network call request store.
    ConnectRequest { call_id: u16 },
    /// The call the backend requested is through-connected.
    ConnectConfirm { call_id: u16 },
    /// The call is cleared towards the backend.
    Release { call_id: u16, cause: u8 },
}

impl CmceEvent {
    /// The call this event names, for the events that name one.
    pub fn get_call_id(&self) -> Option<u16> {
        match *self {
            CmceEvent::TxStart { call_id }
            | CmceEvent::TxEnd { call_id }
            | CmceEvent::SetupRequest { call_id }
            | CmceEvent::SetupAccept { call_id }
            | CmceEvent::Alert { call_id }
            | CmceEvent::ConnectRequest { call_id }
            | CmceEvent::ConnectConfirm { call_id }
            | CmceEvent::Release { call_id, .. } => Some(call_id),
            CmceEvent::SetupReject { .. } => None,
        }
    }

    /// The Brew session this event names, for the events that have no circuit behind them.
    pub fn get_brew_uuid(&self) -> Option<uuid::Uuid> {
        match *self {
            CmceEvent::SetupReject { brew_uuid, .. } => Some(brew_uuid),
            _ => None,
        }
    }
}
