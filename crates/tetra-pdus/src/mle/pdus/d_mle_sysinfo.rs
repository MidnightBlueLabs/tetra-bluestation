use core::fmt;

use tetra_core::{BitBuffer, pdu_parse_error::PduParseErr};

use crate::mle::fields::bs_service_details::BsServiceDetails;

/// Clause 18.4.2.2
#[derive(Debug, Clone)]
pub struct DMleSysinfo {
    // 14
    pub location_area: u16,
    // 16
    pub subscriber_class: u16,
    // 12
    pub bs_service_details: BsServiceDetails,
}

impl DMleSysinfo {
    pub fn from_bitbuf(buf: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let location_area = buf.read_field(14, "location_area")? as u16;
        let subscriber_class = buf.read_field(16, "subscriber_class")? as u16;

        // Read 12 bits from BS Service details information element
        let bs_service_details = BsServiceDetails::from_bitbuf(buf)?;

        Ok(DMleSysinfo {
            location_area,
            subscriber_class,
            bs_service_details,
        })
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) {
        buf.write_bits(self.location_area as u64, 14);
        buf.write_bits(self.subscriber_class as u64, 16);
        // Write 12 bits from BS Service details information element
        self.bs_service_details.to_bitbuf(buf);
    }
}

impl fmt::Display for DMleSysinfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "d_mle_sysinfo {{")?;
        writeln!(f, "  location_area: {}", self.location_area)?;
        writeln!(f, "  subscriber_class: {:#x}", self.subscriber_class)?;
        writeln!(f, "  bs_service_details: {}", self.bs_service_details)?;
        write!(f, "}}")
    }
}
