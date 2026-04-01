use core::fmt;

use tetra_core::{BitBuffer, pdu_parse_error::PduParseErr};

/// 16.10.5 Class of MS (Table 16.31)
/// 24 bits total, MSB-first
#[derive(Debug, Clone)]
pub struct ClassOfMs {
    /// Bit 1: 0=simplex only, 1=duplex+simplex
    pub freq_simplex_duplex: bool,
    /// Bit 2: Multislot phase modulation capability
    pub multislot_phase_mod: bool,
    /// Bit 3: Concurrent multicarrier capability
    pub concurrent_multicarrier: bool,
    /// Bit 4: Voice capability
    pub voice: bool,
    /// Bit 5: 0=E2E encryption supported, 1=not supported (reversed polarity)
    pub e2e_encryption_not_supported: bool,
    /// Bit 6: Circuit mode data capability
    pub circuit_mode_data: bool,
    /// Bit 7: TETRA packet data capability
    pub tetra_packet_data: bool,
    /// Bit 8: Fast switching capability
    pub fast_switching: bool,
    /// Bit 9: DCK encryption capability
    pub dck_encryption: bool,
    /// Bit 10: CLCH needed
    pub clch_needed: bool,
    /// Bit 11: Concurrent circuit mode capability
    pub concurrent_circuit_mode: bool,
    /// Bit 12: Original advanced link capability
    pub original_advanced_link: bool,
    /// Bit 13: Minimum mode
    pub minimum_mode: bool,
    /// Bit 14: Carrier specific signalling capability
    pub carrier_specific_signalling: bool,
    /// Bit 15: Authentication capability
    pub authentication: bool,
    /// Bit 16: SCK encryption capability
    pub sck_encryption: bool,
    /// Bits 17-19: Air interface version (3 bits)
    pub air_interface_version: u8,
    /// Bit 20: Common SCCH capability
    pub common_scch: bool,
    /// Bit 21: Reserved
    pub reserved_21: bool,
    /// Bit 22: MAC-D-BLCK capability
    pub mac_d_blck: bool,
    /// Bit 23: Extended advanced link capability
    pub extended_advanced_link: bool,
    /// Bit 24: D8PSK capability
    pub d8psk: bool,
}

impl ClassOfMs {
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let val = buffer.read_field(24, "class_of_ms")? as u32;

        Ok(ClassOfMs {
            freq_simplex_duplex: (val >> 23) & 1 != 0,
            multislot_phase_mod: (val >> 22) & 1 != 0,
            concurrent_multicarrier: (val >> 21) & 1 != 0,
            voice: (val >> 20) & 1 != 0,
            e2e_encryption_not_supported: (val >> 19) & 1 != 0,
            circuit_mode_data: (val >> 18) & 1 != 0,
            tetra_packet_data: (val >> 17) & 1 != 0,
            fast_switching: (val >> 16) & 1 != 0,
            dck_encryption: (val >> 15) & 1 != 0,
            clch_needed: (val >> 14) & 1 != 0,
            concurrent_circuit_mode: (val >> 13) & 1 != 0,
            original_advanced_link: (val >> 12) & 1 != 0,
            minimum_mode: (val >> 11) & 1 != 0,
            carrier_specific_signalling: (val >> 10) & 1 != 0,
            authentication: (val >> 9) & 1 != 0,
            sck_encryption: (val >> 8) & 1 != 0,
            air_interface_version: ((val >> 5) & 0b111) as u8,
            common_scch: (val >> 4) & 1 != 0,
            reserved_21: (val >> 3) & 1 != 0,
            mac_d_blck: (val >> 2) & 1 != 0,
            extended_advanced_link: (val >> 1) & 1 != 0,
            d8psk: val & 1 != 0,
        })
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) -> Result<(), PduParseErr> {
        let mut val: u32 = 0;
        val |= (self.freq_simplex_duplex as u32) << 23;
        val |= (self.multislot_phase_mod as u32) << 22;
        val |= (self.concurrent_multicarrier as u32) << 21;
        val |= (self.voice as u32) << 20;
        val |= (self.e2e_encryption_not_supported as u32) << 19;
        val |= (self.circuit_mode_data as u32) << 18;
        val |= (self.tetra_packet_data as u32) << 17;
        val |= (self.fast_switching as u32) << 16;
        val |= (self.dck_encryption as u32) << 15;
        val |= (self.clch_needed as u32) << 14;
        val |= (self.concurrent_circuit_mode as u32) << 13;
        val |= (self.original_advanced_link as u32) << 12;
        val |= (self.minimum_mode as u32) << 11;
        val |= (self.carrier_specific_signalling as u32) << 10;
        val |= (self.authentication as u32) << 9;
        val |= (self.sck_encryption as u32) << 8;
        val |= (self.air_interface_version as u32 & 0b111) << 5;
        val |= (self.common_scch as u32) << 4;
        val |= (self.reserved_21 as u32) << 3;
        val |= (self.mac_d_blck as u32) << 2;
        val |= (self.extended_advanced_link as u32) << 1;
        val |= self.d8psk as u32;
        buf.write_bits(val as u64, 24);
        Ok(())
    }
}

impl fmt::Display for ClassOfMs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "ClassOfMs {{ duplex:{} voice:{} e2e_enc:{} circ_data:{} pkt_data:{} dck:{} sck:{} auth:{} aiv:{} d8psk:{} }}",
            self.freq_simplex_duplex,
            self.voice,
            !self.e2e_encryption_not_supported,
            self.circuit_mode_data,
            self.tetra_packet_data,
            self.dck_encryption,
            self.sck_encryption,
            self.authentication,
            self.air_interface_version,
            self.d8psk,
        )
    }
}
