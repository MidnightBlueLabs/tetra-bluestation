use serde::Deserialize;
use std::collections::HashMap;

use tetra_core::ranges::SortedDisjointSsiRanges;
use toml::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum HomeModeDisplaySdsTextCodingScheme {
    LATIN,
    UTF16,
}

#[derive(Debug, Clone)]
pub struct CfgHomeModeDisplay {
    pub source_issi: u32,
    /// Send interval in TDMA multiframes (1 multiframe = 18 frames = 72 timeslots)
    pub interval_multiframes: u32,
    /// SDS Type4 protocol identifier byte.
    pub protocol_id: u8,
    /// Text coding scheme prepended to home mode display text user data.
    /// `LATIN` => ISO-8859-1 8-bit, `UTF16` => UCS-2/UTF-16BE.
    pub text_coding_scheme: HomeModeDisplaySdsTextCodingScheme,
    /// UTF-8 text payload appended after text coding scheme.
    pub text: String,
}

#[derive(Default, Deserialize)]
pub struct HomeModeDisplayDto {
    pub source_issi: Option<u32>,
    #[serde(alias = "interval_frames")]
    pub interval_multiframes: Option<u32>,
    pub protocol_id: Option<u8>,
    pub text_coding_scheme: Option<HomeModeDisplaySdsTextCodingScheme>,
    pub text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CfgCellInfo {
    // 2 bits, from 18.4.2.1 D-MLE-SYNC
    pub neighbor_cell_broadcast: u8,
    // 2 bits, from 18.4.2.1 D-MLE-SYNC
    pub late_entry_supported: bool,

    /// 12 bits, from MAC SYSINFO
    pub main_carrier: u16,
    /// 4 bits, from MAC SYSINFO
    pub freq_band: u8,
    /// Offset in Hz from 25kHz aligned carrier. Options: 0, 6250, -6250, 12500 Hz
    /// Represented as 0-3 in SYSINFO
    pub freq_offset_hz: i16,
    /// Index in duplex setting table. Sent in SYSINFO. Maps to a specific duplex spacing in Hz.
    /// Custom spacing can be provided optionally by setting
    pub duplex_spacing_id: u8,
    /// Custom duplex spacing in Hz, for users that use a modified, non-standard duplex spacing table.
    pub custom_duplex_spacing: Option<u32>,
    /// 1 bits, from MAC SYSINFO
    pub reverse_operation: bool,

    // 14 bits, from 18.4.2.2 D-MLE-SYSINFO
    pub location_area: u16,
    // 16 bits, from 18.4.2.2 D-MLE-SYSINFO
    pub subscriber_class: u16,

    // 1-bit service flags
    pub registration: bool,
    pub deregistration: bool,
    pub priority_cell: bool,
    pub no_minimum_mode: bool,
    pub migration: bool,
    pub system_wide_services: bool,
    pub voice_service: bool,
    pub circuit_mode_data_service: bool,
    pub sndcp_service: bool,
    pub aie_service: bool,
    pub advanced_link: bool,

    // From SYNC
    pub system_code: u8,
    pub colour_code: u8,
    pub sharing_mode: u8,
    pub ts_reserved_frames: u8,
    pub u_plane_dtx: bool,
    pub frame_18_ext: bool,

    pub local_ssi_ranges: SortedDisjointSsiRanges,

    /// IANA timezone name (e.g. "Europe/Amsterdam"). When set, enables D-NWRK-BROADCAST
    /// time broadcasting so MSs can synchronize their clocks.
    pub timezone: Option<String>,

    /// Periodic automatic broadcast of Home Mode Display SDS.
    /// Enabled when `Some`, i.e. `[cell_info.home_mode_display]` exists in config.
    pub home_mode_display: Option<CfgHomeModeDisplay>,
}

#[derive(Default, Deserialize)]
pub struct CellInfoDto {
    pub main_carrier: u16,
    pub freq_band: u8,
    pub freq_offset: i16,
    pub duplex_spacing: u8,
    pub reverse_operation: bool,
    pub custom_duplex_spacing: Option<u32>,

    pub location_area: u16,

    pub neighbor_cell_broadcast: Option<u8>,
    pub late_entry_supported: Option<bool>,
    pub subscriber_class: Option<u16>,
    pub registration: Option<bool>,
    pub deregistration: Option<bool>,
    pub priority_cell: Option<bool>,
    pub no_minimum_mode: Option<bool>,
    pub migration: Option<bool>,
    pub system_wide_services: Option<bool>,
    pub voice_service: Option<bool>,
    pub circuit_mode_data_service: Option<bool>,
    pub sndcp_service: Option<bool>,
    pub aie_service: Option<bool>,
    pub advanced_link: Option<bool>,

    pub system_code: Option<u8>,
    pub colour_code: Option<u8>,
    pub sharing_mode: Option<u8>,
    pub ts_reserved_frames: Option<u8>,
    pub u_plane_dtx: Option<bool>,
    pub frame_18_ext: Option<bool>,

