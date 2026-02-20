/// Clause 21.2.1 LLC PDU types
/// Bits: 4
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LlcPduType {
    BlAdata = 0,
    BlData = 1,
    BlUdata = 2,
    BlAck = 3,
    BlAdataFcs = 4,
    BlDataFcs = 5,
    BlUdataFcs = 6,
    BlAckFcs = 7,
    AlSetup = 8,
    AlDataAlFinal = 9,
    AlAlUdataAlUfinal = 10,
    AlAckAlRnr = 11,
    AlReconnect = 12,
    SuppLlcPdu = 13,
    L2SigPdu = 14,
    AlDisc = 15,
}

impl std::convert::TryFrom<u64> for LlcPduType {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            0 => Ok(LlcPduType::BlAdata),
            1 => Ok(LlcPduType::BlData),
            2 => Ok(LlcPduType::BlUdata),
            3 => Ok(LlcPduType::BlAck),
            4 => Ok(LlcPduType::BlAdataFcs),
            5 => Ok(LlcPduType::BlDataFcs),
            6 => Ok(LlcPduType::BlUdataFcs),
            7 => Ok(LlcPduType::BlAckFcs),
            8 => Ok(LlcPduType::AlSetup),
            9 => Ok(LlcPduType::AlDataAlFinal),
            10 => Ok(LlcPduType::AlAlUdataAlUfinal),
            11 => Ok(LlcPduType::AlAckAlRnr),
            12 => Ok(LlcPduType::AlReconnect),
            13 => Ok(LlcPduType::SuppLlcPdu),
            14 => Ok(LlcPduType::L2SigPdu),
            15 => Ok(LlcPduType::AlDisc),
            _ => Err(()),
        }
    }
}

impl LlcPduType {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            LlcPduType::BlAdata => 0,
            LlcPduType::BlData => 1,
            LlcPduType::BlUdata => 2,
            LlcPduType::BlAck => 3,
            LlcPduType::BlAdataFcs => 4,
            LlcPduType::BlDataFcs => 5,
            LlcPduType::BlUdataFcs => 6,
            LlcPduType::BlAckFcs => 7,
            LlcPduType::AlSetup => 8,
            LlcPduType::AlDataAlFinal => 9,
            LlcPduType::AlAlUdataAlUfinal => 10,
            LlcPduType::AlAckAlRnr => 11,
            LlcPduType::AlReconnect => 12,
            LlcPduType::SuppLlcPdu => 13,
            LlcPduType::L2SigPdu => 14,
            LlcPduType::AlDisc => 15,
        }
    }
}

impl From<LlcPduType> for u64 {
    fn from(e: LlcPduType) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for LlcPduType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LlcPduType::BlAdata => write!(f, "BlAdata"),
            LlcPduType::BlData => write!(f, "BlData"),
            LlcPduType::BlUdata => write!(f, "BlUdata"),
            LlcPduType::BlAck => write!(f, "BlAck"),
            LlcPduType::BlAdataFcs => write!(f, "BlAdataFcs"),
            LlcPduType::BlDataFcs => write!(f, "BlDataFcs"),
            LlcPduType::BlUdataFcs => write!(f, "BlUdataFcs"),
            LlcPduType::BlAckFcs => write!(f, "BlAckFcs"),
            LlcPduType::AlSetup => write!(f, "AlSetup"),
            LlcPduType::AlDataAlFinal => write!(f, "AlDataAlFinal"),
            LlcPduType::AlAlUdataAlUfinal => write!(f, "AlAlUdataAlUfinal"),
            LlcPduType::AlAckAlRnr => write!(f, "AlAckAlRnr"),
            LlcPduType::AlReconnect => write!(f, "AlReconnect"),
            LlcPduType::SuppLlcPdu => write!(f, "SuppLlcPdu"),
            LlcPduType::L2SigPdu => write!(f, "L2SigPdu"),
            LlcPduType::AlDisc => write!(f, "AlDisc"),
        }
    }
}
