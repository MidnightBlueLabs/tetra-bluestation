use core::fmt;
use std::panic;

use tetra_core::{BitBuffer, SsiType, TetraAddress, pdu_parse_error::PduParseErr};

use crate::umac::{
    enums::mac_resource_addr_type::MacResourceAddrType,
    fields::{EventLabel, basic_slotgrant::BasicSlotgrant, channel_allocation::ChanAllocElement},
};

/// Clause 21.4.3.1 MAC-RESOURCE
#[derive(Debug, Clone, Default)]
pub struct MacResource {
    /// 1 bit, designates if SDU is followed by fill bits to obtain 8-bit alignment.
    /// May be initially set to 0 and updated through MacResource::update_len_and_fill_ind
    /// Carries no meaning if Null PDU
    pub fill_bits: bool,
    /// 1 bit, only relevant if slot granting element present.
    /// 0 -> current chan, 1 -> grant on allocated chan
    /// Carries no meaning if Null PDU
    pub pos_of_grant: u8,
    /// 2 bits. upper bit = encryption enabled, lower bit = cck parity
    /// Carries no meaning if Null PDU
    pub encryption_mode: u8,
    /// 1 bit. If true, random access acknowledged
    /// Carries no meaning if Null PDU
    pub random_access_flag: bool,
    /// 6 bits, 0b111111 = FRAG START, 0b111110 = 2ND SLOT STOLEN
    /// May be left as 0 and updated through MacResource::update_len_and_fill_ind
    pub length_ind: u8,

    /// 3 bits.
    /// If not present, this is a null PDU
    pub addr: Option<TetraAddress>,
    // pub addr_type: MacResourceAddrType,
    // // 24 opt
    // pub ssi: Option<u32>,
    // // 10 opt
    pub event_label: Option<EventLabel>,
    // // 6 opt
    pub usage_marker: Option<u8>,
    // 1
    // pub power_control_flag: bool,
    /// 4 opt
    pub power_control_element: Option<u8>,
    // 1
    // pub slot_granting_flag: bool,
    /// 8 opt
    pub slot_granting_element: Option<BasicSlotgrant>,
    // 1
    // pub chan_alloc_flag: bool,
    pub chan_alloc_element: Option<ChanAllocElement>,
}

impl MacResource {
    pub fn null_pdu() -> Self {
        MacResource {
            fill_bits: false,
            pos_of_grant: 0,
            encryption_mode: 0,
            random_access_flag: false,
            length_ind: 2,
            addr: None,
            event_label: None,
            usage_marker: None,
            power_control_element: None,
            slot_granting_element: None,
            chan_alloc_element: None,
        }
    }

