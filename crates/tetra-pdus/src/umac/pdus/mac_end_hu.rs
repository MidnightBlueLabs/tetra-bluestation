use core::fmt;

use tetra_core::BitBuffer;
use tetra_core::pdu_parse_error::PduParseErr;

use crate::umac::enums::reservation_requirement::ReservationRequirement;

/// Clause 21.4.2.2 MAC-END-HU
#[derive(Debug, Clone)]
pub struct MacEndHu {
    // 1
    pub fill_bits: bool,
    // 1
    // pub length_ind_or_cap_req: bool,
    // 4 opt
    pub length_ind: Option<u8>,
    // 4 opt
    pub reservation_req: Option<ReservationRequirement>,
}

impl MacEndHu {
    pub fn from_bitbuf(buf: &mut BitBuffer) -> Result<Self, PduParseErr> {
        // required constant mac_pdu_type
        let mac_pdu_type = buf.read_field(1, "mac_pdu_type")?;
        assert!(mac_pdu_type == 1);
        let fill_bits = buf.read_field(1, "fill_bits")? != 0;

        let length_ind_or_cap_req = buf.read_field(1, "length_ind_or_cap_req")?;
        let (length_ind, reservation_req) = if length_ind_or_cap_req == 0 {
            let len = buf.read_field(4, "length_ind")? as u8;
            (Some(len), None)
        } else {
            let val = buf.read_field(4, "reservation_req")?;
            let res_req = ReservationRequirement::try_from(val).map_err(|_| PduParseErr::InvalidValue {
                field: "reservation_req",
                value: val,
            })?;
            (None, Some(res_req))
        };

        Ok(MacEndHu {
            fill_bits,
            length_ind,
            reservation_req,
        })
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) {
        assert!(self.length_ind.is_some() || self.reservation_req.is_some());
        assert!(!(self.length_ind.is_some() && self.reservation_req.is_some()));

        // write required constant mac_pdu_type
        buf.write_bits(1, 1);
        buf.write_bits(self.fill_bits as u8 as u64, 1);

        if let Some(v) = self.length_ind {
            buf.write_bits(0, 1); // length_ind_or_cap_req
            buf.write_bits(v as u64, 4);
        } else {
            buf.write_bits(1, 1); // length_ind_or_cap_req
            buf.write_bits(self.reservation_req.unwrap() as u64, 4);
        }
    }
}

impl fmt::Display for MacEndHu {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MacEndHu {{ fill_bits: {}", self.fill_bits)?;
        if let Some(v) = self.length_ind {
            write!(f, "  length_ind: {}", v)?;
        }
        if let Some(v) = self.reservation_req {
            write!(f, "  reservation_req: {}", v)?;
        }
        write!(f, "}}")
    }
}
