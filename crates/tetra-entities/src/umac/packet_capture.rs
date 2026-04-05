use std::net::UdpSocket;
use tetra_config::bluestation::CfgPacketCapture;
use tetra_core::TdmaTime;
use tetra_saps::tmv::enums::logical_chans::LogicalChannel;
use tetra_saps::tmv::{TmvUnitdataInd, TmvUnitdataReqSlot};

/// TETRA capture format, defined by the Wireshark `tetra` dissector, which is weirdly undocumented...
/// https://github.com/wireshark/wireshark/blob/master/epan/dissectors/asn1/tetra/packet-tetra-template.c

pub enum WiresharkTetraType {
    UnitDataRequest = 1,
    UnitDataIndication = 2,
    MACTimer = 3,
    IndicationDone = 127,
    RequestDone = 128
}

/// Pack the time into the format used by the Wireshark dissector
fn pack_time(time: TdmaTime) -> [u8; 4] {
    let mut t: u32 = 0;
    t |= (time.t as u32 & 0xF) << 11;
    t |= (time.f as u32 & 0x1F) << 6;
    t |= time.m as u32 & 0x3F;
    t.to_le_bytes()
}

/// Annoyingly the Wireshark dissector has a different order for the channels
/// This function defines the mapping.
fn logical_channel_to_lch_id(logical_channel: LogicalChannel) -> u32 {
    match logical_channel {
        LogicalChannel::Aach => 1,
        LogicalChannel::SchHd => 3,
        LogicalChannel::SchF => 2,
        LogicalChannel::Stch => 11,
        LogicalChannel::SchHu => 15,
        LogicalChannel::TchS => 7,
        LogicalChannel::Tch24 => 9,
        LogicalChannel::Tch48 => 10,
        LogicalChannel::Tch72 => 7,
        LogicalChannel::Bsch => 5,
        LogicalChannel::Bnch => 6,
        LogicalChannel::Blch => 0,
        LogicalChannel::Clch => 0,
    }
}

/// Build the TXREG data for a set of (upto 3) logical channels
fn build_txreg(chans: &[LogicalChannel]) -> [u8; 4] {

    assert!(chans.len() <= 3, "Too many logical channels specified to build TXREG");

    let chan_count = chans.len() as u32;
    let mut v: u32 = 0;

    // Pack each of the channel types
    for &t in chans.iter().rev() {
        let chan_id = logical_channel_to_lch_id(t);
        v = (v << 4) | (chan_id & 0xF);
    }

    // Special case for 2 channels
    if chan_count == 2 {
        v <<= 4;
    }

    // Include the channel count
    v = (v << 2) | ((chan_count - 1) & 0x3);
    v.to_le_bytes()
}

/// Build the RXREG data for a set of (upto 3) logical channels
/// Similar to TXREG except includes CRC indications for the channels too
fn build_rxreg(chans: &[LogicalChannel], crc_flags: &[bool]) -> [u8; 4] {

    assert!(chans.len() <= 3, "Too many logical channels specified to build RXREG (got {})", chans.len());
    assert_eq!(
        chans.len(),
        crc_flags.len(),
        "Different number of logical channels and crc flags building RXREG (chans {}, crc_flags {}",
        chans.len(), crc_flags.len()
    );

    let chan_count = chans.len() as u32;

    // Start with the channel count
    let mut v: u32 = chan_count & 0x3;

    // Pack each of the channel types
    for (i, &t) in chans.iter().enumerate() {
        let chan_id = logical_channel_to_lch_id(t);
        let shift = (i + 1) * 4;
        v |= (chan_id & 0xF) << shift;
    }

    // Set the CRC bits too
    for (i, &crc) in crc_flags.iter().enumerate() {
        if crc {
            v |= 1 << (i + 2);
        }
    }

    v.to_le_bytes()
}

