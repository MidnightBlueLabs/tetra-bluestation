use std::collections::HashMap;

use serde::Deserialize;
use toml::Value;

use crate::bluestation::{CfgLimeSdr, CfgPlutoSdr, CfgSoapySdr, CfgSxCeiver, CfgUsrpB2xx, SoapySdrDto, SoapySdrIoCfg};

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

#[derive(Deserialize)]
pub struct PhyIoDto {
    pub backend: PhyBackend,

    pub dl_tx_file: Option<String>,
    pub ul_rx_file: Option<String>,
    pub ul_input_file: Option<String>,
    pub dl_input_file: Option<String>,

    pub soapysdr: Option<SoapySdrDto>,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

pub fn phy_dto_to_cfg(src: PhyIoDto) -> CfgPhyIo {
    let soapysdr = src.soapysdr.map(|soapy_dto| {
        let ppm_base = soapy_dto.ppm_err.unwrap_or(0.0);
        let mut soapy_cfg = CfgSoapySdr {
            ul_freq: soapy_dto.rx_freq,
            dl_freq: soapy_dto.tx_freq,
            ppm_err: ppm_base,
            ppm_err_tx: soapy_dto.ppm_err_tx.unwrap_or(ppm_base),
            ppm_err_rx: soapy_dto.ppm_err_rx.unwrap_or(ppm_base),
            tx_freq_offset: soapy_dto.tx_freq_offset.unwrap_or(0.0),
            io_cfg: SoapySdrIoCfg::default(),
        };

        if let Some(usrp_dto) = soapy_dto.iocfg_usrpb2xx {
            soapy_cfg.io_cfg.iocfg_usrpb2xx = Some(CfgUsrpB2xx {
                rx_ant: usrp_dto.rx_ant,
                tx_ant: usrp_dto.tx_ant,
                rx_gain_pga: usrp_dto.rx_gain_pga,
                tx_gain_pga: usrp_dto.tx_gain_pga,
            });
        }
        if let Some(lime_dto) = soapy_dto.iocfg_limesdr {
            soapy_cfg.io_cfg.iocfg_limesdr = Some(CfgLimeSdr {
                rx_ant: lime_dto.rx_ant,
                tx_ant: lime_dto.tx_ant,
                rx_gain_lna: lime_dto.rx_gain_lna,
                rx_gain_tia: lime_dto.rx_gain_tia,
                rx_gain_pga: lime_dto.rx_gain_pga,
                tx_gain_pad: lime_dto.tx_gain_pad,
                tx_gain_iamp: lime_dto.tx_gain_iamp,
            });
        }
        if let Some(sx_dto) = soapy_dto.iocfg_sxceiver {
            soapy_cfg.io_cfg.iocfg_sxceiver = Some(CfgSxCeiver {
                rx_ant: sx_dto.rx_ant,
                tx_ant: sx_dto.tx_ant,
                rx_gain_lna: sx_dto.rx_gain_lna,
                rx_gain_pga: sx_dto.rx_gain_pga,
                tx_gain_dac: sx_dto.tx_gain_dac,
                tx_gain_mixer: sx_dto.tx_gain_mixer,
            });
        }
        if let Some(pluto_dto) = soapy_dto.iocfg_plutosdr {
            soapy_cfg.io_cfg.iocfg_plutosdr = Some(CfgPlutoSdr {
                rx_ant: pluto_dto.rx_ant,
                tx_ant: pluto_dto.tx_ant,
                rx_gain_pga: pluto_dto.rx_gain_pga,
                tx_gain_pga: pluto_dto.tx_gain_pga,
                uri: pluto_dto.uri,
                timestamp_every: pluto_dto.timestamp_every,
                usb_direct: pluto_dto.usb_direct,
                loopback: pluto_dto.loopback,
            });
        }

        soapy_cfg
    });

    CfgPhyIo {
        backend: src.backend,
        dl_tx_file: src.dl_tx_file,
        ul_rx_file: src.ul_rx_file,
        ul_input_file: src.ul_input_file,
        dl_input_file: src.dl_input_file,
        soapysdr,
    }
}
