/// Clause 29.4.3.9 SDS Protocol identifier. Values undefined here may be user definition or reserved
/// Bits: 8
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SdsProtocolId {
    Otak = 1,
    SimpleTextMessaging = 2,
    SimpleLocationSystem = 3,
    WirelessDatagramProtocol = 4,
    WirelessControlMessageProtocol = 5,
    MDmo = 6,
    PinAuth = 7,
    EteeMessage = 8,
    SimpleImmediateTextMessaging = 9,
    LocationInformationProtocol = 10,
    NetAssistProtocol2 = 11,
    ConcatenatedSdsMessage = 12,
    Dotam = 13,
    SimpleAgnssService = 14,
    TextMessagingSdsTl = 130,
    LocationSystemSdsTl = 131,
    WirelessDatagramProtocolSdsTl = 132,
    WirelessControlMessageProtocolSdsTl = 133,
    MDmoSdsTl = 134,
    EteeMessageSdsTl = 136,
    ImmediateTextMessagingSdsTl = 137,
    MessageWithUserDataHeader = 138,
    ConcatenatedSdsMessageSdsTl = 140,
    AgnssServiceSdsTl = 141,
}

impl std::convert::TryFrom<u64> for SdsProtocolId {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            1 => Ok(SdsProtocolId::Otak),
            2 => Ok(SdsProtocolId::SimpleTextMessaging),
            3 => Ok(SdsProtocolId::SimpleLocationSystem),
            4 => Ok(SdsProtocolId::WirelessDatagramProtocol),
            5 => Ok(SdsProtocolId::WirelessControlMessageProtocol),
            6 => Ok(SdsProtocolId::MDmo),
            7 => Ok(SdsProtocolId::PinAuth),
            8 => Ok(SdsProtocolId::EteeMessage),
            9 => Ok(SdsProtocolId::SimpleImmediateTextMessaging),
            10 => Ok(SdsProtocolId::LocationInformationProtocol),
            11 => Ok(SdsProtocolId::NetAssistProtocol2),
            12 => Ok(SdsProtocolId::ConcatenatedSdsMessage),
            13 => Ok(SdsProtocolId::Dotam),
            14 => Ok(SdsProtocolId::SimpleAgnssService),
            130 => Ok(SdsProtocolId::TextMessagingSdsTl),
            131 => Ok(SdsProtocolId::LocationSystemSdsTl),
            132 => Ok(SdsProtocolId::WirelessDatagramProtocolSdsTl),
            133 => Ok(SdsProtocolId::WirelessControlMessageProtocolSdsTl),
            134 => Ok(SdsProtocolId::MDmoSdsTl),
            136 => Ok(SdsProtocolId::EteeMessageSdsTl),
            137 => Ok(SdsProtocolId::ImmediateTextMessagingSdsTl),
            138 => Ok(SdsProtocolId::MessageWithUserDataHeader),
            140 => Ok(SdsProtocolId::ConcatenatedSdsMessageSdsTl),
            141 => Ok(SdsProtocolId::AgnssServiceSdsTl),
            _ => Err(()),
        }
    }
}

impl SdsProtocolId {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            SdsProtocolId::Otak => 1,
            SdsProtocolId::SimpleTextMessaging => 2,
            SdsProtocolId::SimpleLocationSystem => 3,
            SdsProtocolId::WirelessDatagramProtocol => 4,
            SdsProtocolId::WirelessControlMessageProtocol => 5,
            SdsProtocolId::MDmo => 6,
            SdsProtocolId::PinAuth => 7,
            SdsProtocolId::EteeMessage => 8,
            SdsProtocolId::SimpleImmediateTextMessaging => 9,
            SdsProtocolId::LocationInformationProtocol => 10,
            SdsProtocolId::NetAssistProtocol2 => 11,
            SdsProtocolId::ConcatenatedSdsMessage => 12,
            SdsProtocolId::Dotam => 13,
            SdsProtocolId::SimpleAgnssService => 14,
            SdsProtocolId::TextMessagingSdsTl => 130,
            SdsProtocolId::LocationSystemSdsTl => 131,
            SdsProtocolId::WirelessDatagramProtocolSdsTl => 132,
            SdsProtocolId::WirelessControlMessageProtocolSdsTl => 133,
            SdsProtocolId::MDmoSdsTl => 134,
            SdsProtocolId::EteeMessageSdsTl => 136,
            SdsProtocolId::ImmediateTextMessagingSdsTl => 137,
            SdsProtocolId::MessageWithUserDataHeader => 138,
            SdsProtocolId::ConcatenatedSdsMessageSdsTl => 140,
            SdsProtocolId::AgnssServiceSdsTl => 141,
        }
    }
}

impl From<SdsProtocolId> for u64 {
    fn from(e: SdsProtocolId) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for SdsProtocolId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SdsProtocolId::Otak => write!(f, "Otak"),
            SdsProtocolId::SimpleTextMessaging => write!(f, "SimpleTextMessaging"),
            SdsProtocolId::SimpleLocationSystem => write!(f, "SimpleLocationSystem"),
            SdsProtocolId::WirelessDatagramProtocol => write!(f, "WirelessDatagramProtocol"),
            SdsProtocolId::WirelessControlMessageProtocol => write!(f, "WirelessControlMessageProtocol"),
            SdsProtocolId::MDmo => write!(f, "MDmo"),
            SdsProtocolId::PinAuth => write!(f, "PinAuth"),
            SdsProtocolId::EteeMessage => write!(f, "EteeMessage"),
            SdsProtocolId::SimpleImmediateTextMessaging => write!(f, "SimpleImmediateTextMessaging"),
            SdsProtocolId::LocationInformationProtocol => write!(f, "LocationInformationProtocol"),
            SdsProtocolId::NetAssistProtocol2 => write!(f, "NetAssistProtocol2"),
            SdsProtocolId::ConcatenatedSdsMessage => write!(f, "ConcatenatedSdsMessage"),
            SdsProtocolId::Dotam => write!(f, "Dotam"),
            SdsProtocolId::SimpleAgnssService => write!(f, "SimpleAgnssService"),
            SdsProtocolId::TextMessagingSdsTl => write!(f, "TextMessagingSdsTl"),
            SdsProtocolId::LocationSystemSdsTl => write!(f, "LocationSystemSdsTl"),
            SdsProtocolId::WirelessDatagramProtocolSdsTl => write!(f, "WirelessDatagramProtocolSdsTl"),
            SdsProtocolId::WirelessControlMessageProtocolSdsTl => write!(f, "WirelessControlMessageProtocolSdsTl"),
            SdsProtocolId::MDmoSdsTl => write!(f, "MDmoSdsTl"),
            SdsProtocolId::EteeMessageSdsTl => write!(f, "EteeMessageSdsTl"),
            SdsProtocolId::ImmediateTextMessagingSdsTl => write!(f, "ImmediateTextMessagingSdsTl"),
            SdsProtocolId::MessageWithUserDataHeader => write!(f, "MessageWithUserDataHeader"),
            SdsProtocolId::ConcatenatedSdsMessageSdsTl => write!(f, "ConcatenatedSdsMessageSdsTl"),
            SdsProtocolId::AgnssServiceSdsTl => write!(f, "AgnssServiceSdsTl"),
        }
    }
}