/// Build a Wireshark TETRA dissector message for a TMV-UNITDATA Request
pub fn make_unitdata_request(slot: &TmvUnitdataReqSlot) -> Vec<u8> {

    let mut header = Vec::new();
    let mut blocks_concat = Vec::new();

    // Type
    header.push(WiresharkTetraType::UnitDataRequest as u8);

    // Carrier is always 0, since we only support one for now
    header.extend_from_slice(&[0]);

    // Packed timer
    header.extend_from_slice(&pack_time(slot.ts));

    let mut logical_channels: Vec<LogicalChannel> = Vec::new();

    // Add each of the channels for the blocks
    if let Some(blk1) = slot.blk1.as_ref() {
        logical_channels.push(blk1.logical_channel);
        (blocks_concat).extend_from_slice(&blk1.mac_block.clone().into_bytes());
    }
    if let Some(bbk) = slot.bbk.as_ref() {
        logical_channels.push(bbk.logical_channel);
        (blocks_concat).extend_from_slice(&bbk.mac_block.clone().into_bytes());
    }
    if let Some(blk2) = slot.blk2.as_ref() {
        logical_channels.push(blk2.logical_channel);
        (blocks_concat).extend_from_slice(&blk2.mac_block.clone().into_bytes());
    }

    // Build the TXREG value for this message
    let txreg = build_txreg(logical_channels.as_slice());
    header.extend_from_slice(&txreg);

    // Append the concat'd bytes containing the message payloads
    [header, blocks_concat].concat()
}

/// Build a Wireshark TETRA dissector message for a TMV-UNITDATA Indication
/// While the dissector does support multiplexed channels, we'll only ever send one at once.
pub fn make_unitdata_indication(indication: &TmvUnitdataInd, time: TdmaTime) -> Vec<u8> {

    let mut header = Vec::new();

    // Type
    header.push(WiresharkTetraType::UnitDataIndication as u8);

    // Carrier is always 0, since we only support one for now
    header.extend_from_slice(&[0]);

    // Packed timer
    header.extend_from_slice(&pack_time(time));

    // Length of PDU bytes
    let block_buf_clone = indication.pdu.clone();
    let block_bytes = block_buf_clone.into_bytes();
    header.extend((block_bytes.len() as u32).to_le_bytes());

    // Build RXREG
    let rxreg = build_rxreg(&vec![indication.logical_channel], &[indication.crc_pass]);
    header.extend_from_slice(&rxreg);

    // Append block contents
    [header, block_bytes].concat()
}

pub struct PacketCaptureClient {

    /// UDP socket used to send messages to Wireshark
    socket: UdpSocket,

    /// The destination host for the UDP messages, which Wireshark should be listening on
    host: String,

    /// The destination port for the UDP messages, which Wireshark should be listening on
    port: u16,
}

impl PacketCaptureClient {

    pub fn new(config: &CfgPacketCapture) -> std::io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        Ok(Self {
            socket,
            host: config.host.clone(),
            port: config.port,
        })
    }

    /// Send a TMV-UNITDATA Request message to Wireshark
    pub fn capture_tmv_unitdata_req(&self, slot: &TmvUnitdataReqSlot) -> std::io::Result<()> {
        let data = make_unitdata_request(slot);
        self.socket.send_to(&data, (self.host.as_ref(), self.port))?;
        Ok(())
    }

    /// Send a TMV-UNITDATA Indication message to Wireshark
    pub fn capture_tmv_unitdata_ind(&self, indication: &TmvUnitdataInd, time: TdmaTime) -> std::io::Result<()> {
        let data = make_unitdata_indication(indication, time);
        self.socket.send_to(&data, (self.host.as_ref(), self.port))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use tetra_core::{BitBuffer, PhysicalChannel};
    use tetra_saps::tmv::TmvUnitdataReq;
    use super::*;

    #[test]
    fn test_time_packs_correctly() {
        let packed = pack_time( TdmaTime { t: 1, f: 1, m: 1, h: 1 });
        assert_eq!(packed, [0x41, 0x08, 0x00, 0x00]);
    }

    #[test]
    fn test_txreg_builds_correctly() {

        let txreg = build_txreg(&[
            LogicalChannel::Bsch,
            LogicalChannel::Aach,
            LogicalChannel::Bnch
        ]);

        assert_eq!(txreg, [0x56, 0x18, 0x00, 0x00]);
    }

    #[test]
    fn test_rxreg_builds_correctly() {

        let rxreg = build_rxreg(&[
            LogicalChannel::SchF
        ], &[
            true
        ]);

        assert_eq!(rxreg, [0x25, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_tmv_unitdata_request_header() {

        let unitdata_req = make_unitdata_request(&TmvUnitdataReqSlot {
            ts: TdmaTime { t: 1, f: 2, m: 3, h: 0 },
            ul_phy_chan: PhysicalChannel::Cp,
            blk1: Some(TmvUnitdataReq {
                mac_block: BitBuffer::new(0),
                logical_channel: LogicalChannel::SchF,
                scrambling_code: 0,
            }),
            bbk: None,
            blk2: None
        });

        assert_eq!(unitdata_req, vec![0x01, 0x83, 0x08, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00]);
    }
}
