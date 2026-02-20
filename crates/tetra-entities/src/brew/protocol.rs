//! Brew protocol binary message parsing and serialization (2-byte [kind, type] prefix, little-endian)

use uuid::Uuid;

// ─── Message classes ───────────────────────────────────────────────

pub const BREW_CLASS_SUBSCRIBER: u8 = 0xf0;
pub const BREW_CLASS_CALL_CONTROL: u8 = 0xf1;
pub const BREW_CLASS_FRAME: u8 = 0xf2;
pub const BREW_CLASS_ERROR: u8 = 0xf3;
pub const BREW_CLASS_SERVICE: u8 = 0xf4;

// ─── Subscriber control types (0xf0) ──────────────────────────────

pub const BREW_SUBSCRIBER_DEREGISTER: u8 = 0;
pub const BREW_SUBSCRIBER_REGISTER: u8 = 1;
pub const BREW_SUBSCRIBER_REREGISTER: u8 = 2;
pub const BREW_SUBSCRIBER_AFFILIATE: u8 = 8;
pub const BREW_SUBSCRIBER_DEAFFILIATE: u8 = 9;

// ─── Call control types (0xf1) ────────────────────────────────────

pub const CALL_STATE_GROUP_TX: u8 = 2;
pub const CALL_STATE_GROUP_IDLE: u8 = 3;
pub const CALL_STATE_SETUP_REQUEST: u8 = 4;
pub const CALL_STATE_SETUP_ACCEPT: u8 = 5;
pub const CALL_STATE_SETUP_REJECT: u8 = 6;
pub const CALL_STATE_CALL_ALERT: u8 = 7;
pub const CALL_STATE_CONNECT_REQUEST: u8 = 8;
pub const CALL_STATE_CONNECT_CONFIRM: u8 = 9;
pub const CALL_STATE_CALL_RELEASE: u8 = 10;
pub const CALL_STATE_SHORT_TRANSFER: u8 = 11;
pub const CALL_STATE_SIMPLEX_GRANTED: u8 = 12;
pub const CALL_STATE_SIMPLEX_IDLE: u8 = 13;

// ─── Frame types (0xf2) ──────────────────────────────────────────

pub const FRAME_TYPE_TRAFFIC_CHANNEL: u8 = 0;
pub const FRAME_TYPE_SDS_TRANSFER: u8 = 1;
pub const FRAME_TYPE_SDS_REPORT: u8 = 2;
pub const FRAME_TYPE_DTMF_DATA: u8 = 3;
pub const FRAME_TYPE_PACKET_DATA: u8 = 4;

// ─── Error types (0xf3) ──────────────────────────────────────────

pub const BREW_TYPE_MALFORMED: u8 = 0;
pub const BREW_TYPE_RESTRICTED: u8 = 1;

// ─── Parsed message types ─────────────────────────────────────────

/// Top-level parsed Brew message
#[derive(Debug, Clone)]
pub enum BrewMessage {
    Subscriber(BrewSubscriberMessage),
    CallControl(BrewCallControlMessage),
    Frame(BrewFrameMessage),
    Error(BrewErrorMessage),
    Service(BrewServiceMessage),
}

/// Subscriber control (0xf0)
#[derive(Debug, Clone)]
pub struct BrewSubscriberMessage {
    pub msg_type: u8,
    pub number: u32,      // ISSI
    pub time: u64,        // UNIX timestamp
    pub fraction: u32,    // Nanoseconds
    pub groups: Vec<u32>, // GSSIs (for affiliate/deaffiliate)
}

/// Group transmission data, part of CALL_STATE_GROUP_TX
#[derive(Debug, Clone)]
pub struct BrewGroupTransmission {
    pub source: u32,      // ISSI of caller
    pub destination: u32, // GSSI of group
    pub priority: u8,
    pub access: u8,
    pub service: u16, // Speech service
}

