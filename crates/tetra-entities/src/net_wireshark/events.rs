use tetra_core::{BitBuffer, PhyBlockNum, TdmaTime};
use tetra_saps::tmv::enums::logical_chans::LogicalChannel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type1Direction {
    Downlink = 0,
    Uplink = 1,
}

#[derive(Debug, Clone)]
pub struct Type1Capture {
    pub direction: Type1Direction,
    pub logical_channel: LogicalChannel,
    pub block_num: PhyBlockNum,
    pub crc_pass: bool,
    pub time: TdmaTime,
    pub scrambling_code: u32,
    pub bits: BitBuffer,
}
