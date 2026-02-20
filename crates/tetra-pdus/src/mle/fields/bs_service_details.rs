use core::fmt;

use tetra_core::{BitBuffer, assert_warn, pdu_parse_error::PduParseErr};

/// Clause 18.5.2.1 D-MLE-SYSINFO Table 18.26: BS Service details information element
#[derive(Debug, Clone)]
pub struct BsServiceDetails {
    // 1
    pub registration: bool,
    // 1
    pub deregistration: bool,
    // 1
    pub priority_cell: bool,
    // 1
    pub no_minimum_mode: bool,
    // 1
    pub migration: bool,
    // 1
    pub system_wide_services: bool,
    // 1
    pub voice_service: bool,
    // 1
    pub circuit_mode_data_service: bool,
    // 1
    // pub Reserved: bool,
    // 1
    pub sndcp_service: bool,
    // 1
    pub aie_service: bool,
    // 1
    pub advanced_link: bool,
}

impl BsServiceDetails {
    pub fn from_bitbuf(buf: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let registration = buf.read_field(1, "registration")? != 0;
        let deregistration = buf.read_field(1, "deregistration")? != 0;
        let priority_cell = buf.read_field(1, "priority_cell")? != 0;
        let no_minimum_mode = buf.read_field(1, "no_minimum_mode")? != 0;
        let migration = buf.read_field(1, "migration")? != 0;
        let system_wide_services = buf.read_field(1, "system_wide_services")? != 0;
        let voice_service = buf.read_field(1, "voice_service")? != 0;
        let circuit_mode_data_service = buf.read_field(1, "circuit_mode_data_service")? != 0;
        let reserved = buf.read_field(1, "reserved")?;
        assert_warn!(reserved == 0, "Reserved bit should be 0");
        let sndcp_service = buf.read_field(1, "sndcp_service")? != 0;
        let aie_service = buf.read_field(1, "aie_service")? != 0;
        let advanced_link = buf.read_field(1, "advanced_link")? != 0;

        Ok(BsServiceDetails {
            registration,
            deregistration,
            priority_cell,
            no_minimum_mode,
            migration,
            system_wide_services,
            voice_service,
            circuit_mode_data_service,
            sndcp_service,
            aie_service,
            advanced_link,
        })
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) {
        buf.write_bits(self.registration as u8 as u64, 1);
        buf.write_bits(self.deregistration as u8 as u64, 1);
        buf.write_bits(self.priority_cell as u8 as u64, 1);
        buf.write_bits(self.no_minimum_mode as u8 as u64, 1);
        buf.write_bits(self.migration as u8 as u64, 1);
        buf.write_bits(self.system_wide_services as u8 as u64, 1);
        buf.write_bits(self.voice_service as u8 as u64, 1);
        buf.write_bits(self.circuit_mode_data_service as u8 as u64, 1);
        buf.write_bits(0, 1);
        buf.write_bits(self.sndcp_service as u8 as u64, 1);
        buf.write_bits(self.aie_service as u8 as u64, 1);
        buf.write_bits(self.advanced_link as u8 as u64, 1);
    }
}

impl fmt::Display for BsServiceDetails {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "BsServiceDetails {{ registration: {}, deregistration: {}, priority_cell: {}, minimum_mode: {}, migration: {}, system_wide_services: {}, voice_service: {}, circuit_mode_data_service: {}, sndcp_service: {}, aie_service: {}, advanced_link: {} }}",
            self.registration,
            self.deregistration,
            self.priority_cell,
            self.no_minimum_mode,
            self.migration,
            self.system_wide_services,
            self.voice_service,
            self.circuit_mode_data_service,
            self.sndcp_service,
            self.aie_service,
            self.advanced_link
        )
    }
}
