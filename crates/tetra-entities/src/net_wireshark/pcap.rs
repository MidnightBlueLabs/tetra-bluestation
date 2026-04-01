use std::{
    io::{BufWriter, Write},
    net::{IpAddr, Ipv4Addr},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

const PCAP_MAGIC_LE: u32 = 0xA1B2C3D4;
const PCAP_VERSION_MAJOR: u16 = 2;
const PCAP_VERSION_MINOR: u16 = 4;
const PCAP_SNAPLEN: u32 = 65535;
const PCAP_LINKTYPE_ETHERNET: u32 = 1;

const ETHER_TYPE_IPV4: u16 = 0x0800;
const IPV4_PROTOCOL_UDP: u8 = 17;
const IPV4_HEADER_LEN: usize = 20;
const UDP_HEADER_LEN: usize = 8;
const ETHERNET_HEADER_LEN: usize = 14;

const PCAP_SOURCE_MAC: [u8; 6] = [0x02, 0x42, 0x53, 0x54, 0x31, 0x01];
const PCAP_DEST_MAC: [u8; 6] = [0x02, 0x42, 0x53, 0x54, 0x31, 0x02];
const PCAP_SOURCE_IP: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 2);
const PCAP_SOURCE_PORT: u16 = 62069;

#[derive(Debug)]
pub enum PcapError {
    Io(std::io::Error),
    PacketTooLarge(usize),
    SystemTime(std::time::SystemTimeError),
}

