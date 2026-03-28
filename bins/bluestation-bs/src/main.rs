use clap::Parser;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use tetra_config::bluestation::{PhyBackend, SharedConfig, StackConfig, parsing};
use tetra_core::{TdmaTime, debug};
use tetra_entities::MessageRouter;
use tetra_entities::net_brew::entity::BrewEntity;
use tetra_entities::net_brew::new_websocket_transport;
use tetra_entities::net_telemetry::channel::{self, TelemetrySink};
use tetra_entities::net_telemetry::worker::TelemetryWorker;
use tetra_entities::net_telemetry::{TELEMETRY_HEARTBEAT_INTERVAL, TELEMETRY_HEARTBEAT_TIMEOUT, TELEMETRY_PROTOCOL_VERSION};
use tetra_entities::network::transports::websocket::{WebSocketTransport, WebSocketTransportConfig};
use tetra_entities::{
    cmce::cmce_bs::CmceBs,
    llc::llc_bs_ms::Llc,
    lmac::lmac_bs::LmacBs,
    mle::mle_bs::MleBs,
    mm::mm_bs::MmBs,
    phy::{components::soapy_dev::RxTxDevSoapySdr, phy_bs::PhyBs},
    sndcp::sndcp_bs::Sndcp,
    umac::umac_bs::UmacBs,
};

/// Load configuration file
fn load_config_from_toml(cfg_path: &str) -> StackConfig {
    match parsing::from_file(cfg_path) {
        Ok(c) => c,
        Err(e) => {
            println!("Failed to load configuration from {}: {}", cfg_path, e);
            std::process::exit(1);
        }
    }
}

fn build_telemetry_worker(cfg: &mut StackConfig) -> (Option<thread::JoinHandle<()>>, Option<channel::TelemetrySink>) {
    let telemetry_cfg = match cfg.telemetry {
        Some(ref t) => t.clone(),
        None => {
            return (None, None);
        }
    };

    let custom_root_certs = telemetry_cfg.ca_cert.as_ref().map(|path| {
        let der_bytes = std::fs::read(path).unwrap_or_else(|e| {
            eprintln!("Failed to read CA certificate from '{}': {}", path, e);
            std::process::exit(1);
        });
        vec![rustls::pki_types::CertificateDer::from(der_bytes)]
    });

    let ws_config = WebSocketTransportConfig {
        host: telemetry_cfg.host,
        port: telemetry_cfg.port,
        use_tls: telemetry_cfg.use_tls,
        digest_auth_credentials: None,
        basic_auth_credentials: telemetry_cfg.credentials,
        endpoint_path: "/".to_string(),
        subprotocol: Some(TELEMETRY_PROTOCOL_VERSION.to_string()),
        user_agent: format!("BlueStation/{}", tetra_core::STACK_VERSION),
        heartbeat_interval: TELEMETRY_HEARTBEAT_INTERVAL,
        heartbeat_timeout: TELEMETRY_HEARTBEAT_TIMEOUT,
        custom_root_certs,
    };

    let (sink, source) = channel::telemetry_channel();

    let handle = thread::spawn(move || {
        let transport = WebSocketTransport::new(ws_config);
        let mut worker = TelemetryWorker::new(source, transport);
        worker.run();
    });

    (Some(handle), Some(sink))
}

/// Start base station stack
fn build_bs_stack(cfg: &mut SharedConfig, telemetry_sink: Option<TelemetrySink>) -> MessageRouter {
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
    let mle = MleBs::new(cfg.clone());
    let mm = MmBs::new(cfg.clone(), telemetry_sink.clone());
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
    if let Some(ref brew_cfg) = cfg.config().brew {
        let transport = new_websocket_transport(brew_cfg);
        let brew_entity = BrewEntity::new(cfg.clone(), transport);
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
    about = "TETRA BlueStation base station stack",
    long_about = "Runs the TETRA BlueStation base station stack using the provided TOML configuration files"
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
    eprintln!("  Wouter Bokslag / Midnight Blue");
    eprintln!("  https://github.com/MidnightBlueLabs/tetra-bluestation");
    eprintln!("  Version: {}", tetra_core::STACK_VERSION);

    // Parse command-line arguments and load still-mutable config
    let args = Args::parse();
    let mut cfg = load_config_from_toml(&args.config);

    // Build telemetry worker
    let (_telemetry_join, telemetry_sink) = build_telemetry_worker(&mut cfg);

    // Build immutable, cheaply clonable SharedConfig and build the base station stack
    let mut shared_cfg = SharedConfig::from_parts(cfg, None);
    let _log_guard = debug::setup_logging_default(shared_cfg.config().debug_log.clone());
    let mut router = build_bs_stack(&mut shared_cfg, telemetry_sink);

    // Set up Ctrl+C handler for graceful shutdown
    let is_running = Arc::new(AtomicBool::new(true));
    let is_running_clone = is_running.clone();
    ctrlc::set_handler(move || {
        is_running_clone.store(false, Ordering::SeqCst);
    })
    .expect("failed to set Ctrl+C handler");

    // Start the stack
    router.run_stack(None, Some(is_running));

    // router drops here → entities are dropped, networked entities disconnect.
}
