use core::fmt;

use tetra_core::pdu_parse_error::PduParseErr;
use tetra_core::{BitBuffer, SsiType, TetraAddress};

use crate::umac::enums::reservation_requirement::ReservationRequirement;

/// Clause 21.4.2.3 MAC-DATA
#[derive(Debug, Clone)]
pub struct MacData {
    // 1
    pub fill_bits: bool,
    // 1
    pub encrypted: bool,
    // 2
    // pub addr_type: u8,
    // 24 opt, if addr_type in [0,2,3]
    pub addr: Option<TetraAddress>,
    // 10 opt, if addr_type == 1
    pub event_label: Option<u16>,

    /// 6 bit, optional. If not provided, frag_flag and reservation_req must be provided
    pub length_ind: Option<u8>,
    /// 1 bit, optional. If not provided, length_ind must be provided
    pub frag_flag: Option<bool>,
    /// 4 opt, optional. If not provided, length_ind must be provided
    pub reservation_req: Option<ReservationRequirement>,
}

impl MacData {
    pub fn from_bitbuf(buf: &mut BitBuffer) -> Result<Self, PduParseErr> {
        // required constant mac_pdu_type
        assert!(buf.read_field(2, "mac_pdu_type")? == 0);
        let fill_bits = buf.read_field(1, "fill_bits")? != 0;
        let encrypted = buf.read_field(1, "encrypted")? != 0;
        let addr_type = buf.read_field(2, "addr_type")? as u8;
        let (addr, event_label) = match addr_type {
            0 => {
                let ssi = buf.read_field(24, "ssi")? as u32;
                let addr = TetraAddress {
                    ssi,
                    ssi_type: SsiType::Ssi,
                    encrypted,
                }; // Either ISSI or GSSI
                (Some(addr), None)
            }
            1 => {
                let event_label = buf.read_field(10, "event_label")? as u16;
                (None, Some(event_label))
            }
            2 => {
                let ssi = buf.read_field(24, "ssi")? as u32;
                let addr = TetraAddress {
                    ssi,
                    ssi_type: SsiType::Ussi,
                    encrypted,
                };
                (Some(addr), None)
            }
            3 => {
                let ssi = buf.read_field(24, "ssi")? as u32;
                let addr = TetraAddress {
                    ssi,
                    ssi_type: SsiType::Smi,
                    encrypted,
                };
                (Some(addr), None)
            }
            _ => {
                unreachable!();
            }
        };

        let length_ind_or_cap_req = buf.read_field(1, "length_ind_or_cap_req")?;
        let (length_ind, frag_flag, reservation_req) = match length_ind_or_cap_req {
            0 => (Some(buf.read_field(6, "length_ind")? as u8), None, None),
            1 => {
                let frag_flag = buf.read_field(1, "frag_flag")? != 0;
                let val = buf.read_field(4, "reservation_requirement")?;
                let res_req = ReservationRequirement::try_from(val).unwrap(); // can't fail
                buf.read_bits(1); // Reserved bit
                (None, Some(frag_flag), Some(res_req))
            }
            _ => {
                unreachable!();
            }
        };

        Ok(MacData {
            fill_bits,
            encrypted,
            event_label,
            addr,
            length_ind,
            frag_flag,
            reservation_req,
        })
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) {
        // write required constant mac_pdu_type
        buf.write_bits(0, 2);
        buf.write_bits(self.fill_bits as u8 as u64, 1);
        buf.write_bits(self.encrypted as u8 as u64, 1);

        assert!(
            self.addr.is_some() ^ self.event_label.is_some(),
            "either addr or event_label must be set"
        );

        // If addr is given; we write one of three address types followed by the 24-bit addr
        if let Some(addr) = &self.addr {
            assert!(addr.encrypted == self.encrypted);
            match addr.ssi_type {
                SsiType::Ssi | SsiType::Issi | SsiType::Gssi => {
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
                _ => {
                    panic!("unexpected ssi_type {:?}", addr.ssi_type)
                }
            };
        } else if let Some(event_label) = self.event_label {
            // We must have an event label
            buf.write_bits(1, 2);
            buf.write_bits(event_label as u64, 10);
        } else {
            unreachable!();
        }

        // Check if we have a length indication or if we start fragmentation
        if self.length_ind.is_some() {
            buf.write_bits(0, 1); // length_ind_or_cap_req
            buf.write_bits(self.length_ind.unwrap() as u64, 6);
        } else {
            assert!(self.frag_flag.is_some() && self.reservation_req.is_some());
            buf.write_bits(1, 1); // length_ind_or_cap_req
            buf.write_bits(self.frag_flag.unwrap() as u64, 1);
            buf.write_bits(self.reservation_req.unwrap() as u64, 4);
            buf.write_bits(0, 1); // Reserved bit
        }
    }
}

impl fmt::Display for MacData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MacData {{ fill_bits: {} encrypted: {}", self.fill_bits, self.encrypted)?;
        if let Some(v) = &self.addr {
            write!(f, " addr: {}", v)?;
        }
        if let Some(v) = self.event_label {
            write!(f, " event_label: {}", v)?;
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