/// Call control (0xf1)
#[derive(Debug, Clone)]
pub struct BrewCallControlMessage {
    pub call_state: u8,
    pub identifier: Uuid, // Call session UUID (16 bytes)
    pub payload: BrewCallPayload,
}

/// Union-like payload for call control messages
#[derive(Debug, Clone)]
pub enum BrewCallPayload {
    /// CALL_STATE_GROUP_TX
    GroupTransmission(BrewGroupTransmission),
    /// CALL_STATE_GROUP_IDLE, CALL_STATE_SETUP_REJECT, CALL_STATE_CALL_RELEASE
    Cause(u8),
    /// CALL_STATE_SETUP_ACCEPT, CALL_STATE_CALL_ALERT — no extra payload
    Empty,
    /// Unknown/unhandled call state
    Raw(Vec<u8>),
}

/// Voice and data frames (0xf2)
#[derive(Debug, Clone)]
pub struct BrewFrameMessage {
    pub frame_type: u8,
    pub identifier: Uuid, // Call session UUID
    pub length_bits: u16, // Length of data in bits
    pub data: Vec<u8>,
}

/// Error messages (0xf3)
#[derive(Debug, Clone)]
pub struct BrewErrorMessage {
    pub error_type: u8,
    pub data: Vec<u8>,
}

/// Service messages (0xf4)
#[derive(Debug, Clone)]
pub struct BrewServiceMessage {
    pub service_type: u8,
    pub json_data: String,
}

// ─── Parsing ──────────────────────────────────────────────────────

/// Parse error
#[derive(Debug)]
pub enum BrewParseError {
    TooShort(usize),
    UnknownClass(u8),
    InvalidUtf8,
    InvalidUuid,
}

impl std::fmt::Display for BrewParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort(n) => write!(f, "message too short: {} bytes", n),
            Self::UnknownClass(c) => write!(f, "unknown message class: 0x{:02x}", c),
            Self::InvalidUtf8 => write!(f, "invalid UTF-8 in service message"),
            Self::InvalidUuid => write!(f, "invalid UUID in call control message"),
        }
    }
}

/// Read a little-endian u16 from a byte slice
fn read_u16_le(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

/// Read a little-endian u32 from a byte slice
fn read_u32_le(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]])
}

/// Read a little-endian u64 from a byte slice
fn read_u64_le(data: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
        data[offset + 4],
        data[offset + 5],
        data[offset + 6],
        data[offset + 7],
    ])
}

/// Write a little-endian u16 to a byte vec
fn write_u16_le(buf: &mut Vec<u8>, val: u16) {
    buf.extend_from_slice(&val.to_le_bytes());
}

/// Write a little-endian u32 to a byte vec
fn write_u32_le(buf: &mut Vec<u8>, val: u32) {
    buf.extend_from_slice(&val.to_le_bytes());
}

/// Write a little-endian u64 to a byte vec
fn write_u64_le(buf: &mut Vec<u8>, val: u64) {
    buf.extend_from_slice(&val.to_le_bytes());
}

/// Parse a raw binary Brew message into a typed BrewMessage
pub fn parse_brew_message(data: &[u8]) -> Result<BrewMessage, BrewParseError> {
    if data.len() < 2 {
        return Err(BrewParseError::TooShort(data.len()));
    }

    let kind = data[0];
    let msg_type = data[1];

    match kind {
        BREW_CLASS_SUBSCRIBER => parse_subscriber(msg_type, data),
        BREW_CLASS_CALL_CONTROL => parse_call_control(msg_type, data),
        BREW_CLASS_FRAME => parse_frame(msg_type, data),
        BREW_CLASS_ERROR => parse_error(msg_type, data),
        BREW_CLASS_SERVICE => parse_service(msg_type, data),
        _ => Err(BrewParseError::UnknownClass(kind)),
    }
}

