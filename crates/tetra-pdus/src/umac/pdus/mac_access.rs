use core::fmt;

use tetra_core::{BitBuffer, SsiType, TetraAddress, pdu_parse_error::PduParseErr};

use crate::umac::{enums::reservation_requirement::ReservationRequirement, fields::EventLabel};

/// Clause 21.4.2.1 MAC-ACCESS
#[derive(Debug, Clone)]
pub struct MacAccess {
    // 1
    pub fill_bits: bool,
    // 1
    pub encrypted: bool,
    // 2
    // pub addr_type: u8,
    pub addr: Option<TetraAddress>,
    // 24 opt
    // pub ssi: Option<u32>,
    // 10 opt
    pub event_label: Option<EventLabel>,
    // 1
    // pub optional_field_flag: bool,
    // 1 opt
    // pub length_ind_or_cap_req: Option<bool>,

    // 5 opt
    pub length_ind: Option<u8>,
    // 1 opt
    pub frag_flag: Option<bool>,
    // 4 opt
    pub reservation_req: Option<ReservationRequirement>,
}

impl MacAccess {
    pub fn from_bitbuf(buf: &mut BitBuffer) -> Result<Self, PduParseErr> {
        // required constant mac_pdu_type
        let mac_pdu_type = buf.read_field(1, "mac_pdu_type")?;
        assert!(mac_pdu_type == 0);
        let fill_bits = buf.read_field(1, "fill_bits")? != 0;
        let encrypted = buf.read_field(1, "encrypted")? != 0;

        let addr_type = buf.read_field(2, "addr_type")? as u8;
        let (addr, event_label) = match addr_type {
            0 => {
                let address = TetraAddress {
                    ssi_type: SsiType::Ssi,
                    ssi: buf.read_field(24, "ssi")? as u32,
                    encrypted,
                };
                (Some(address), None)
            }
            1 => {
                let ev_label = buf.read_field(10, "event_label")? as u16;
                (None, Some(ev_label))
            }
            2 => {
                let address = TetraAddress {
                    ssi_type: SsiType::Ussi,
                    ssi: buf.read_field(24, "ussi")? as u32,
                    encrypted,
                };
                (Some(address), None)
            }
            3 => {
                let address = TetraAddress {
                    ssi_type: SsiType::Smi,
                    ssi: buf.read_field(24, "smi")? as u32,
                    encrypted,
                };
                (Some(address), None)
            }
            _ => {
                panic!();
            }
        };

        let optional_field_flag = buf.read_field(1, "optional_field_flag")? != 0;
        let (length_ind, frag_flag, reservation_req) = if optional_field_flag {
            let length_ind_or_cap_req = buf.read_field(1, "length_ind_or_cap_req")?;
            if length_ind_or_cap_req == 0 {
                let len = buf.read_field(5, "length_ind")? as u8;
                (Some(len), None, None)
            } else {
                let frag = buf.read_field(1, "frag_flag")? != 0;
                let val = buf.read_field(4, "reservation_req")?;
                let res_req = ReservationRequirement::try_from(val).unwrap(); // Never fails
                (None, Some(frag), Some(res_req))
            }
        } else {
            (None, None, None)
        };

        Ok(MacAccess {
            fill_bits,
            encrypted,
            addr,
            event_label,
            length_ind,
            frag_flag,
            reservation_req,
        })
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) {
        // write required constant mac_pdu_type
        buf.write_bits(0, 1);
        buf.write_bits(self.fill_bits as u8 as u64, 1);
        buf.write_bits(self.encrypted as u8 as u64, 1);

        // Derive addr_type from addr and write type and field
        if let Some(addr) = self.addr {
            assert!(addr.encrypted == self.encrypted, "pdu and addr need same encryption status");
            match addr.ssi_type {
                SsiType::Ssi => {
                    buf.write_bits(0, 2);
                    buf.write_bits(addr.ssi as u64, 24);
                }
                SsiType::Ussi => {
                    buf.write_bits(2, 2);
                    buf.write_bits(addr.ssi as u64, 24);
                }
                SsiType::Smi => {
                    buf.write_bits(3, 2);
                    buf.write_bits(addr.ssi as u64, 24);
                }
                _ => panic!("unexpected ssi_type {:?}", addr.ssi_type),
            }
        } else if let Some(event_label) = self.event_label {
            buf.write_bits(1, 2);
            buf.write_bits(event_label as u64, 10);
        } else {
            panic!("either addr or event label must be supplied");
        }

        if self.length_ind.is_some() || self.frag_flag.is_some() || self.reservation_req.is_some() {
            assert!(!(self.length_ind.is_some() && self.frag_flag.is_some() && self.reservation_req.is_some()));

            buf.write_bits(1, 1); // optional field flag
            if self.length_ind.is_some() {
                buf.write_bits(0, 1); // length_ind_or_cap_req
                buf.write_bits(self.length_ind.unwrap() as u64, 5);
            } else {
                buf.write_bits(1, 1); // length_ind_or_cap_req
                buf.write_bits(self.frag_flag.unwrap() as u64, 1);
                buf.write_bits(self.reservation_req.unwrap() as u64, 4);
            }
        } else {
            buf.write_bits(0, 1); // optional field flag
        }
    }

    pub fn is_null_pdu(&self) -> bool {
        self.length_ind.unwrap_or(1) == 0
    }

    pub fn is_frag_start(&self) -> bool {
        self.frag_flag.unwrap_or(false)
    }
}

impl fmt::Display for MacAccess {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MacAccess {{ fill_bits: {} encrypted: {}", self.fill_bits, self.encrypted)?;
        if let Some(addr) = self.addr {
            write!(f, " addr: {}", addr)?;
        }
        if let Some(event_label) = self.event_label {
            write!(f, " event_label: {:?}", event_label)?;
        }
        if let Some(v) = self.length_ind {
            write!(f, " length_ind: {}", v)?;
        }
        if let Some(v) = self.frag_flag {
            write!(f, " frag_flag: {}", v)?;
        }
        if let Some(v) = self.reservation_req {
            write!(f, " reservation_req: {}", v)?;
        }
        write!(f, " }}")
    }
}
