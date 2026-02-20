mod common;

use common::{ComponentTest, default_test_config};
use tetra_config::stack_config_soapy::{CfgSoapySdr, LimeSdrCfg, UsrpB2xxCfg};
use tetra_config::{PhyBackend, SharedConfig, StackMode};
use tetra_core::debug;
use tetra_entities::llc::llc_bs_ms::Llc;
use tetra_entities::lmac::lmac_bs::LmacBs;
use tetra_entities::mle::mle_bs_ms::Mle;
use tetra_entities::mm::mm_bs::MmBs;
use tetra_entities::phy::components::soapy_dev::RxTxDevSoapySdr;
use tetra_entities::phy::phy_bs::PhyBs;
use tetra_entities::umac::umac_bs::UmacBs;
use tetra_entities::{MessageRouter, TetraEntityTrait};

const DL_FREQ: f64 = 438.025e6;
const UL_FREQ: f64 = DL_FREQ - 5.0e6;

/// Builds a message router with the necessary components for a base station stack.
#[allow(dead_code)]
fn build_bs_stack_components(shared_config: &SharedConfig, phy_component: Box<dyn TetraEntityTrait>) -> MessageRouter {
    let mut router = MessageRouter::new(shared_config.clone());

    let lmac = LmacBs::new(shared_config.clone());
    let umac = UmacBs::new(shared_config.clone());
    let llc = Llc::new(shared_config.clone());
    let mle = Mle::new(shared_config.clone());
    let mm: MmBs = MmBs::new(shared_config.clone());

    router.register_entity(phy_component);
    router.register_entity(Box::new(lmac));
    router.register_entity(Box::new(umac));
    router.register_entity(Box::new(llc));
    router.register_entity(Box::new(mle));
    router.register_entity(Box::new(mm));

    router
}

/// Calls tick() on all components and subsequently delivers all messages
/// Either infinitely (num_ticks is None) or for a specified number of ticks.
#[allow(dead_code)]
fn run_stack(_config: &mut SharedConfig, router: &mut MessageRouter, num_ticks: Option<u64>) {
    let mut ticks: u64 = 0;
    loop {
        router.tick_start();
        router.deliver_all_messages();
        ticks += 1;
        if let Some(num_ticks) = num_ticks {
            if ticks >= num_ticks {
                break;
            }
        }
    }
}

#[test]
#[ignore] // Requires LimeSDR hardware
fn test_limesdr_bs() {
    // Setup logging and make default stack configuration
    debug::setup_logging_default(None);
    let mut raw_config = default_test_config(StackMode::Bs);

    // Update default config to suit our needs
    raw_config.phy_io.backend = PhyBackend::SoapySdr;
    let mut soapy_cfg = CfgSoapySdr::default();
    soapy_cfg.ul_freq = UL_FREQ;
    soapy_cfg.dl_freq = DL_FREQ;
    soapy_cfg.io_cfg.iocfg_limesdr = Some(LimeSdrCfg {
        rx_ant: None,
        tx_ant: None,
        rx_gain_lna: None,
        rx_gain_tia: None,
        rx_gain_pga: None,
        tx_gain_pad: None,
        tx_gain_iamp: None,
    });
    raw_config.phy_io.soapysdr = Some(soapy_cfg);

    let mut test = ComponentTest::new(raw_config, None);

    // Create PHY and insert it into the message router
    let rxdev = RxTxDevSoapySdr::new(&test.config);
    let phy = PhyBs::new(test.config.clone(), rxdev);
    test.register_entity(phy);
    test.run_stack(None);
}

#[test]
#[ignore] // Requires USRP hardware
fn test_usrp_bs() {
    // Setup logging and make default stack configuration
    debug::setup_logging_default(None);
    let mut raw_config = default_test_config(StackMode::Bs);

    // Update default config to suit our needs
    raw_config.phy_io.backend = PhyBackend::SoapySdr;
    let mut soapy_cfg = CfgSoapySdr::default();
    soapy_cfg.ul_freq = UL_FREQ;
    soapy_cfg.dl_freq = DL_FREQ;
    soapy_cfg.io_cfg.iocfg_usrpb2xx = Some(UsrpB2xxCfg {
        rx_ant: None,
        tx_ant: None,
        rx_gain_pga: None,
        tx_gain_pga: None,
    });
    raw_config.phy_io.soapysdr = Some(soapy_cfg);

    let mut test = ComponentTest::new(raw_config, None);

    // Create PHY and insert it into the message router
    let rxdev = RxTxDevSoapySdr::new(&test.config);
    let phy = PhyBs::new(test.config.clone(), rxdev);
    test.register_entity(phy);
    test.run_stack(None);
}