fn parse_subscriber(msg_type: u8, data: &[u8]) -> Result<BrewMessage, BrewParseError> {
    // Minimum: kind(1) + type(1) + number(4) + time(8) + fraction(4) = 18 bytes
    if data.len() < 18 {
        return Err(BrewParseError::TooShort(data.len()));
    }

    let number = read_u32_le(data, 2);
    let time = read_u64_le(data, 6);
    let fraction = read_u32_le(data, 14);

    // Remaining bytes are GSSIs (4 bytes each) for affiliate/deaffiliate
    let mut groups = Vec::new();
    let mut offset = 18;
    while offset + 4 <= data.len() {
        groups.push(read_u32_le(data, offset));
        offset += 4;
    }

    Ok(BrewMessage::Subscriber(BrewSubscriberMessage {
        msg_type,
        number,
        time,
        fraction,
        groups,
    }))
}

fn parse_call_control(call_state: u8, data: &[u8]) -> Result<BrewMessage, BrewParseError> {
    // Minimum: kind(1) + type(1) + uuid(16) = 18 bytes
    if data.len() < 18 {
        return Err(BrewParseError::TooShort(data.len()));
    }

    let uuid_bytes: [u8; 16] = data[2..18].try_into().map_err(|_| BrewParseError::InvalidUuid)?;
    let identifier = Uuid::from_bytes(uuid_bytes);

    let payload_data = &data[18..];

    let payload = match call_state {
        CALL_STATE_GROUP_TX => {
            // BrewGroupTransmission: source(4) + destination(4) + priority(1) + access(1) + service(2) = 12 bytes
            if payload_data.len() < 12 {
                return Err(BrewParseError::TooShort(data.len()));
            }
            BrewCallPayload::GroupTransmission(BrewGroupTransmission {
                source: read_u32_le(payload_data, 0),
                destination: read_u32_le(payload_data, 4),
                priority: payload_data[8],
                access: payload_data[9],
                service: read_u16_le(payload_data, 10),
            })
        }

        CALL_STATE_GROUP_IDLE | CALL_STATE_SETUP_REJECT | CALL_STATE_CALL_RELEASE => {
            // Single byte cause
            if payload_data.is_empty() {
                return Err(BrewParseError::TooShort(data.len()));
            }
            BrewCallPayload::Cause(payload_data[0])
        }

        CALL_STATE_SETUP_ACCEPT | CALL_STATE_CALL_ALERT => {
            // No extra payload
            BrewCallPayload::Empty
        }

        _ => {
            // Store raw for unhandled types
            BrewCallPayload::Raw(payload_data.to_vec())
        }
    };

    Ok(BrewMessage::CallControl(BrewCallControlMessage {
        call_state,
        identifier,
        payload,
    }))
}

fn parse_frame(frame_type: u8, data: &[u8]) -> Result<BrewMessage, BrewParseError> {
    // kind(1) + type(1) + uuid(16) + length(2) = 20 bytes minimum
    if data.len() < 20 {
        return Err(BrewParseError::TooShort(data.len()));
    }

    let uuid_bytes: [u8; 16] = data[2..18].try_into().map_err(|_| BrewParseError::InvalidUuid)?;
    let identifier = Uuid::from_bytes(uuid_bytes);

    let length_bits = read_u16_le(data, 18);
    let frame_data = data[20..].to_vec();

    Ok(BrewMessage::Frame(BrewFrameMessage {
        frame_type,
        identifier,
        length_bits,
        data: frame_data,
    }))
}

fn parse_error(error_type: u8, data: &[u8]) -> Result<BrewMessage, BrewParseError> {
    Ok(BrewMessage::Error(BrewErrorMessage {
        error_type,
        data: data[2..].to_vec(),
    }))
}

fn parse_service(service_type: u8, data: &[u8]) -> Result<BrewMessage, BrewParseError> {
    // Data is NULL-terminated JSON
    let json_bytes = &data[2..];
    // Find NULL terminator or use full length
    let end = json_bytes.iter().position(|&b| b == 0).unwrap_or(json_bytes.len());
    let json_str = std::str::from_utf8(&json_bytes[..end]).map_err(|_| BrewParseError::InvalidUtf8)?;

    Ok(BrewMessage::Service(BrewServiceMessage {
        service_type,
        json_data: json_str.to_string(),
    }))
}

