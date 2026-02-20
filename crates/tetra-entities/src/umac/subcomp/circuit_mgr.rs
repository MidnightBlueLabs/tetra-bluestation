use std::collections::VecDeque;

use tetra_core::Direction;
use tetra_saps::control::call_control::Circuit;

pub struct CircuitMgr {
    pub dl: [Option<Circuit>; 4],
    pub ul: [Option<Circuit>; 4],

    /// Data blocks queued to be transmitted, per timeslot
    pub tx_data: [VecDeque<Vec<u8>>; 4],
}

impl CircuitMgr {
    pub fn new() -> Self {
        Self {
            dl: [None, None, None, None],
            ul: [None, None, None, None],
            tx_data: [VecDeque::new(), VecDeque::new(), VecDeque::new(), VecDeque::new()],
        }
    }

    pub fn is_active(&self, dir: Direction, ts: u8) -> bool {
        match dir {
            Direction::Dl => self.dl[ts as usize - 1].is_some(),
            Direction::Ul => self.ul[ts as usize - 1].is_some(),
            _ => panic!("can only use with specific ul/dl direction"),
        }
    }

    pub fn get_usage(&self, dir: Direction, ts: u8) -> Option<u8> {
        match dir {
            Direction::Dl => {
                if let Some(circuit) = &self.dl[ts as usize - 1] {
                    Some(circuit.usage)
                } else {
                    None
                }
            }
            Direction::Ul => {
                if let Some(circuit) = &self.ul[ts as usize - 1] {
                    Some(circuit.usage)
                } else {
                    None
                }
            }
            _ => panic!("can only use with specific ul/dl direction"),
        }
    }

    /// Closes an active circuit, and return the Circuit to the caller
    pub fn close_circuit(&mut self, dir: Direction, ts: u8) -> Option<Circuit> {
        match dir {
            Direction::Dl => {
                self.tx_data[ts as usize - 1].clear();
                self.dl[ts as usize - 1].take()
            }
            Direction::Ul => self.ul[ts as usize - 1].take(),
            _ => panic!("can only use with specific ul/dl direction"),
        }
    }

    /// Creates a new circuit on the given direction and timeslot
    /// This channel should be free, if not, warnings will be issued and the existing circuit will be closed first
    pub fn create_circuit(&mut self, dir: Direction, circuit: Circuit) {
        let ts = circuit.ts;

        // Sanity check
        if self.is_active(dir, ts) {
            tracing::warn!("CircuitMgr::create had still active circuit on {:?} {}", dir, ts);
            self.close_circuit(dir, ts);
        }

        match dir {
            Direction::Dl => {
                if !self.tx_data[ts as usize - 1].is_empty() {
                    tracing::warn!("CircuitMgr::create had pending tx_data on Dl {}", ts);
                    self.tx_data[ts as usize - 1].clear();
                }
                self.dl[ts as usize - 1] = Some(circuit);
            }
            Direction::Ul => self.ul[ts as usize - 1] = Some(circuit),
            _ => panic!("can only use with specific ul/dl direction"),
        }
    }

    /// Put a block in the queue for transmission on an associated channel
    pub fn put_block(&mut self, ts: u8, block: Vec<u8>) {
        if !self.is_active(Direction::Dl, ts) {
            tracing::warn!("CircuitMgr::put_block on inactive circuit {:?} {}", Direction::Dl, ts);
            return;
        }
        self.tx_data[ts as usize - 1].push_back(block);
    }

    /// Take a to-be-transmitted block from the queue
    pub fn take_block(&mut self, ts: u8) -> Option<Vec<u8>> {
        if !self.is_active(Direction::Dl, ts) {
            tracing::warn!("CircuitMgr::take_block on inactive circuit {:?} {}", Direction::Dl, ts);
            return None;
        }
        self.tx_data[ts as usize - 1].pop_front()
    }
}