    pub fn from_bitbuf(buf: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let mut s = MacResource {
            fill_bits: false,
            pos_of_grant: 0,
            encryption_mode: 0,
            random_access_flag: false,
            length_ind: 0,
            addr: None,
            event_label: None,
            usage_marker: None,
            power_control_element: None,
            slot_granting_element: None,
            chan_alloc_element: None,
        };

        // required constant mac_pdu_type
        assert!(buf.read_field(2, "mac_pdu_type")? == 0);
        s.fill_bits = buf.read_field(1, "fill_bits")? != 0;
        s.pos_of_grant = buf.read_field(1, "pos_of_grant")? as u8;
        s.encryption_mode = buf.read_field(2, "encryption_mode")? as u8;
        s.random_access_flag = buf.read_field(1, "random_access_flag")? != 0;
        s.length_ind = buf.read_field(6, "length_ind")? as u8;

        // Parse address type and fields
        let bits = buf.read_field(3, "addr_type")?;
        let addr_type = MacResourceAddrType::try_from(bits).expect("invalid address type");

        match addr_type {
            MacResourceAddrType::NullPdu => {
                // Other fields don't carry meaning in null PDU, so for clarity we set them to defaults
                // While this deviates from the truly received message, it may prevent a bug or two
                s.fill_bits = false;
                s.pos_of_grant = 0;
                s.encryption_mode = 0;
                s.random_access_flag = false;
            }

            MacResourceAddrType::Ssi => {
                s.addr = Some(TetraAddress {
                    ssi: buf.read_field(24, "ssi")? as u32,
                    encrypted: s.encryption_mode != 0,
                    ssi_type: SsiType::Ssi,
                });
            }
            MacResourceAddrType::EventLabel => {
                s.event_label = Some(buf.read_field(10, "event_label")? as u16);
            }
            MacResourceAddrType::Ussi => {
                s.addr = Some(TetraAddress {
                    ssi: buf.read_field(24, "ussi")? as u32,
                    encrypted: s.encryption_mode != 0,
                    ssi_type: SsiType::Ussi,
                });
            }
            MacResourceAddrType::Smi => {
                s.addr = Some(TetraAddress {
                    ssi: buf.read_field(24, "smi")? as u32,
                    encrypted: s.encryption_mode != 0,
                    ssi_type: SsiType::Smi,
                });
            }
            MacResourceAddrType::SsiAndEventLabel => {
                s.addr = Some(TetraAddress {
                    ssi: buf.read_field(24, "ssi")? as u32,
                    encrypted: s.encryption_mode != 0,
                    ssi_type: SsiType::Ssi,
                });
                s.event_label = Some(buf.read_field(10, "event_label")? as u16);
            }
            MacResourceAddrType::SsiAndUsageMarker => {
                s.addr = Some(TetraAddress {
                    ssi: buf.read_field(24, "ssi")? as u32,
                    encrypted: s.encryption_mode != 0,
                    ssi_type: SsiType::Ssi,
                });
                s.usage_marker = Some(buf.read_field(6, "usage_marker")? as u8);
            }
            MacResourceAddrType::SmiAndEventLabel => {
                s.addr = Some(TetraAddress {
                    ssi: buf.read_field(24, "smi")? as u32,
                    encrypted: s.encryption_mode != 0,
                    ssi_type: SsiType::Smi,
                });
                s.event_label = Some(buf.read_field(10, "event_label")? as u16);
            }
        }

        if addr_type == MacResourceAddrType::NullPdu {
            s.encryption_mode = 0;
            return Ok(s);
        }

        let power_control_flag = buf.read_field(1, "power_control_flag")?;
        if power_control_flag == 1 {
            s.power_control_element = Some(buf.read_field(4, "power_control_element")? as u8);
        }

        let slot_granting_flag = buf.read_field(1, "slot_granting_flag")?;
        if slot_granting_flag == 1 {
            // Read 8-bit BasicSlotgrant element
            s.slot_granting_element = Some(BasicSlotgrant::from_bitbuf(buf)?);
        }

        let chan_alloc_flag = buf.read_field(1, "chan_alloc_flag")?;
        if chan_alloc_flag == 1 {
            s.chan_alloc_element = Some(ChanAllocElement::from_bitbuf(buf)?);
        }

        Ok(s)
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) {
        assert!(self.length_ind > 0, "length_ind must be set before writing MacResource PDU");

        buf.write_bits(0, 2);
        buf.write_bits(self.fill_bits as u8 as u64, 1);
        buf.write_bits(self.pos_of_grant as u64, 1);
        buf.write_bits(self.encryption_mode as u64, 2);
        buf.write_bits(self.random_access_flag as u8 as u64, 1);
        buf.write_bits(self.length_ind as u64, 6);

        // Derive SSI type from provided info
        let addr_type;
        if self.is_null_pdu() {
            assert!(!self.fill_bits, "fill_bits carries no meaning for Null PDU");
            assert!(self.pos_of_grant == 0, "pos_of_grant carries no meaning for Null PDU");
            assert!(self.encryption_mode == 0, "encryption_mode carries no meaning for Null PDU");
            assert!(!self.random_access_flag, "random_access_flag carries no meaning for Null PDU");

            addr_type = MacResourceAddrType::NullPdu;
        } else if let Some(addr) = self.addr {
            if addr.ssi_type == SsiType::Ssi || addr.ssi_type == SsiType::Gssi || addr.ssi_type == SsiType::Issi {
                if self.event_label.is_none() && self.usage_marker.is_none() {
                    addr_type = MacResourceAddrType::Ssi;
                } else if self.event_label.is_some() && self.usage_marker.is_none() {
                    addr_type = MacResourceAddrType::SsiAndEventLabel;
                } else if self.usage_marker.is_some() && self.event_label.is_none() {
                    addr_type = MacResourceAddrType::SsiAndUsageMarker;
                } else {
                    panic!("Invalid address type");
                }
            } else if addr.ssi_type == SsiType::Ussi && self.event_label.is_none() && self.usage_marker.is_none() {
                addr_type = MacResourceAddrType::Ussi;
            } else if addr.ssi_type == SsiType::Smi {
                assert!(self.usage_marker.is_none());
                if self.event_label.is_some() {
                    addr_type = MacResourceAddrType::SmiAndEventLabel;
                } else {
                    addr_type = MacResourceAddrType::Smi;
                }
            } else {
                panic!("Invalid address type");
            }
        } else {
            assert!(self.usage_marker.is_none());
            assert!(self.event_label.is_some());
            if self.event_label.is_none() {
                addr_type = MacResourceAddrType::NullPdu;
            } else {
                addr_type = MacResourceAddrType::EventLabel;
            }
        }

        // Write address type and fields
        buf.write_bits(addr_type as u64, 3);
        match addr_type {
            MacResourceAddrType::NullPdu => {}
            MacResourceAddrType::Ssi | MacResourceAddrType::Ussi | MacResourceAddrType::Smi => {
                assert!(self.addr.unwrap().encrypted == (self.encryption_mode != 0));
                buf.write_bits(self.addr.unwrap().ssi as u64, 24);
            }
            MacResourceAddrType::EventLabel => {
                buf.write_bits(self.event_label.unwrap() as u64, 10);
            }
            MacResourceAddrType::SsiAndEventLabel | MacResourceAddrType::SmiAndEventLabel => {
                assert!(self.addr.unwrap().encrypted == (self.encryption_mode != 0));
                buf.write_bits(self.addr.unwrap().ssi as u64, 24);
                buf.write_bits(self.event_label.unwrap() as u64, 10);
            }
            MacResourceAddrType::SsiAndUsageMarker => {
                assert!(self.addr.unwrap().encrypted == (self.encryption_mode != 0));
                buf.write_bits(self.addr.unwrap().ssi as u64, 24);
                buf.write_bits(self.usage_marker.unwrap() as u64, 6);
            }
        }

        if addr_type == MacResourceAddrType::NullPdu {
            // No additional fields
            return;
        }

        if let Some(v) = self.power_control_element {
            buf.write_bits(1, 1);
            buf.write_bits(v as u64, 4);
        } else {
            buf.write_bits(0, 1);
        }

        if let Some(v) = &self.slot_granting_element {
            buf.write_bits(1, 1);
            v.to_bitbuf(buf); // 8-bit BasicSlotgrant element
        } else {
            buf.write_bits(0, 1);
        }

        if let Some(v) = &self.chan_alloc_element {
            buf.write_bits(1, 1); // Chan alloc flag
            v.to_bitbuf(buf);
        } else {
            buf.write_bits(0, 1);
        }
    }