// ─── Building (outgoing messages) ─────────────────────────────────

/// Build a subscriber registration message
pub fn build_subscriber_register(issi: u32, groups: &[u32]) -> Vec<u8> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();

    let mut buf = Vec::with_capacity(18 + groups.len() * 4);
    buf.push(BREW_CLASS_SUBSCRIBER);
    buf.push(BREW_SUBSCRIBER_REGISTER);
    write_u32_le(&mut buf, issi);
    write_u64_le(&mut buf, now.as_secs());
    write_u32_le(&mut buf, now.subsec_nanos());
    buf
}

/// Build a subscriber affiliation message
pub fn build_subscriber_affiliate(issi: u32, groups: &[u32]) -> Vec<u8> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();

    let mut buf = Vec::with_capacity(18 + groups.len() * 4);
    buf.push(BREW_CLASS_SUBSCRIBER);
    buf.push(BREW_SUBSCRIBER_AFFILIATE);
    write_u32_le(&mut buf, issi);
    write_u64_le(&mut buf, now.as_secs());
    write_u32_le(&mut buf, now.subsec_nanos());
    for &gssi in groups {
        write_u32_le(&mut buf, gssi);
    }
    buf
}

/// Build a subscriber deaffiliation message
pub fn build_subscriber_deaffiliate(issi: u32, groups: &[u32]) -> Vec<u8> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();

    let mut buf = Vec::with_capacity(18 + groups.len() * 4);
    buf.push(BREW_CLASS_SUBSCRIBER);
    buf.push(BREW_SUBSCRIBER_DEAFFILIATE);
    write_u32_le(&mut buf, issi);
    write_u64_le(&mut buf, now.as_secs());
    write_u32_le(&mut buf, now.subsec_nanos());
    for &gssi in groups {
        write_u32_le(&mut buf, gssi);
    }
    buf
}

/// Build a subscriber deregistration message
pub fn build_subscriber_deregister(issi: u32) -> Vec<u8> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();

    let mut buf = Vec::with_capacity(18);
    buf.push(BREW_CLASS_SUBSCRIBER);
    buf.push(BREW_SUBSCRIBER_DEREGISTER);
    write_u32_le(&mut buf, issi);
    write_u64_le(&mut buf, now.as_secs());
    write_u32_le(&mut buf, now.subsec_nanos());
    buf
}

/// Build a group call transmission start message (GROUP_TX)
/// Sent when a local radio starts transmitting on a subscribed group
pub fn build_group_tx(session_uuid: &Uuid, source_issi: u32, dest_gssi: u32, priority: u8, service: u16) -> Vec<u8> {
    // kind(1) + type(1) + uuid(16) + source(4) + dest(4) + priority(1) + access(1) + service(2) = 30
    let mut buf = Vec::with_capacity(30);
    buf.push(BREW_CLASS_CALL_CONTROL);
    buf.push(CALL_STATE_GROUP_TX);
    buf.extend_from_slice(session_uuid.as_bytes());
    write_u32_le(&mut buf, source_issi);
    write_u32_le(&mut buf, dest_gssi);
    buf.push(priority);
    buf.push(0); // access = 0 (normal)
    write_u16_le(&mut buf, service);
    buf
}

/// Build a voice frame message (ACELP traffic channel data)
/// `data` should be packed ACELP bits (1 bit per byte in STE format, with
/// a leading STE header byte prepended by the caller if needed)
pub fn build_voice_frame(session_uuid: &Uuid, length_bits: u16, data: &[u8]) -> Vec<u8> {
    // kind(1) + type(1) + uuid(16) + length(2) + data = 20 + data.len()
    let mut buf = Vec::with_capacity(20 + data.len());
    buf.push(BREW_CLASS_FRAME);
    buf.push(FRAME_TYPE_TRAFFIC_CHANNEL);
    buf.extend_from_slice(session_uuid.as_bytes());
    write_u16_le(&mut buf, length_bits);
    buf.extend_from_slice(data);
    buf
}

