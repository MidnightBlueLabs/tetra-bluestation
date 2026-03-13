use tetra_config::bluestation::{CfgCellInfo, CfgNetInfo, CfgPhyIo, PhyBackend, StackConfig, StackMode};
use tetra_core::{freqs::FreqInfo, ranges::SortedDisjointSsiRanges};

/// Creates a default config for testing. It can still be modified as needed
/// before passing it to the ComponentTest constructor
pub fn default_test_config_bs() -> StackConfig {
    let phy_io = default_phy_io();
    let net_info = default_net_info();
    let freq_info = FreqInfo::from_components(4, 1521, 0, false, 4, None).unwrap();
    let cell_info = default_cell_info(freq_info);

    // Put together components and return this proto config
    StackConfig {
        stack_mode: StackMode::Bs,
        debug_log: None,
        phy_io,
        net: net_info,
        cell: cell_info,
        brew: None,
    }
}

pub fn default_phy_io() -> CfgPhyIo {
    CfgPhyIo {
        backend: PhyBackend::None,
        dl_tx_file: None,
        ul_rx_file: None,
        ul_input_file: None,
        dl_input_file: None,
        soapysdr: None,
    }
}

pub fn default_net_info() -> CfgNetInfo {
    CfgNetInfo { mcc: 204, mnc: 1337 }
}

pub fn default_cell_info(freq_info: FreqInfo) -> CfgCellInfo {
    CfgCellInfo {
        colour_code: 1,
        location_area: 2,
        main_carrier: freq_info.carrier,
        freq_band: freq_info.band,
        freq_offset_hz: freq_info.freq_offset_hz,
        duplex_spacing_id: freq_info.duplex_spacing_id,
        custom_duplex_spacing: None,
        reverse_operation: freq_info.reverse_operation,
        neighbor_cell_broadcast: 0,
        late_entry_supported: false,
        subscriber_class: 65535, // All subscriber classes allowed
        registration: true,
        deregistration: true,
        priority_cell: false,
        no_minimum_mode: false,
        migration: false,
        system_wide_services: true,
        voice_service: true,
        circuit_mode_data_service: false,
        sndcp_service: false,
        aie_service: false,
        advanced_link: false,
        system_code: 3, // 3 = ETSI EN 300 392-2 V3.1.1
        sharing_mode: 0,
        ts_reserved_frames: 0,
        u_plane_dtx: false,
        frame_18_ext: false,
        local_ssi_ranges: SortedDisjointSsiRanges::from_vec_ssirange(vec![]),
        home_mode_display: None,
    }
}

pub fn default_test_config_ms() -> StackConfig {
    let mut config = default_test_config_bs();
    config.stack_mode = StackMode::Ms;
    config
}
