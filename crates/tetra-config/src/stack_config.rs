use serde::Deserialize;
use std::sync::{Arc, RwLock};
use tetra_core::TimeslotAllocator;
use tetra_core::freqs::FreqInfo;
use tetra_core::ranges::SortedDisjointSsiRanges;

use crate::stack_config_brew::CfgBrew;

use super::stack_config_soapy::CfgSoapySdr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum StackMode {
    Bs,
    Ms,
    Mon,
}

/// The PHY layer backend type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum PhyBackend {
    Undefined,
    None,
    SoapySdr,
}

/// PHY layer I/O configuration
#[derive(Debug, Clone)]
pub struct CfgPhyIo {
    /// Backend type: Soapysdr, File, or None
    pub backend: PhyBackend,

    pub dl_tx_file: Option<String>,
    pub ul_rx_file: Option<String>,
    pub ul_input_file: Option<String>,
    pub dl_input_file: Option<String>,

    /// For Soapysdr backend: SoapySDR configuration
    pub soapysdr: Option<CfgSoapySdr>,
}

impl Default for CfgPhyIo {
    fn default() -> Self {
        Self {
            backend: PhyBackend::Undefined,
            dl_tx_file: None,
            ul_rx_file: None,
            ul_input_file: None,
            dl_input_file: None,
            soapysdr: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CfgNetInfo {
    /// 10 bits, from 18.4.2.1 D-MLE-SYNC
    pub mcc: u16,
    /// 14 bits, from 18.4.2.1 D-MLE-SYNC
    pub mnc: u16,
}

#[derive(Debug, Clone)]
pub struct CfgCellInfo {
    // 2 bits, from 18.4.2.1 D-MLE-SYNC
    pub neighbor_cell_broadcast: u8,
    // 2 bits, from 18.4.2.1 D-MLE-SYNC
    pub cell_load_ca: u8,
    // 1 bit, from 18.4.2.1 D-MLE-SYNC
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
}

impl Default for CfgCellInfo {
    fn default() -> Self {
        Self {
            freq_band: default_freq_band(),
            main_carrier: default_main_carrier(),
            freq_offset_hz: 0,
            duplex_spacing_id: 0,
            custom_duplex_spacing: None,
            reverse_operation: false,

            neighbor_cell_broadcast: 0,
            cell_load_ca: 0,
            late_entry_supported: false,
            location_area: 0,
            subscriber_class: 0,
            registration: true,
            deregistration: true,
            priority_cell: false,
            no_minimum_mode: false,
            migration: false,
            system_wide_services: false,
            voice_service: false,
            circuit_mode_data_service: false,
            sndcp_service: false,
            aie_service: false,
            advanced_link: false,

            system_code: 0,
            colour_code: 0,
            sharing_mode: 0,
            ts_reserved_frames: 0,
            u_plane_dtx: false,
            frame_18_ext: false,

            local_ssi_ranges: SortedDisjointSsiRanges::from_vec_ssirange(vec![]),
        }
    }
}

#[inline]
fn default_freq_band() -> u8 {
    4
}

#[inline]
fn default_main_carrier() -> u16 {
    1521
}

#[derive(Debug, Clone)]
pub struct StackConfig {
    pub stack_mode: StackMode,
    pub debug_log: Option<String>,

    pub phy_io: CfgPhyIo,

    /// Network info is REQUIRED - no default provided
    pub net: CfgNetInfo,

    pub cell: CfgCellInfo,

    /// Brew protocol (TetraPack/BrandMeister) configuration
    pub brew: Option<CfgBrew>,
}

impl StackConfig {
    pub fn new(mode: StackMode, mcc: u16, mnc: u16) -> Self {
        StackConfig {
            stack_mode: mode,
            debug_log: None,
            phy_io: CfgPhyIo::default(),
            net: CfgNetInfo { mcc, mnc },
            cell: CfgCellInfo::default(),

            brew: None,
        }
    }

    /// Validate that all required configuration fields are properly set.
    pub fn validate(&self) -> Result<(), &str> {
        // Check input device settings
        match self.phy_io.backend {
            PhyBackend::SoapySdr => {
                let Some(ref soapy_cfg) = self.phy_io.soapysdr else {
                    return Err("soapysdr configuration must be provided for Soapysdr backend");
                };

                // Validate that exactly one hardware configuration is present
                let config_count = [
                    soapy_cfg.io_cfg.iocfg_usrpb2xx.is_some(),
                    soapy_cfg.io_cfg.iocfg_limesdr.is_some(),
                    soapy_cfg.io_cfg.iocfg_sxceiver.is_some(),
                ]
                .iter()
                .filter(|&&x| x)
                .count();
                if config_count != 1 {
                    return Err(
                        "soapysdr backend requires exactly one hardware configuration (iocfg_usrpb2xx, iocfg_limesdr, or iocfg_sxceiver)",
                    );
                }
            }
            PhyBackend::None => {} // For testing
            PhyBackend::Undefined => {
                return Err("phy_io backend must be defined");
            }
        };

        // Sanity check on main carrier property fields in SYSINFO
        if self.phy_io.backend == PhyBackend::SoapySdr {
            let soapy_cfg = self
                .phy_io
                .soapysdr
                .as_ref()
                .expect("SoapySdr config must be set for SoapySdr PhyIo");

            // let Ok(freqinfo) = FreqInfo::from_dlul_freqs(soapy_cfg.dl_freq as u32, soapy_cfg.ul_freq as u32) else {
            //     return Err("Invalid PhyIo DL/UL frequencies (can't map to TETRA SYSINFO settings)");
            // };
            let Ok(freq_info) = FreqInfo::from_components(
                self.cell.freq_band,
                self.cell.main_carrier,
                self.cell.freq_offset_hz,
                self.cell.reverse_operation,
                self.cell.duplex_spacing_id,
                self.cell.custom_duplex_spacing,
            ) else {
                return Err("Invalid cell info frequency settings");
            };

            let (dlfreq, ulfreq) = freq_info.get_freqs();

            println!("    {:?}", freq_info);
            println!("    Derived DL freq: {} Hz, UL freq: {} Hz\n", dlfreq, ulfreq);

            if soapy_cfg.dl_freq as u32 != dlfreq {
                return Err("PhyIo DlFrequency does not match computed FreqInfo");
            };
            if soapy_cfg.ul_freq as u32 != ulfreq {
                return Err("PhyIo UlFrequency does not match computed FreqInfo");
            };
        }

        Ok(())
    }
}

/// Mutable, stack-editable state (mutex-protected).
#[derive(Debug, Clone)]
pub struct StackState {
    pub cell_load_ca: u8,
    pub timeslot_alloc: TimeslotAllocator,
    /// Backhaul/network connection to SwMI (e.g., Brew/TetraPack). False -> fallback mode.
    pub network_connected: bool,
}

impl Default for StackState {
    fn default() -> Self {
        Self {
            cell_load_ca: 0,
            timeslot_alloc: TimeslotAllocator::default(),
            network_connected: false,
        }
    }
}

/// Global shared configuration: immutable config + mutable state.
#[derive(Clone)]
pub struct SharedConfig {
    /// Read-only configuration (immutable after construction).
    cfg: Arc<StackConfig>,
    /// Mutable state guarded with RwLock (write by the stack, read by others).
    state: Arc<RwLock<StackState>>,
}

impl SharedConfig {
    pub fn new(mode: StackMode, mcc: u16, mnc: u16) -> Self {
        Self::from_config(StackConfig::new(mode, mcc, mnc))
    }

    pub fn from_config(cfg: StackConfig) -> Self {
        Self::from_parts(cfg, StackState::default())
    }

    pub fn from_parts(cfg: StackConfig, state: StackState) -> Self {
        // Check config for validity before returning the SharedConfig object
        match cfg.validate() {
            Ok(_) => {}
            Err(e) => panic!("Invalid stack configuration: {}", e),
        }

        Self {
            cfg: Arc::new(cfg),
            state: Arc::new(RwLock::new(state)),
        }
    }

    /// Access immutable config.
    pub fn config(&self) -> Arc<StackConfig> {
        Arc::clone(&self.cfg)
    }

    /// Read guard for mutable state.
    pub fn state_read(&self) -> std::sync::RwLockReadGuard<'_, StackState> {
        self.state.read().expect("StackState RwLock blocked")
    }

    /// Write guard for mutable state.
    pub fn state_write(&self) -> std::sync::RwLockWriteGuard<'_, StackState> {
        self.state.write().expect("StackState RwLock blocked")
    }
}
