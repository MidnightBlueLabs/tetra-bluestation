use std::{cell::RefCell, rc::Rc};

pub use circuits::*;
pub use global_state::GlobalState;
pub use subscribers::{Subscriber, SubscriberStore};

pub mod circuits;
pub mod global_state;
pub mod subscribers;

#[derive(Debug, Clone)]
pub struct InternalStateInner {
    /// Assorted stack state information such as LST bit
    pub global_state: Option<GlobalState>,

    /// Circuits and calls
    pub circuits: Option<CircuitStore>,

    /// Subscribers and attached groups
    pub subscribers: Option<SubscriberStore>,
}

impl Default for InternalStateInner {
    fn default() -> Self {
        Self {
            global_state: Some(GlobalState::default()),
            circuits: Some(CircuitStore::default()),
            subscribers: Some(SubscriberStore::default()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct InternalState {
    shared: Rc<RefCell<InternalStateInner>>,
}

impl InternalState {
    pub fn from_initial_state(initial: InternalStateInner) -> InternalState {
        let shared = Rc::new(RefCell::new(initial));
        InternalState { shared }
    }

    pub fn new() -> InternalState {
        let initial = InternalStateInner::default();
        Self::from_initial_state(initial)
    }

    pub fn from_config(config: SharedConfig) -> InternalState {
        // Make default state
        let ret = Self::new();

        // Populate LST from config if brew is disabled
        if config.config().brew.is_none() {
            ret.with_global_state(|s| s.network_connected = config.config().cell.system_wide_services);
        }

        ret
    }

    // pub fn update<F>(&self, f: F)
    // where
    //     F: FnOnce(&mut InternalStateInner),
    // {
    //     let mut state = self.shared.borrow_mut();
    //     f(&mut state);
    // }

    pub fn read<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&InternalStateInner) -> R,
    {
        let state = self.shared.borrow();
        f(&state)
    }

    pub fn take_global_state(&self) -> GlobalState {
        let mut state = self.shared.borrow_mut();
        match state.global_state.take() {
            None => panic!("GlobalState could not be taken - someone else didn't put it back?"),
            Some(x) => x,
        }
    }

    pub fn put_global_state(&self, global_state: GlobalState) {
        let mut state = self.shared.borrow_mut();
        match state.global_state.replace(global_state) {
            Some(_) => panic!("GlobalState already there upon put_circuits"),
            None => {}
        }
    }

    pub fn with_global_state<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&mut GlobalState) -> R,
    {
        let mut global_state = self.take_global_state();
        let out = f(&mut global_state);
        self.put_global_state(global_state);
        out
    }

    pub fn take_circuits(&self) -> CircuitStore {
        let mut state = self.shared.borrow_mut();
        match state.circuits.take() {
            None => panic!("CircuitStore could not be taken - someone else didn't put it back?"),
            Some(x) => x,
        }
    }

    pub fn put_circuits(&self, circuits: CircuitStore) {
        let mut state = self.shared.borrow_mut();
        match state.circuits.replace(circuits) {
            Some(_) => panic!("CircuitStore already there upon put_circuits"),
            None => {}
        }
    }

    pub fn with_circuits<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&mut CircuitStore) -> R,
    {
        let mut circuits = self.take_circuits();
        let out = f(&mut circuits);
        self.put_circuits(circuits);
        out
    }

    pub fn take_subscribers(&self) -> SubscriberStore {
        let mut state = self.shared.borrow_mut();
        match state.subscribers.take() {
            None => panic!("SubscriberStore could not be taken - someone else didn't put it back?"),
            Some(x) => x,
        }
    }

    pub fn put_subscribers(&self, subscribers: SubscriberStore) {
        let mut state = self.shared.borrow_mut();
        match state.subscribers.replace(subscribers) {
            Some(_) => panic!("SubscriberStore already there upon put_circuits"),
            None => {}
        }
    }

    pub fn with_subscribers<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&mut SubscriberStore) -> R,
    {
        let mut subscribers = self.take_subscribers();
        let out = f(&mut subscribers);
        self.put_subscribers(subscribers);
        out
    }
}

// TODO FIXME cleanup below remainders of state

use tetra_core::TimeslotAllocator;

use crate::bluestation::SharedConfig;

/// Mutable, stack-editable state (mutex-protected).
#[derive(Debug, Clone)]
pub struct StackState {
    pub timeslot_alloc: TimeslotAllocator,
    // Backhaul/network connection to SwMI (e.g., Brew/TetraPack). False -> fallback mode.
}

impl Default for StackState {
    fn default() -> Self {
        Self {
            timeslot_alloc: TimeslotAllocator::default(),
            // network_connected: false,
        }
    }
}