    pub local_ssi_ranges: Option<Vec<(u32, u32)>>,

    pub timezone: Option<String>,

    pub home_mode_display: Option<HomeModeDisplayDto>,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

pub fn cell_dto_to_cfg(ci: CellInfoDto) -> CfgCellInfo {
    CfgCellInfo {
        main_carrier: ci.main_carrier,
        freq_band: ci.freq_band,
        freq_offset_hz: ci.freq_offset,
        duplex_spacing_id: ci.duplex_spacing,
        reverse_operation: ci.reverse_operation,
        custom_duplex_spacing: ci.custom_duplex_spacing,
        location_area: ci.location_area,
        neighbor_cell_broadcast: ci.neighbor_cell_broadcast.unwrap_or(0),
        late_entry_supported: ci.late_entry_supported.unwrap_or(false),
        subscriber_class: ci.subscriber_class.unwrap_or(65535), // All subscriber classes allowed
        registration: ci.registration.unwrap_or(true),
        deregistration: ci.deregistration.unwrap_or(true),
        priority_cell: ci.priority_cell.unwrap_or(false),
        no_minimum_mode: ci.no_minimum_mode.unwrap_or(false),
        migration: ci.migration.unwrap_or(false),
        system_wide_services: ci.system_wide_services.unwrap_or(false),
        voice_service: ci.voice_service.unwrap_or(true),
        circuit_mode_data_service: ci.circuit_mode_data_service.unwrap_or(false),
        sndcp_service: ci.sndcp_service.unwrap_or(false),
        aie_service: ci.aie_service.unwrap_or(false),
        advanced_link: ci.advanced_link.unwrap_or(false),
        system_code: ci.system_code.unwrap_or(3), // 3 = ETSI EN 300 392-2 V3.1.1
        colour_code: ci.colour_code.unwrap_or(0),
        sharing_mode: ci.sharing_mode.unwrap_or(0),
        ts_reserved_frames: ci.ts_reserved_frames.unwrap_or(0),
        u_plane_dtx: ci.u_plane_dtx.unwrap_or(false),
        frame_18_ext: ci.frame_18_ext.unwrap_or(false),
        local_ssi_ranges: ci
            .local_ssi_ranges
            .map(SortedDisjointSsiRanges::from_vec_tuple)
            .unwrap_or(SortedDisjointSsiRanges::from_vec_ssirange(vec![])),
        timezone: ci.timezone,
        home_mode_display: ci.home_mode_display.map(|h| CfgHomeModeDisplay {
            source_issi: h.source_issi.unwrap_or(0),
            interval_multiframes: h.interval_multiframes.unwrap_or(96),
            protocol_id: h.protocol_id.unwrap_or(220),
            text_coding_scheme: h.text_coding_scheme.unwrap_or(HomeModeDisplaySdsTextCodingScheme::LATIN),
            text: h.text.unwrap_or_default(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_cell_dto() -> CellInfoDto {
        CellInfoDto {
            main_carrier: 0,
            freq_band: 0,
            freq_offset: 0,
            duplex_spacing: 0,
            reverse_operation: false,
            custom_duplex_spacing: None,
            location_area: 0,
            neighbor_cell_broadcast: None,
            late_entry_supported: None,
            subscriber_class: None,
            registration: None,
            deregistration: None,
            priority_cell: None,
            no_minimum_mode: None,
            migration: None,
            system_wide_services: None,
            voice_service: None,
            circuit_mode_data_service: None,
            sndcp_service: None,
            aie_service: None,
            advanced_link: None,
            system_code: None,
            colour_code: None,
            sharing_mode: None,
            ts_reserved_frames: None,
            u_plane_dtx: None,
            frame_18_ext: None,
            local_ssi_ranges: None,
            timezone: None,
            home_mode_display: None,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn test_home_mode_display_disabled_when_subconfig_missing() {
        let cfg = cell_dto_to_cfg(base_cell_dto());
        assert!(cfg.home_mode_display.is_none());
    }

    #[test]
    fn test_home_mode_display_enabled_by_subconfig_presence_and_protocol_default() {
        let mut dto = base_cell_dto();
        dto.home_mode_display = Some(HomeModeDisplayDto {
            source_issi: Some(123456),
            interval_multiframes: Some(25),
            protocol_id: None,
            text_coding_scheme: Some(HomeModeDisplaySdsTextCodingScheme::LATIN),
            text: Some("HOME MODE".to_string()),
        });

        let cfg = cell_dto_to_cfg(dto);
        let home_mode = cfg.home_mode_display.expect("home_mode_display should be enabled");
        assert_eq!(home_mode.source_issi, 123456);
        assert_eq!(home_mode.interval_multiframes, 25);
        assert_eq!(home_mode.protocol_id, 220);
        assert_eq!(home_mode.text, "HOME MODE");
    }
}
