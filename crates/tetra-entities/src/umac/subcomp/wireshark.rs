use std::net::UdpSocket;

/// TETRA capture format, defined by the Wireshark `tetra` dissector, which is weirdly undocumented...
/// https://github.com/wireshark/wireshark/blob/master/epan/dissectors/asn1/tetra/packet-tetra-template.c

/// The possible logical channels
pub enum WiresharkTetraLogicalChannel {
    AccessAssignment = 1,
    SignallingFull = 2,
    SignallingHalf = 3,
    BroadcastSync = 5,
    BroadcastNetwork = 6,
    TrafficFull = 7,
    TrafficHalf = 8,
    Traffic2_4 = 9,
    Traffic4_8 = 10,
    Stealing = 11,
    SignallingHalfUplink = 15,
}

/// The direction of the message
/// Uplink signifies a message from MS to BS
/// Downlink signifies a message from BS to MS
pub enum WiresharkTetraDirection {
    Uplink = 0,
    Downlink = 1,
}

/// A message to be sent to Wireshark for dissection
/// May contain upto 3 logical channels worth of data.
pub struct WiresharkTetraMessage {
    
}


pub struct WiresharkSender {
    socket: UdpSocket,
}

impl WiresharkSender {

    pub fn new() -> std::io::Result<Self> {
        let socket = UdpSocket::bind("127.0.0.1:7075")?;
        Ok(Self { socket })
    }

    // pub fn send(&self, header: &GsmtapHeader, payload: &[u8]) -> std::io::Result<()> {
    //     let mut packet = Vec::with_capacity(16 + payload.len());
    //     packet.extend_from_slice(&header.to_bytes());
    //     packet.extend_from_slice(payload);
    //     self.socket.send_to(&packet, "127.0.0.1:7074")?;
    //     Ok(())
    // }
}