impl std::fmt::Display for PcapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err}"),
            Self::PacketTooLarge(len) => write!(f, "PCAP packet length {len} exceeds u16"),
            Self::SystemTime(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for PcapError {}

impl From<std::io::Error> for PcapError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<std::time::SystemTimeError> for PcapError {
    fn from(value: std::time::SystemTimeError) -> Self {
        Self::SystemTime(value)
    }
}

pub struct PcapWriter<W: Write> {
    writer: W,
    packet_id: u16,
    destination_ip: Ipv4Addr,
    destination_port: u16,
}

impl PcapWriter<BufWriter<std::fs::File>> {
    pub fn create<P: AsRef<Path>>(path: P, host: &str, destination_port: u16) -> Result<Self, PcapError> {
        let file = std::fs::File::create(path)?;
        let writer = BufWriter::new(file);
        Self::new(writer, host, destination_port)
    }
}

impl<W: Write> PcapWriter<W> {
    pub fn new(mut writer: W, host: &str, destination_port: u16) -> Result<Self, PcapError> {
        write_global_header(&mut writer)?;

        let destination_ip = resolve_destination_ip(host);
        if destination_ip == Ipv4Addr::LOCALHOST && host.parse::<IpAddr>().ok().and_then(|ip| match ip {
            IpAddr::V4(v4) => Some(v4),
            IpAddr::V6(_) => None,
        }).is_none() {
            tracing::warn!(
                "Wireshark PCAP output uses synthetic IPv4 packets; falling back to 127.0.0.1 for host '{}'",
                host
            );
        }

        Ok(Self {
            writer,
            packet_id: 0,
            destination_ip,
            destination_port,
        })
    }

    pub fn write_datagram(&mut self, payload: &[u8]) -> Result<(), PcapError> {
        let packet = build_udp_ipv4_ethernet_packet(payload, self.packet_id, self.destination_ip, self.destination_port)?;
        self.packet_id = self.packet_id.wrapping_add(1);
        write_packet_record(&mut self.writer, &packet)?;
        self.writer.flush()?;
        Ok(())
    }
}

fn resolve_destination_ip(host: &str) -> Ipv4Addr {
    host.parse::<IpAddr>()
        .ok()
        .and_then(|ip| match ip {
            IpAddr::V4(v4) => Some(v4),
            IpAddr::V6(_) => None,
        })
        .unwrap_or(Ipv4Addr::LOCALHOST)
}

fn write_global_header<W: Write>(writer: &mut W) -> Result<(), PcapError> {
    writer.write_all(&PCAP_MAGIC_LE.to_le_bytes())?;
    writer.write_all(&PCAP_VERSION_MAJOR.to_le_bytes())?;
    writer.write_all(&PCAP_VERSION_MINOR.to_le_bytes())?;
    writer.write_all(&0i32.to_le_bytes())?;
    writer.write_all(&0u32.to_le_bytes())?;
    writer.write_all(&PCAP_SNAPLEN.to_le_bytes())?;
    writer.write_all(&PCAP_LINKTYPE_ETHERNET.to_le_bytes())?;
    Ok(())
}

fn write_packet_record<W: Write>(writer: &mut W, packet: &[u8]) -> Result<(), PcapError> {
    let ts = SystemTime::now().duration_since(UNIX_EPOCH)?;
    let ts_sec = u32::try_from(ts.as_secs()).map_err(|_| PcapError::PacketTooLarge(packet.len()))?;
    let ts_usec = ts.subsec_micros();
    let incl_len = u32::try_from(packet.len()).map_err(|_| PcapError::PacketTooLarge(packet.len()))?;

    writer.write_all(&ts_sec.to_le_bytes())?;
    writer.write_all(&ts_usec.to_le_bytes())?;
    writer.write_all(&incl_len.to_le_bytes())?;
    writer.write_all(&incl_len.to_le_bytes())?;
    writer.write_all(packet)?;
    Ok(())
}

fn build_udp_ipv4_ethernet_packet(
    payload: &[u8],
    packet_id: u16,
    destination_ip: Ipv4Addr,
    destination_port: u16,
) -> Result<Vec<u8>, PcapError> {
    let udp_len = u16::try_from(UDP_HEADER_LEN + payload.len()).map_err(|_| PcapError::PacketTooLarge(payload.len()))?;
    let ipv4_total_len =
        u16::try_from(IPV4_HEADER_LEN + usize::from(udp_len)).map_err(|_| PcapError::PacketTooLarge(payload.len()))?;

    let mut packet = Vec::with_capacity(ETHERNET_HEADER_LEN + usize::from(ipv4_total_len));
    packet.extend_from_slice(&PCAP_DEST_MAC);
    packet.extend_from_slice(&PCAP_SOURCE_MAC);
    packet.extend_from_slice(&ETHER_TYPE_IPV4.to_be_bytes());

    let mut ipv4_header = [0u8; IPV4_HEADER_LEN];
    ipv4_header[0] = 0x45;
    ipv4_header[1] = 0;
    ipv4_header[2..4].copy_from_slice(&ipv4_total_len.to_be_bytes());
    ipv4_header[4..6].copy_from_slice(&packet_id.to_be_bytes());
    ipv4_header[6..8].copy_from_slice(&0u16.to_be_bytes());
    ipv4_header[8] = 64;
    ipv4_header[9] = IPV4_PROTOCOL_UDP;
    ipv4_header[12..16].copy_from_slice(&PCAP_SOURCE_IP.octets());
    ipv4_header[16..20].copy_from_slice(&destination_ip.octets());
    let checksum = ipv4_checksum(&ipv4_header);
    ipv4_header[10..12].copy_from_slice(&checksum.to_be_bytes());
    packet.extend_from_slice(&ipv4_header);

    packet.extend_from_slice(&PCAP_SOURCE_PORT.to_be_bytes());
    packet.extend_from_slice(&destination_port.to_be_bytes());
    packet.extend_from_slice(&udp_len.to_be_bytes());
    packet.extend_from_slice(&0u16.to_be_bytes());
    packet.extend_from_slice(payload);

    Ok(packet)
}

fn ipv4_checksum(header: &[u8]) -> u16 {
    let mut sum = 0u32;
    for chunk in header.chunks_exact(2) {
        sum += u16::from_be_bytes([chunk[0], chunk[1]]) as u32;
    }

    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    !(sum as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_udp_ipv4_ethernet_packet() {
        let payload = b"TBSWexample";
        let packet = build_udp_ipv4_ethernet_packet(payload, 0x1234, Ipv4Addr::new(127, 0, 0, 1), 42069).unwrap();

        assert_eq!(packet.len(), ETHERNET_HEADER_LEN + IPV4_HEADER_LEN + UDP_HEADER_LEN + payload.len());
        assert_eq!(&packet[12..14], &ETHER_TYPE_IPV4.to_be_bytes());
        assert_eq!(packet[14], 0x45);
        assert_eq!(packet[23], IPV4_PROTOCOL_UDP);
        assert_eq!(&packet[26..30], &PCAP_SOURCE_IP.octets());
        assert_eq!(&packet[30..34], &[127, 0, 0, 1]);
        assert_eq!(&packet[34..36], &PCAP_SOURCE_PORT.to_be_bytes());
        assert_eq!(&packet[36..38], &42069u16.to_be_bytes());
        assert_eq!(&packet[(ETHERNET_HEADER_LEN + IPV4_HEADER_LEN + UDP_HEADER_LEN)..], payload);
    }

    #[test]
    fn test_writes_global_header_and_record() {
        let mut bytes = Vec::new();
        let mut writer = PcapWriter::new(&mut bytes, "127.0.0.1", 42069).unwrap();
        writer.write_datagram(b"TBSW").unwrap();

        assert!(bytes.len() > 24);
        assert_eq!(&bytes[0..4], &PCAP_MAGIC_LE.to_le_bytes());
        assert_eq!(&bytes[20..24], &PCAP_LINKTYPE_ETHERNET.to_le_bytes());
        let incl_len = u32::from_le_bytes(bytes[32..36].try_into().unwrap()) as usize;
        assert_eq!(incl_len, bytes.len() - 40);
        assert_eq!(&bytes[(bytes.len() - 4)..], b"TBSW");
    }
}