/// Build a group call idle (hangup) message
pub fn build_group_idle(session_uuid: &Uuid, cause: u8) -> Vec<u8> {
    let mut buf = Vec::with_capacity(19);
    buf.push(BREW_CLASS_CALL_CONTROL);
    buf.push(CALL_STATE_GROUP_IDLE);
    buf.extend_from_slice(session_uuid.as_bytes());
    buf.push(cause);
    buf
}

/// Build a service query (query subscriber profiles)
pub fn build_query_subscribers(issis: &[u32]) -> Vec<u8> {
    let json = serde_json::to_string(issis).unwrap_or_else(|_| "[]".to_string());
    let mut buf = Vec::with_capacity(3 + json.len());
    buf.push(BREW_CLASS_SERVICE);
    buf.push(1); // Query subscribers type
    buf.extend_from_slice(json.as_bytes());
    buf.push(0); // NULL terminator
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_group_tx() {
        let uuid = Uuid::new_v4();
        let mut data = vec![BREW_CLASS_CALL_CONTROL, CALL_STATE_GROUP_TX];
        data.extend_from_slice(uuid.as_bytes());
        // BrewGroupTransmission: source(4) + dest(4) + priority(1) + access(1) + service(2)
        write_u32_le(&mut data, 1001); // source ISSI
        write_u32_le(&mut data, 26); // destination GSSI
        data.push(3); // priority
        data.push(0); // access
        write_u16_le(&mut data, 0); // service

        let msg = parse_brew_message(&data).unwrap();
        if let BrewMessage::CallControl(cc) = msg {
            assert_eq!(cc.call_state, CALL_STATE_GROUP_TX);
            assert_eq!(cc.identifier, uuid);
            if let BrewCallPayload::GroupTransmission(gt) = cc.payload {
                assert_eq!(gt.source, 1001);
                assert_eq!(gt.destination, 26);
                assert_eq!(gt.priority, 3);
            } else {
                panic!("Expected GroupTransmission payload");
            }
        } else {
            panic!("Expected CallControl message");
        }
    }

    #[test]
    fn test_parse_voice_frame() {
        let uuid = Uuid::new_v4();
        let mut data = vec![BREW_CLASS_FRAME, FRAME_TYPE_TRAFFIC_CHANNEL];
        data.extend_from_slice(uuid.as_bytes());
        write_u16_le(&mut data, 274); // length in bits
        // 36 bytes of fake ACELP data
        let acelp = vec![0x80; 36];
        data.extend_from_slice(&acelp);

        let msg = parse_brew_message(&data).unwrap();
        if let BrewMessage::Frame(frame) = msg {
            assert_eq!(frame.frame_type, FRAME_TYPE_TRAFFIC_CHANNEL);
            assert_eq!(frame.identifier, uuid);
            assert_eq!(frame.length_bits, 274);
            assert_eq!(frame.data.len(), 36);
        } else {
            panic!("Expected Frame message");
        }
    }

    #[test]
    fn test_parse_group_idle() {
        let uuid = Uuid::new_v4();
        let mut data = vec![BREW_CLASS_CALL_CONTROL, CALL_STATE_GROUP_IDLE];
        data.extend_from_slice(uuid.as_bytes());
        data.push(0); // cause = normal

        let msg = parse_brew_message(&data).unwrap();
        if let BrewMessage::CallControl(cc) = msg {
            assert_eq!(cc.call_state, CALL_STATE_GROUP_IDLE);
            if let BrewCallPayload::Cause(cause) = cc.payload {
                assert_eq!(cause, 0);
            } else {
                panic!("Expected Cause payload");
            }
        } else {
            panic!("Expected CallControl message");
        }
    }
}
