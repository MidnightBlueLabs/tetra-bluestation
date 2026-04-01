use serde::Deserialize;
use std::collections::HashMap;

use tetra_core::ranges::SortedDisjointSsiRanges;
use toml::Value;

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

    pub ms_txpwr_max_cell: u8,

    pub local_ssi_ranges: SortedDisjointSsiRanges,

    /// IANA timezone name (e.g. "Europe/Amsterdam"). When set, enables D-NWRK-BROADCAST
    /// time broadcasting so MSs can synchronize their clocks.
    pub timezone: Option<String>,
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

    pub ms_txpwr_max_cell: Option<u8>,

    pub local_ssi_ranges: Option<Vec<(u32, u32)>>,

    pub timezone: Option<String>,

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
        ms_txpwr_max_cell: ci.ms_txpwr_max_cell.unwrap_or(4), // 30 dBm (1W), Table 18.57
        local_ssi_ranges: ci
            .local_ssi_ranges
            .map(SortedDisjointSsiRanges::from_vec_tuple)
            .unwrap_or(SortedDisjointSsiRanges::from_vec_ssirange(vec![])),
        timezone: ci.timezone,
    }
}
