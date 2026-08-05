use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use serde::Deserialize;
use toml::Value;

use crate::bluestation::{CellInfoDto, CfgControlDto, NetInfoDto, apply_control_patch, cell_dto_to_cfg, net_dto_to_cfg};

use super::config::{StackConfig, StackMode};
use super::sec_brew::{CfgBrewDto, apply_brew_patch};
use super::sec_telemetry::{CfgTelemetryDto, apply_telemetry_patch};
use super::{PhyIoDto, phy_dto_to_cfg};

/// Build `StackConfig` from a TOML configuration file
pub fn from_toml_str(toml_str: &str) -> Result<StackConfig, Box<dyn std::error::Error>> {
    let root: TomlConfigRoot = toml::from_str(toml_str)?;

    // Various sanity checks
    let expected_config_version = "0.6";
    if !root.config_version.eq(expected_config_version) {
        return Err(format!(
            "Unrecognized config_version: {}, expect {}",
            root.config_version, expected_config_version
        )
        .into());
    }
    if !root.extra.is_empty() {
        return Err(format!("Unrecognized top-level fields: {:?}", sorted_keys(&root.extra)).into());
    }

    if !root.phy_io.extra.is_empty() {
        return Err(format!("Unrecognized fields: phy_io::{:?}", sorted_keys(&root.phy_io.extra)).into());
    }
    if let Some(ref soapy) = root.phy_io.soapysdr {
        let extra_keys = sorted_keys(&soapy.extra);
        let extra_keys_filtered = extra_keys
            .iter()
            .filter(|key| !(key.starts_with("rx_gain_") || key.starts_with("tx_gain_")))
            .collect::<Vec<&&str>>();
        if !extra_keys_filtered.is_empty() {
            return Err(format!("Unrecognized fields: phy_io.soapysdr::{:?}", extra_keys_filtered).into());
        }
    }
    if !root.net_info.extra.is_empty() {
        return Err(format!("Unrecognized fields in net_info: {:?}", sorted_keys(&root.net_info.extra)).into());
    }
    if !root.cell_info.extra.is_empty() {
        return Err(format!("Unrecognized fields in cell_info: {:?}", sorted_keys(&root.cell_info.extra)).into());
    }

    // Optional brew section
    if let Some(ref brew) = root.brew {
        if !brew.extra.is_empty() {
            return Err(format!("Unrecognized fields in brew config: {:?}", sorted_keys(&brew.extra)).into());
        }
    }

    // Optional telemetry section
    if let Some(ref telemetry) = root.telemetry {
        if !telemetry.extra.is_empty() {
            return Err(format!("Unrecognized fields in telemetry config: {:?}", sorted_keys(&telemetry.extra)).into());
        }
    }

    // Optional control section (legacy name: command)
    if let Some(ref control) = root.command {
        if !control.extra.is_empty() {
            return Err(format!("Unrecognized fields in control config: {:?}", sorted_keys(&control.extra)).into());
        }
    }

    // Build config from required and optional values
    let mut cfg = StackConfig {
        stack_mode: root.stack_mode,
        debug_log: root.debug_log,
        phy_io: phy_dto_to_cfg(root.phy_io),
        net: net_dto_to_cfg(root.net_info),
        cell: cell_dto_to_cfg(root.cell_info),
        brew: None,
        telemetry: None,
        control: None,
    };

    if let Some(brew) = root.brew {
        cfg.brew = Some(apply_brew_patch(brew));
    }

    if let Some(telemetry) = root.telemetry {
        cfg.telemetry = Some(apply_telemetry_patch(telemetry)?);
    }

    if let Some(command) = root.command {
        cfg.control = Some(apply_control_patch(command)?);
    }

    Ok(cfg)
}

