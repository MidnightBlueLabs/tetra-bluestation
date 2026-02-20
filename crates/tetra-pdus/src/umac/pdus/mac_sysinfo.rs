use core::fmt;

use tetra_core::pdu_parse_error::PduParseErr;
use tetra_core::{BitBuffer, assert_warn, unimplemented_log};

use crate::umac::enums::sysinfo_opt_field_flag::SysinfoOptFieldFlag;
use crate::umac::fields::sysinfo_default_def_for_access_code_a::SysinfoDefaultDefForAccessCodeA;
use crate::umac::fields::sysinfo_ext_services::SysinfoExtendedServices;
use crate::umac::fields::ts_common_frames::TsCommonFrames;

/// Clause 21.4.4.1 SYSINFO
#[derive(Debug, Clone)]
pub struct MacSysinfo {
    // 12
    pub main_carrier: u16,
    // 4
    pub freq_band: u8,
    // 2
    pub freq_offset_index: u8,
    // 3
    pub duplex_spacing: u8,
    // 1
    pub reverse_operation: bool,
    // 2 Number of common secondary control channels on CA main carrier
    pub num_of_csch: u8,
    // 3
    pub ms_txpwr_max_cell: u8,
    // 4
    pub rxlev_access_min: u8,
    // 4
    pub access_parameter: u8,
    // 4
    pub radio_dl_timeout: u8,
    // 1, if false, has hyperframe number
    // pub has_cck_field: bool,
    // 16 opt
    pub cck_id: Option<u16>,
    // 16 opt
    pub hyperframe_number: Option<u16>,
    // 2
    pub option_field: SysinfoOptFieldFlag,
    // 20 opt
    pub ts_common_frames: Option<TsCommonFrames>,
    // 20 opt
    pub default_access_code: Option<SysinfoDefaultDefForAccessCodeA>,
    pub ext_services: Option<SysinfoExtendedServices>,
}

