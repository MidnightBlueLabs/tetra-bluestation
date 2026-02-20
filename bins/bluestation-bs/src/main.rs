use clap::Parser;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tetra_config::{PhyBackend, SharedConfig, StackMode, toml_config};
use tetra_core::{TdmaTime, debug};
use tetra_entities::MessageRouter;
use tetra_entities::brew::entity::BrewEntity;
use tetra_entities::brew::worker::BrewConfig;
use tetra_entities::{
    cmce::cmce_bs::CmceBs,
    llc::llc_bs_ms::Llc,
    lmac::lmac_bs::LmacBs,
    mle::mle_bs_ms::Mle,
    mm::mm_bs::MmBs,
    phy::{components::soapy_dev::RxTxDevSoapySdr, phy_bs::PhyBs},
    sndcp::sndcp_bs::Sndcp,
    umac::umac_bs::UmacBs,
};

/// Load configuration file
fn load_config_from_toml(cfg_path: &str) -> SharedConfig {
    match toml_config::from_file(cfg_path) {
        Ok(c) => c,
        Err(e) => {
            println!("Failed to load configuration from {}: {}", cfg_path, e);
            std::process::exit(1);
        }
    }
}

/// Start base station stack
fn build_bs_stack(cfg: &mut SharedConfig) -> MessageRouter {
    let mut router = MessageRouter::new(cfg.clone());

    // Add suitable Phy component based on PhyIo type
    match cfg.config().phy_io.backend {
        PhyBackend::SoapySdr => {
            let rxdev = RxTxDevSoapySdr::new(cfg);
            let phy = PhyBs::new(cfg.clone(), rxdev);
            router.register_entity(Box::new(phy));
        }
        _ => {
            panic!("Unsupported PhyIo type: {:?}", cfg.config().phy_io.backend);
        }
    }

    // Add remaining components
    let lmac = LmacBs::new(cfg.clone());
    let umac = UmacBs::new(cfg.clone());
    let llc = Llc::new(cfg.clone());
    let mle = Mle::new(cfg.clone());
    let mm = MmBs::new(cfg.clone());
    let sndcp = Sndcp::new(cfg.clone());
    let cmce = CmceBs::new(cfg.clone());
    router.register_entity(Box::new(lmac));
    router.register_entity(Box::new(umac));
    router.register_entity(Box::new(llc));
    router.register_entity(Box::new(mle));
    router.register_entity(Box::new(mm));
    router.register_entity(Box::new(sndcp));
    router.register_entity(Box::new(cmce));

    // Register Brew entity if enabled
    if let Some(brew_cfg) = cfg.config().brew.clone() {
        let brew_config = BrewConfig {
            host: brew_cfg.host,
            port: brew_cfg.port,
            tls: brew_cfg.tls,
            username: brew_cfg.username,
            password: brew_cfg.password,
            issi: brew_cfg.issi,
            groups: brew_cfg.groups,
            reconnect_delay: Duration::from_secs(brew_cfg.reconnect_delay_secs),
        };
        let brew_entity = BrewEntity::new(cfg.clone(), brew_config);
        router.register_entity(Box::new(brew_entity));
        eprintln!(" -> Brew/TetraPack integration enabled");
    }

    // Init network time
    router.set_dl_time(TdmaTime::default());

    router
}

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "TETRA BlueStation Stack",
    long_about = "Runs the TETRA BlueStation stack using the provided TOML configuration files"
)]

struct Args {
    /// Config file (required)
    #[arg(help = "TOML config with network/cell parameters")]
    config: String,
}

fn main() {
    eprintln!("░▀█▀░█▀▀░▀█▀░█▀▄░█▀█░░░░░█▀▄░█░░░█░█░█▀▀░█▀▀░▀█▀░█▀█░▀█▀░▀█▀░█▀█░█▀█");
    eprintln!("░░█░░█▀▀░░█░░█▀▄░█▀█░▄▄▄░█▀▄░█░░░█░█░█▀▀░▀▀█░░█░░█▀█░░█░░░█░░█░█░█░█");
    eprintln!("░░▀░░▀▀▀░░▀░░▀░▀░▀░▀░░░░░▀▀░░▀▀▀░▀▀▀░▀▀▀░▀▀▀░░▀░░▀░▀░░▀░░▀▀▀░▀▀▀░▀░▀\n");
    eprintln!("    Wouter Bokslag / Midnight Blue");
    eprintln!(" -> https://github.com/MidnightBlueLabs/tetra-bluestation");
    eprintln!(" -> https://midnightblue.nl\n");

    let args = Args::parse();
    let mut cfg = load_config_from_toml(&args.config);
    let _log_guard = debug::setup_logging_default(cfg.config().debug_log.clone());

    let mut router = match cfg.config().stack_mode {
        StackMode::Mon => {
            unimplemented!("Monitor mode is not implemented");
        }
        StackMode::Ms => {
            unimplemented!("MS mode is not implemented");
        }
        StackMode::Bs => build_bs_stack(&mut cfg),
    };

    // Set up Ctrl+C handler for graceful shutdown
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .expect("failed to set Ctrl+C handler");

    router.run_stack(None, Some(running));
    // router drops here → entities are dropped → BrewEntity::Drop fires teardown
}
