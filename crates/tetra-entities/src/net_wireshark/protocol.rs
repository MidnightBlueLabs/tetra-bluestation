use tetra_core::PhyBlockNum;
use tetra_saps::tmv::enums::logical_chans::LogicalChannel;

use crate::net_wireshark::events::{Type1Capture, Type1Direction};

pub const WIRESHARK_MAGIC: &[u8; 4] = b"TBSW";
pub const WIRESHARK_PROTOCOL_VERSION: u8 = 1;

#[derive(Debug)]
pub enum ProtocolError {
    BitLengthTooLarge(usize),
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtocolError::BitLengthTooLarge(bit_len) => write!(f, "Wireshark capture bit length {bit_len} exceeds u16"),
        }
    }
}

impl std::error::Error for ProtocolError {}

pub fn encode_datagram(frame: &Type1Capture) -> Result<Vec<u8>, ProtocolError> {
    let bitstr = frame.bits.to_bitstr();
    let bit_len = u16::try_from(bitstr.len()).map_err(|_| ProtocolError::BitLengthTooLarge(bitstr.len()))?;

    let mut datagram = Vec::with_capacity(21 + bitstr.len());
    datagram.extend_from_slice(WIRESHARK_MAGIC);
    datagram.push(WIRESHARK_PROTOCOL_VERSION);
    datagram.push(match frame.direction {
        Type1Direction::Downlink => 0,
        Type1Direction::Uplink => 1,
    });
    datagram.push(logical_channel_to_u8(frame.logical_channel));
    datagram.push(block_num_to_u8(frame.block_num));
    datagram.push(u8::from(frame.crc_pass));
    datagram.push(0);
    datagram.extend_from_slice(&frame.time.h.to_be_bytes());
    datagram.push(frame.time.m);
    datagram.push(frame.time.f);
    datagram.push(frame.time.t);
    datagram.extend_from_slice(&frame.scrambling_code.to_be_bytes());
    datagram.extend_from_slice(&bit_len.to_be_bytes());
    datagram.extend_from_slice(bitstr.as_bytes());
    Ok(datagram)
}

pub fn logical_channel_to_u8(logical_channel: LogicalChannel) -> u8 {
    match logical_channel {
        LogicalChannel::Aach => 1,
        LogicalChannel::Bsch => 2,
        LogicalChannel::Bnch => 3,
        LogicalChannel::SchHd => 4,
        LogicalChannel::SchF => 5,
        LogicalChannel::Stch => 6,
        LogicalChannel::SchHu => 7,
        LogicalChannel::TchS => 8,
        LogicalChannel::Tch24 => 9,
        LogicalChannel::Tch48 => 10,
        LogicalChannel::Tch72 => 11,
        LogicalChannel::Blch => 12,
        LogicalChannel::Clch => 13,
    }
}

pub fn block_num_to_u8(block_num: PhyBlockNum) -> u8 {
    match block_num {
        PhyBlockNum::Both => 0,
        PhyBlockNum::Block1 => 1,
        PhyBlockNum::Block2 => 2,
        PhyBlockNum::Undefined => 255,
    }
}