    pub fn is_null_pdu(&self) -> bool {
        self.addr.is_none() && self.event_label.is_none() && self.usage_marker.is_none()
    }

    pub fn compute_header_len(&self) -> usize {
        let mut ret = 16;
        if self.is_null_pdu() {
            return ret;
        }

        if self.event_label.is_some() {
            ret += 10
        };
        if self.usage_marker.is_some() {
            ret += 6
        };
        if self.addr.is_some() {
            ret += 24
        };

        ret += 1;
        if self.power_control_element.is_some() {
            ret += 4
        };
        ret += 1;
        if self.slot_granting_element.is_some() {
            ret += 8
        };
        ret += 1;
        if let Some(chan_alloc) = self.chan_alloc_element.as_ref() {
            ret += chan_alloc.compute_len();
        };

        ret
    }

    /// Updates the length_ind and fill_bits fields based on the computed header lenght and provided SDU length
    /// Returns the number of fill bits that need to be added to the PDU
    pub fn update_len_and_fill_ind(&mut self, sdu_len: usize) -> usize {
        let hdr_len = self.compute_header_len();
        let total_len = hdr_len + sdu_len;
        let total_len_bytes = (total_len + 7) / 8;
        let num_fill_bits = (8 - (total_len % 8)) % 8;

        self.length_ind = total_len_bytes as u8;
        self.fill_bits = num_fill_bits != 0;
        num_fill_bits
    }
}

impl fmt::Display for MacResource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MacResource {{ fill_bits: {}, pos_of_grant: {}, encryption_mode: {}, random_access_flag: {}, length_ind: {}",
            self.fill_bits, self.pos_of_grant, self.encryption_mode, self.random_access_flag, self.length_ind
        )?;

        if let Some(addr) = &self.addr {
            write!(f, "  addr {{ ssi: {} }}", addr.ssi)?;
            if let Some(v) = self.event_label {
                write!(f, "    event_label: {}", v)?;
            }
            if let Some(v) = self.usage_marker {
                write!(f, "    usage_marker: {}", v)?;
            }
            write!(f, "  }}")?;
        } else {
            write!(f, "  addr: Null PDU")?;
        }

        if let Some(v) = self.power_control_element {
            write!(f, "  power_control_element: {}", v)?;
        }
        if let Some(v) = &self.slot_granting_element {
            write!(f, "  slot_granting_element: {}", v)?;
        }
        if let Some(v) = &self.chan_alloc_element {
            write!(f, "  chan_alloc_element: {:?}", v)?;
        }
        write!(f, " }}")
    }
}

#[cfg(test)]
mod tests {

    use tetra_core::debug;

    use super::*;

    #[test]
    fn test_mac_resource_with_chanalloc() {
        debug::setup_logging_verbose();

        let mut buffer = BitBuffer::from_bitstr("00000000100111100000000000000000110011001111100010100101100010111111000011");
        let pdu = MacResource::from_bitbuf(&mut buffer).unwrap();
        println!("Parsed MacResource: {:?}", pdu);

        assert!(buffer.get_len_remaining() == 0);
        assert_eq!(pdu.chan_alloc_element.as_ref().unwrap().carrier_num, 1528);

        let mut new = BitBuffer::new_autoexpand(buffer.get_len());
        pdu.to_bitbuf(&mut new);
        assert_eq!(new.to_bitstr(), buffer.to_bitstr());
    }
}
