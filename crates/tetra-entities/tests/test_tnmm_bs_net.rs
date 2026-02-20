mod common;

use common::{ComponentTest, default_test_config};
use std::time::Duration;
use tetra_config::StackMode;
use tetra_core::debug::setup_logging_verbose;
use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{Sap, TdmaTime};
use tetra_entities::network::netentity::NetEntity;
use tetra_entities::network::transports::NetworkAddress;
use tetra_entities::network::transports::quic::QuicTransport;
use tetra_entities::network::transports::tcp::TcpTransport;
use tetra_entities::tnmm_net::net_entity_tnmm_worker::NetEntityTnmmWorker;
use tetra_saps::sapmsg::{SapMsg, SapMsgInner};
use tetra_saps::tnmm::TnmmTestDemand;

fn build_test(use_quic: bool) -> ComponentTest {
    setup_logging_verbose();
    let config = default_test_config(StackMode::Bs);
    let ts = TdmaTime::default().add_timeslots(2);
    let mut test = ComponentTest::new(config, Some(ts));
    let components = vec![
        // TetraEntity::Umac,
        // TetraEntity::Llc,
        // TetraEntity::Mle,
    ];
    let sinks: Vec<TetraEntity> = vec![TetraEntity::Mm];
    test.populate_entities(components, sinks);

    if use_quic {
        // Insert QUIC-based TnmmNetEntity
        let network_endpoint = NetworkAddress::Udp {
            host: "127.0.0.1".to_string(),
            port: 4433,
        };
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime");
        let transport = QuicTransport::new(
            network_endpoint,
            Duration::from_secs(5),
            true, // skip_cert_verification - for testing
            runtime,
        )
        .expect("Failed to create QUIC transport");
        let tnmm = NetEntity::<NetEntityTnmmWorker<QuicTransport>>::new(
            test.get_shared_config(),
            TetraEntity::User,
            TetraEntity::Mm,
            Sap::TnmmSap,
            transport,
        )
        .expect("Failed to create TNMM QUIC entity");
        test.register_entity(tnmm);
    } else {
        // Insert simple TCP TnmmNetEntity
        let network_endpoint = NetworkAddress::Tcp {
            host: "127.0.0.1".to_string(),
            port: 8443,
        };
        let transport = TcpTransport::new(network_endpoint, Duration::from_secs(5), Duration::from_secs(30));
        let tnmm = NetEntity::<NetEntityTnmmWorker<TcpTransport>>::new(
            test.get_shared_config(),
            TetraEntity::User,
            TetraEntity::Mm,
            Sap::TnmmSap,
            transport,
        )
        .expect("Failed to create TNMM entity");
        test.register_entity(tnmm);
    }
    test
}

fn run_test(mut test: ComponentTest) {
    // Build our test message
    let sapmsg = SapMsg {
        sap: Sap::TnmmSap,
        src: TetraEntity::Mm,
        dest: TetraEntity::User,
        dltime: TdmaTime::default(),
        msg: SapMsgInner::TnmmTestDemand(TnmmTestDemand { issi: 1001 }),
    };

    // Uncomment below to witness socket timeout and reconnect
    // std::thread::sleep(std::time::Duration::from_secs(70));

    // Submit and process our message
    test.submit_message(sapmsg);
    test.run_stack(Some(1));
    std::thread::sleep(std::time::Duration::from_secs(1));
    tracing::info!("Sink messages: {:?}", test.dump_sinks());
    test.run_stack(Some(1));
    // std::thread::sleep(std::time::Duration::from_secs(1));
    tracing::info!("Sink messages: {:?}", test.dump_sinks());
    test.run_stack(Some(1));
    // std::thread::sleep(std::time::Duration::from_secs(1));
    tracing::info!("Sink messages: {:?}", test.dump_sinks());
    test.run_stack(Some(1));
    // std::thread::sleep(std::time::Duration::from_secs(1));
    tracing::info!("Sink messages: {:?}", test.dump_sinks());
}

#[test]
fn test_tnmm_netentity_tcp() {
    let test = build_test(false);
    run_test(test);
}

#[test]
fn test_tnmm_netentity_quic() {
    let test = build_test(true);
    run_test(test);
}