/// Build `SharedConfig` from any reader.
pub fn from_reader<R: Read>(reader: R) -> Result<StackConfig, Box<dyn std::error::Error>> {
    let mut contents = String::new();
    let mut reader = BufReader::new(reader);
    reader.read_to_string(&mut contents)?;
    from_toml_str(&contents)
}

/// Build `SharedConfig` from a file path.
pub fn from_file<P: AsRef<Path>>(path: P) -> Result<StackConfig, Box<dyn std::error::Error>> {
    let f = File::open(path)?;
    let r = BufReader::new(f);
    let cfg = from_reader(r)?;
    Ok(cfg)
}

fn sorted_keys(map: &HashMap<String, Value>) -> Vec<&str> {
    let mut v: Vec<&str> = map.keys().map(|s| s.as_str()).collect();
    v.sort_unstable();
    v
}

/// ----------------------- DTOs for input shape -----------------------

#[derive(Deserialize)]
struct TomlConfigRoot {
    config_version: String,
    stack_mode: StackMode,
    debug_log: Option<String>,

    phy_io: PhyIoDto,
    net_info: NetInfoDto,
    cell_info: CellInfoDto,

    brew: Option<CfgBrewDto>,
    telemetry: Option<CfgTelemetryDto>,
    #[serde(alias = "control")]
    command: Option<CfgControlDto>,

    #[serde(flatten)]
    extra: HashMap<String, Value>,
}

#[cfg(test)]
mod tests {
    use super::from_toml_str;

    #[test]
    fn parses_bladerf_example_with_canonical_soapy_fields() {
        let source = include_str!("../../../../example_config/bladerf.toml")
            .replace(
                "# device = \"driver=bladerf,serial=00000000000000000000000000000000\"",
                "device = \"driver=bladerf,serial=0123456789abcdef0123456789abcdef\"",
            )
            .replace("# rx_gain_rxvga1 = 29.0", "rx_gain_rxvga1 = 20.0")
            .replace("# tx_gain_txvga2 = 0.0", "tx_gain_txvga2 = 10.0");

        let config = from_toml_str(&source).expect("bladeRF example should parse");
        let soapy = config.phy_io.soapysdr.as_ref().expect("bladeRF example should enable SoapySDR");

        assert_eq!(
            soapy.device.as_deref(),
            Some("driver=bladerf,serial=0123456789abcdef0123456789abcdef")
        );
        assert_eq!(soapy.rx_ant.as_deref(), Some("RX"));
        assert_eq!(soapy.tx_ant.as_deref(), Some("TX"));
        assert_eq!(soapy.rx_gains.get("rxvga1"), Some(&20.0));
        assert_eq!(soapy.tx_gains.get("txvga2"), Some(&10.0));
    }

    #[test]
    fn parses_telemetry_and_control_service_configuration() {
        let source = format!(
            "{}\n\
             [telemetry]\n\
             host = \"127.0.0.1\"\n\
             port = 9001\n\
             use_tls = false\n\
             \n\
             [control]\n\
             host = \"127.0.0.1\"\n\
             port = 9002\n\
             use_tls = false\n",
            include_str!("../../../../example_config/bladerf.toml")
        );

        let config = from_toml_str(&source).expect("telemetry and control sections should parse");
        let telemetry = config.telemetry.expect("telemetry should be enabled");
        let control = config.control.expect("control should be enabled");

        assert_eq!(telemetry.host, "127.0.0.1");
        assert_eq!(telemetry.port, 9001);
        assert_eq!(control.host, "127.0.0.1");
        assert_eq!(control.port, 9002);
    }

    #[test]
    fn rejects_unknown_control_configuration_fields() {
        let source = format!(
            "{}\n\
             [control]\n\
             host = \"127.0.0.1\"\n\
             port = 9002\n\
             unsupported = true\n",
            include_str!("../../../../example_config/bladerf.toml")
        );

        let error = from_toml_str(&source).expect_err("unknown control fields should be rejected");
        assert!(error.to_string().contains("unsupported"));
    }
}
