use std::collections::VecDeque;

use tetra_core::{Direction, TdmaTime, TimeslotAllocator, TimeslotOwner};
use tetra_pdus::cmce::structs::cmce_circuit::CmceCircuit;
use tetra_saps::{
    control::enums::{circuit_mode_type::CircuitModeType, communication_type::CommunicationType},
    lcmc::CallId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CircuitErr {
    NoCircuitFree,
    CircuitAlreadyInUse,
    CircuitNotActive,
}

pub enum CircuitMgrCmd {
    SendDSetup(CallId, u8, u8), // call id, usage number, timeslot
    SendClose(CallId, CmceCircuit),
}

pub struct CircuitMgr {
    pub dltime: TdmaTime,

    /// Holds any Dl and Dl+Ul circuits
    pub dl: [Option<CmceCircuit>; 4],
    /// Holds any Ul-only circuits, with no recipients on this cell
    pub ul_only: [Option<CmceCircuit>; 4],

    /// Data blocks queued to be transmitted, per timeslot
    pub tx_data: [VecDeque<Vec<u8>>; 4],

    /// 14-bit call identifier. Zero value is reserved.
    pub next_call_identifier: u16,
    /// 5-bit usage number. Values 0-3 are reserved.
    pub next_usage_number: u8,
}

impl CircuitMgr {
    pub fn new() -> Self {
        Self {
            dltime: TdmaTime::default(),
            dl: [None, None, None, None],
            ul_only: [None, None, None, None],
            tx_data: [VecDeque::new(), VecDeque::new(), VecDeque::new(), VecDeque::new()],
            next_call_identifier: 4,
            next_usage_number: 4,
        }
    }

    /// Checks if a circuit is active on the given timeslot
    /// Returns (dl_active, ul_active)
    pub fn is_active(&self, ts: u8) -> (bool, bool) {
        match &self.dl[ts as usize - 1] {
            Some(dl) => {
                if dl.direction == Direction::Both {
                    (true, true)
                } else {
                    (true, self.ul_only[ts as usize - 1].is_some())
                }
            }
            None => (false, self.ul_only[ts as usize - 1].is_some()),
        }
    }

    /// Checks if a circuit is active on the given timeslot and direction
    /// Direction must be Dl or Ul
    pub fn is_active_dir(&self, ts: u8, dir: Direction) -> bool {
        match dir {
            Direction::Dl => self.dl[ts as usize - 1].is_some(),
            Direction::Ul => {
                let dl_is_both = if let Some(dl) = &self.dl[ts as usize - 1] {
                    assert!(self.ul_only[ts as usize - 1].is_none());
                    dl.direction == Direction::Both
                } else {
                    false
                };
                self.ul_only[ts as usize - 1].is_some() || dl_is_both
            }

            _ => panic!("can only use with specific ul/dl direction"),
        }
    }

    /// Gets the usage number of an active circuit, (Option<dl_usage>, Option<ul_usage>)
    pub fn get_usage(&self, ts: u8) -> (Option<u8>, Option<u8>) {
        let (dl_usage, dl_is_both) = if let Some(dl) = &self.dl[ts as usize - 1] {
            (Some(dl.usage), dl.direction == Direction::Both)
        } else {
            (None, false)
        };
        let ul_usage = if dl_is_both {
            assert!(self.ul_only[ts as usize - 1].is_none());
            dl_usage
        } else if let Some(ul) = &self.ul_only[ts as usize - 1] {
            Some(ul.usage)
        } else {
            None
        };
        (dl_usage, ul_usage)
    }

    pub fn get_next_call_id(&mut self) -> CallId {
        let call_id = self.next_call_identifier;
        self.next_call_identifier += 1;
        if self.next_call_identifier > 0x3FF {
            self.next_call_identifier = 1; // Wrap around, skip reserved zero value
        }
        call_id
    }

    pub fn get_next_usage_number(&mut self) -> u8 {
        let usage = self.next_usage_number;
        self.next_usage_number += 1;
        if self.next_usage_number > 63 {
            self.next_usage_number = 4; // Wrap around, skip reserved values
        }
        usage
    }

    /// Finds a free timeslot for the given direction (Ul, Dl or Both)
    fn get_free_ts(&self, dir: Direction) -> Result<u8, CircuitErr> {
        // TODO FIXME we may do a bit smarter allocation here
        for ts in 2..=4 {
            let (dl_active, ul_active) = self.is_active(ts);
            match (dir, dl_active, ul_active) {
                (Direction::Dl, false, _) => return Ok(ts),
                (Direction::Ul, false, false) => return Ok(ts),
                (Direction::Ul, true, false) => {
                    // Check if dl circuit covers Dl+Ul
                    let dl = self.dl[ts as usize - 1].as_ref().unwrap();
                    if dl.direction != Direction::Both {
                        return Ok(ts);
                    }
                }
                (Direction::Both, false, false) => return Ok(ts),
                _ => {}
            }
        }
        Err(CircuitErr::NoCircuitFree)
    }

    pub fn allocate_circuit(&mut self, dir: Direction, comm_type: CommunicationType) -> Result<&CmceCircuit, CircuitErr> {
        // Get timeslot, call_id and usage
        let ts = self.get_free_ts(dir)?;
        let call_id = self.get_next_call_id();
        let usage = self.get_next_usage_number();

        // Create circuit
        let circuit = CmceCircuit {
            ts_created: self.dltime,
            direction: dir,
            ts,
            call_id,
            usage,
            circuit_mode: CircuitModeType::TchS, // TODO: only speech supported for now
            // endpoint_id: 0, // TODO, we don't use endpoints as of yet
            comm_type,
            simplex_duplex: false,   // TODO, simplex only for now
            speech_service: Some(0), // TODO, only TETRA encoded speech for now
            etee_encrypted: false,   // TODO, no encryption for now
        };

        // Register circuit and return
        Ok(self.open_circuit(dir, circuit)?)
    }

    /// Allocate circuit using centralized timeslot allocator
    pub fn allocate_circuit_with_allocator(
        &mut self,
        dir: Direction,
        comm_type: CommunicationType,
        timeslot_alloc: &mut TimeslotAllocator,
        owner: TimeslotOwner,
    ) -> Result<&CmceCircuit, CircuitErr> {
        // Get timeslot from centralized allocator
        let ts = timeslot_alloc.allocate_any(owner).ok_or(CircuitErr::NoCircuitFree)?;

        let call_id = self.get_next_call_id();
        let usage = self.get_next_usage_number();

        // Create circuit
        let circuit = CmceCircuit {
            ts_created: self.dltime,
            direction: dir,
            ts,
            call_id,
            usage,
            circuit_mode: CircuitModeType::TchS,
            comm_type,
            simplex_duplex: false,
            speech_service: Some(0),
            etee_encrypted: false,
        };

        // Register circuit and return
        Ok(self.open_circuit(dir, circuit)?)
    }

    /// Closes any active circuits for given timeslot and direction.
    /// Returns the CmceCircuit
    /// When direction is Both, closes both directions
    pub fn close_circuit(&mut self, dir: Direction, ts: u8) -> Result<CmceCircuit, CircuitErr> {
        match dir {
            Direction::Dl | Direction::Both => {
                self.tx_data[ts as usize - 1].clear();
                if dir == Direction::Both && self.ul_only[ts as usize - 1].is_some() {
                    tracing::warn!("Closing Dl+Ul circuit on ts {} while Ul-only circuit exists", ts);
                }
                let circuit = self.dl[ts as usize - 1].take();
                circuit.ok_or(CircuitErr::CircuitNotActive)
            }
            Direction::Ul => {
                let circuit = self.ul_only[ts as usize - 1].take();
                circuit.ok_or(CircuitErr::CircuitNotActive)
            }
            _ => panic!(),
        }
    }

    /// Creates a new circuit on the given direction and timeslot
    /// This channel should be free, if not, warnings will be issued and existing circuit will be closed first
    /// Consumes the circuit but returns a reference
    fn open_circuit(&mut self, dir: Direction, circuit: CmceCircuit) -> Result<&CmceCircuit, CircuitErr> {
        // Sanity check, close circuit and issue warning if exists
        let ts = circuit.ts;
        let (dl_active, ul_active) = self.is_active(ts);
        if dir.includes_dl() && dl_active {
            return Err(CircuitErr::CircuitAlreadyInUse);
        }
        if dir.includes_ul() && ul_active {
            return Err(CircuitErr::CircuitAlreadyInUse);
        }

        match dir {
            Direction::Dl | Direction::Both => {
                if !self.tx_data[ts as usize - 1].is_empty() {
                    tracing::warn!("CircuitMgr::create had pending tx_data on Dl {}", ts);
                    self.tx_data[ts as usize - 1].clear();
                }
                self.dl[ts as usize - 1] = Some(circuit);
                Ok(self.dl[ts as usize - 1].as_ref().unwrap())
            }
            Direction::Ul => {
                self.ul_only[ts as usize - 1] = Some(circuit);
                Ok(self.ul_only[ts as usize - 1].as_ref().unwrap())
            }
            _ => panic!(),
        }
    }

    /// Put a block in the queue for transmission on an associated channel
    pub fn put_block(&mut self, ts: u8, block: Vec<u8>) -> Result<(), CircuitErr> {
        if !self.is_active_dir(ts, Direction::Dl) {
            Err(CircuitErr::CircuitNotActive)
        } else {
            self.tx_data[ts as usize - 1].push_back(block);
            Ok(())
        }
    }

    /// Take a to-be-transmitted block from the queue
    pub fn take_block(&mut self, ts: u8) -> Result<Option<Vec<u8>>, CircuitErr> {
        if !self.is_active_dir(ts, Direction::Dl) {
            return Err(CircuitErr::CircuitNotActive);
        } else {
            Ok(self.tx_data[ts as usize - 1].pop_front())
        }
    }

    /// Closes any circuits that have expired.
    /// Safety timeout: 6 minutes (beyond the 5-minute call timeout T5m).
    /// Active calls are cleaned up earlier by CMCE hangtime/release logic.
    fn close_expired_circuits(&mut self, mut tasks: Option<Vec<CircuitMgrCmd>>) -> Option<Vec<CircuitMgrCmd>> {
        const CIRCUIT_EXPIRY_TIMESLOTS: i32 = 6 * 60 * 18 * 4; // 6 minutes

        let mut to_close: Vec<_> = self
            .dl
            .iter()
            .filter_map(|circuit| circuit.as_ref())
            .filter(|circuit| circuit.ts_created.age(self.dltime) > CIRCUIT_EXPIRY_TIMESLOTS)
            .map(|circuit| (circuit.direction, circuit.ts, circuit.call_id))
            .collect();
        to_close.extend(
            self.ul_only
                .iter()
                .filter_map(|circuit| circuit.as_ref())
                .filter(|circuit| circuit.ts_created.age(self.dltime) > CIRCUIT_EXPIRY_TIMESLOTS)
                .map(|circuit| (circuit.direction, circuit.ts, circuit.call_id)),
        );
        for (dir, ts, call_id) in to_close {
            let circuit = self.close_circuit(dir, ts).unwrap(); // TODO FIXME not so sure about this one
            tasks.get_or_insert_with(Vec::new).push(CircuitMgrCmd::SendClose(call_id, circuit));
        }
        tasks
    }

    pub fn tick_start(&mut self, dltime: TdmaTime) -> Option<Vec<CircuitMgrCmd>> {
        self.dltime = dltime;
        let mut tasks = None;

        if dltime.t == 1 {
            // First, close any expired circuits
            tasks = self.close_expired_circuits(tasks);

            // Next, go through channels, see if D-SETUPs need to be sent
            // Late entry: resend D-SETUP every 20 multiframes (5 seconds)
            const LATE_ENTRY_INTERVAL_TIMESLOTS: i32 = 20 * 18 * 4; // 20 MN = 1440 timeslots

            for circuit in self.dl.iter() {
                if let Some(circuit) = circuit {
                    let age = circuit.ts_created.age(dltime);

                    // Send D-SETUP for the initial frame + 1 backup frame after circuit creation.
                    // Matches ETSI Annex D Figure D.2: 1 initial + 1 back-up on MCCH.
                    if age < 1 * 4 {
                        tasks
                            .get_or_insert_with(Vec::new)
                            .push(CircuitMgrCmd::SendDSetup(circuit.call_id, circuit.usage, circuit.ts));
                    }
                    // Late entry: resend every 20 multiframes
                    else if age % LATE_ENTRY_INTERVAL_TIMESLOTS == 0 {
                        tasks
                            .get_or_insert_with(Vec::new)
                            .push(CircuitMgrCmd::SendDSetup(circuit.call_id, circuit.usage, circuit.ts));
                    }
                }
            }
            return tasks;
        }
        None
    }
}