/// Parses SYSINFO pdu
/// Updates pos to start of TM-SDU
impl MacSysinfo {
    pub fn from_bitbuf(buf: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let mut s = MacSysinfo {
            main_carrier: 0,
            freq_band: 0,
            freq_offset_index: 0,
            duplex_spacing: 0,
            reverse_operation: false,
            num_of_csch: 0,
            ms_txpwr_max_cell: 0,
            rxlev_access_min: 0,
            access_parameter: 0,
            radio_dl_timeout: 0,
            // has_cck_field: false,
            cck_id: None,
            hyperframe_number: None,
            option_field: SysinfoOptFieldFlag::ExtServicesBroadcast,
            ts_common_frames: None,
            default_access_code: None,
            // ext_services: ExtServices::from_bitbuf(buf),
            ext_services: None,
        };

        assert_warn!(buf.read_field(2, "pdu_type")? == 2, "Not a broadcast type");
        assert_warn!(buf.read_field(2, "pdu_subtype")? == 0, "Not a SYSINFO PDU");

        s.main_carrier = buf.read_field(12, "main_carrier")? as u16;
        s.freq_band = buf.read_field(4, "freq_band")? as u8;
        s.freq_offset_index = buf.read_field(2, "freq_offset")? as u8;
        s.duplex_spacing = buf.read_field(3, "duplex_spacing")? as u8;
        s.reverse_operation = buf.read_field(1, "reverse_operation")? != 0;
        s.num_of_csch = buf.read_field(2, "num_of_csch")? as u8;
        s.ms_txpwr_max_cell = buf.read_field(3, "ms_txpwr_max_cell")? as u8;
        s.rxlev_access_min = buf.read_field(4, "rxlev_access_min")? as u8;
        s.access_parameter = buf.read_field(4, "access_parameter")? as u8;
        s.radio_dl_timeout = buf.read_field(4, "radio_dl_timeout")? as u8;

        let has_cck_field = buf.read_field(1, "has_cck_field")? == 1;
        if has_cck_field {
            s.cck_id = Some(buf.read_field(16, "cck_id")? as u16);
        } else {
            s.hyperframe_number = Some(buf.read_field(16, "hyperframe_number")? as u16);
        }

        let bits = buf.read_field(2, "option_field")?;
        s.option_field = SysinfoOptFieldFlag::try_from(bits).unwrap(); // always works

        match s.option_field {
            SysinfoOptFieldFlag::EvenMfDefForTsMode => {
                tracing::trace!("Sysinfo: Even multiframe definition for TS mode");
                buf.seek_rel(20);
                unimplemented_log!("Even multiframe definition for TS mode");
            }
            SysinfoOptFieldFlag::OddMfDefForTsMode => {
                tracing::trace!("Sysinfo: Odd multiframe definition for TS mode");
                buf.seek_rel(20);
                unimplemented_log!("Odd multiframe definition for TS mode");
            }
            SysinfoOptFieldFlag::DefaultDefForAccCodeA => {
                tracing::trace!("Sysinfo: Default definition for access code A");
                s.default_access_code = Some(SysinfoDefaultDefForAccessCodeA::from_bitbuf(buf)?);
            }
            SysinfoOptFieldFlag::ExtServicesBroadcast => {
                tracing::trace!("Sysinfo: Extended services broadcast");
                // TODO FIXME: retrieve aie_enabled bool from global config
                s.ext_services = Some(SysinfoExtendedServices::from_bitbuf(buf, true)?);
            }
        }

        Ok(s)
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) {
        buf.write_bits(2, 2); // Broadcast
        buf.write_bits(0, 2); // SYSINFO PDU;

        buf.write_bits(self.main_carrier as u64, 12);
        buf.write_bits(self.freq_band as u64, 4);
        buf.write_bits(self.freq_offset_index as u64, 2);
        buf.write_bits(self.duplex_spacing as u64, 3);
        buf.write_bits(self.reverse_operation as u8 as u64, 1);
        buf.write_bits(self.num_of_csch as u64, 2);
        buf.write_bits(self.ms_txpwr_max_cell as u64, 3);
        buf.write_bits(self.rxlev_access_min as u64, 4);
        buf.write_bits(self.access_parameter as u64, 4);
        buf.write_bits(self.radio_dl_timeout as u64, 4);

        // Write CCK ID or Hyperframe number
        assert!(
            self.cck_id.is_some() ^ self.hyperframe_number.is_some(),
            "Either cck_id or hyperframe_number must be set"
        );
        if let Some(cck_id) = self.cck_id {
            buf.write_bits(1, 1);
            buf.write_bits(cck_id as u64, 16);
        } else {
            buf.write_bits(0, 1);
            buf.write_bits(self.hyperframe_number.unwrap() as u64, 16);
        }

        // Write option field
        buf.write_bits(self.option_field as u64, 2);
        match self.option_field {
            SysinfoOptFieldFlag::EvenMfDefForTsMode => {
                assert!(self.default_access_code.is_none());
                assert!(self.ext_services.is_none());
                unimplemented_log!("Even multiframe definition for TS mode");
            }
            SysinfoOptFieldFlag::OddMfDefForTsMode => {
                assert!(self.default_access_code.is_none());
                assert!(self.ext_services.is_none());
                self.ts_common_frames.as_ref().unwrap().to_bitbuf(buf);
            }
            SysinfoOptFieldFlag::DefaultDefForAccCodeA => {
                assert!(self.ts_common_frames.is_none());
                assert!(self.ext_services.is_none());
                self.default_access_code.as_ref().unwrap().to_bitbuf(buf);
            }
            SysinfoOptFieldFlag::ExtServicesBroadcast => {
                assert!(self.ts_common_frames.is_none());
                assert!(self.default_access_code.is_none());
                self.ext_services.as_ref().unwrap().to_bitbuf(buf);
            }
        }
    }
}

impl fmt::Display for MacSysinfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "macpdu_sysinfo {{\n  main_carrier: {}\n  freq_band: {}\n  freq_offset: {}\n  duplex_spacing: {}\n  reverse_operation: {}\n  num_of_csch: {}\n  ms_txpwr_max_cell: {}\n  rxlev_access_min: {}\n  access_parameter: {}\n  radio_dl_timeout: {}\n",
            self.main_carrier,
            self.freq_band,
            self.freq_offset_index,
            self.duplex_spacing,
            self.reverse_operation,
            self.num_of_csch,
            self.ms_txpwr_max_cell,
            self.rxlev_access_min,
            self.access_parameter,
            self.radio_dl_timeout
        )?;

        if let Some(cck_id) = self.cck_id {
            writeln!(f, "  cck_id: {}", cck_id)?;
        };
        if let Some(hyperframe_number) = self.hyperframe_number {
            writeln!(f, "  hyperframe_number: {}", hyperframe_number)?;
        };

        writeln!(f, "  option_field: {}", self.option_field)?;

        match self.option_field {
            SysinfoOptFieldFlag::EvenMfDefForTsMode => {
                write!(f, "  Odd Multiframe: {}", self.ts_common_frames.as_ref().unwrap())?;
            }
            SysinfoOptFieldFlag::OddMfDefForTsMode => {
                write!(f, "  Odd Multiframe: {}", self.ts_common_frames.as_ref().unwrap())?;
            }
            SysinfoOptFieldFlag::DefaultDefForAccCodeA => {
                write!(f, "  {}", self.default_access_code.as_ref().unwrap())?;
            }
            SysinfoOptFieldFlag::ExtServicesBroadcast => {
                write!(f, "  {}", self.ext_services.as_ref().unwrap())?;
            }
        }

        write!(f, "}}")
    }
}
