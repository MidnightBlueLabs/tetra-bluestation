use serde::Deserialize;
use std::sync::{Arc, RwLock};
use tetra_core::freqs::FreqInfo;

use crate::bluestation::{CfgCellInfo, CfgNetInfo, CfgPhyIo, PhyBackend, StackState};

use super::sec_brew::CfgBrew;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum StackMode {
    Bs,
    Ms,
    Mon,
}

#[derive(Debug, Clone)]
pub struct StackConfig {
    pub stack_mode: StackMode,
    pub debug_log: Option<String>,

    pub phy_io: CfgPhyIo,
    pub net: CfgNetInfo,
    pub cell: CfgCellInfo,

    /// Brew protocol (TetraPack/BrandMeister) configuration
    pub brew: Option<CfgBrew>,
}

impl StackConfig {
    /// Validate that all required configuration fields are properly set.
    pub fn validate(&self) -> Result<(), &str> {
        // Check input device settings
        match self.phy_io.backend {
            PhyBackend::SoapySdr => {
                if self.phy_io.soapysdr.is_none() {
                    return Err("soapysdr configuration must be provided for Soapysdr backend");
                };
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

        if self.cell.ms_txpwr_max_cell > 7 {
            return Err("ms_txpwr_max_cell must be 0-7 (3 bits)");
        }

        // Validate timezone if configured
        if let Some(ref tz) = self.cell.timezone {
            if tz.parse::<chrono_tz::Tz>().is_err() {
                return Err("Invalid IANA timezone name in cell.timezone");
            }
        }

        Ok(())
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
