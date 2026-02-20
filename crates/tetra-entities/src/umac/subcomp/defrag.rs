use tetra_core::{BitBuffer, SsiType, TdmaTime, TetraAddress, Todo};

const DEFRAG_BUF_INITIAL_LEN: usize = 512;

#[derive(Debug, PartialEq)]
pub enum DefragBufferState {
    Inactive,
    Active,
    Complete,
}

pub struct DefragBuffer {
    pub state: DefragBufferState,
    pub addr: TetraAddress,
    pub t_first: TdmaTime,
    pub t_last: TdmaTime,
    pub num_frags: usize,
    pub aie_info: Option<Todo>,
    pub buffer: BitBuffer,
}

impl DefragBuffer {
    pub fn new() -> Self {
        Self {
            state: DefragBufferState::Inactive,
            addr: TetraAddress {
                ssi: 0,
                ssi_type: SsiType::Unknown,
                encrypted: false,
            },
            t_first: TdmaTime::default(),
            t_last: TdmaTime::default(),
            num_frags: 0,
            aie_info: None,
            buffer: BitBuffer::new_autoexpand(DEFRAG_BUF_INITIAL_LEN),
        }
    }

    pub fn reset(&mut self) {
        self.state = DefragBufferState::Inactive;
        self.addr = TetraAddress {
            ssi: 0,
            ssi_type: SsiType::Unknown,
            encrypted: false,
        };
        self.t_first = TdmaTime::default();
        self.t_last = TdmaTime::default();
        self.num_frags = 0;
        self.aie_info = None;
        self.buffer = BitBuffer::new_autoexpand(DEFRAG_BUF_INITIAL_LEN);
    }
}
