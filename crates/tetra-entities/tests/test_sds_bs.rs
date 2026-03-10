mod common;

use std::time::Duration;

use tetra_config::bluestation::{CfgBrew, StackMode};
use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{BitBuffer, Sap, SsiType, TdmaTime, TetraAddress, debug};
use tetra_pdus::cmce::enums::party_type_identifier::PartyTypeIdentifier;
use tetra_pdus::cmce::pdus::u_sds_data::USdsData;
use tetra_pdus::cmce::pdus::u_status::UStatus;
use tetra_saps::control::sds::CmceSdsData;
use tetra_saps::lcmc::LcmcMleUnitdataInd;
use tetra_saps::sapmsg::{SapMsg, SapMsgInner};

use crate::common::ComponentTest;

/// Helper: register a subscriber ISSI in the StackState subscriber registry
fn register_subscriber(test: &mut ComponentTest, issi: u32) {
    test.config.state_write().subscribers.register(issi);
}

/// Helper: affiliate a subscriber with a GSSI in the StackState subscriber registry
fn affiliate_subscriber(test: &mut ComponentTest, issi: u32, gssi: u32) {
    test.config.state_write().subscribers.affiliate(issi, gssi);
}

/// Helper: build a U-SDS-DATA message from a source ISSI to a dest SSI with 16-bit payload
fn build_u_sds_data_msg(dltime: TdmaTime, source_issi: u32, dest_ssi: u32, payload: u16) -> SapMsg {
    let u_sds = USdsData {
        area_selection: 0,
        called_party_type_identifier: PartyTypeIdentifier::Ssi,
        called_party_short_number_address: None,
        called_party_ssi: Some(dest_ssi as u64),
        called_party_extension: None,
        short_data_type_identifier: 0,
        user_defined_data_1: Some(payload as u64),
        user_defined_data_2: None,
        user_defined_data_3: None,
        length_indicator: None,
        user_defined_data_4: None,
        external_subscriber_number: None,
        dm_ms_address: None,
    };

    let mut sdu = BitBuffer::new_autoexpand(80);
    u_sds.to_bitbuf(&mut sdu).expect("Failed to serialize U-SDS-DATA");
    sdu.seek(0);

    SapMsg {
        sap: Sap::LcmcSap,
        src: TetraEntity::Mle,
        dest: TetraEntity::Cmce,
        dltime,
        msg: SapMsgInner::LcmcMleUnitdataInd(LcmcMleUnitdataInd {
            sdu,
            handle: 1,
            endpoint_id: 1,
            link_id: 1,
            received_tetra_address: TetraAddress::new(source_issi, SsiType::Issi),
            chan_change_resp_req: false,
            chan_change_handle: None,
        }),
    }
}

/// Count D-SDS-DATA messages (LcmcMleUnitdataReq to Mle) in sink output
fn count_d_sds_data(msgs: &[SapMsg]) -> usize {
    msgs.iter()
        .filter(|m| m.dest == TetraEntity::Mle && matches!(&m.msg, SapMsgInner::LcmcMleUnitdataReq(_)))
        .count()
}

/// Count CmceSdsData messages to Brew in sink output
fn count_brew_sds(msgs: &[SapMsg]) -> usize {
    msgs.iter()
        .filter(|m| m.dest == TetraEntity::Brew && matches!(&m.msg, SapMsgInner::CmceSdsData(_)))
        .count()
}

#[test]
fn test_sds_local_delivery() {
    debug::setup_logging_verbose();

    let dltime = TdmaTime { h: 0, m: 1, f: 1, t: 1 };
    let mut test = ComponentTest::new(StackMode::Bs, Some(dltime));

    let components = vec![TetraEntity::Cmce];
    let sinks = vec![TetraEntity::Mle, TetraEntity::Brew];
    test.populate_entities(components, sinks);

    // Register dest ISSI in StackState
    register_subscriber(&mut test, 2000001);

    // Send U-SDS-DATA from source ISSI to registered dest ISSI
    let msg = build_u_sds_data_msg(dltime, 1000001, 2000001, 0xABCD);
    test.submit_message(msg);
    test.run_stack(Some(1));

    let sink_msgs = test.dump_sinks();
    let d_sds_count = count_d_sds_data(&sink_msgs);
    assert_eq!(d_sds_count, 1, "Expected 1 D-SDS-DATA at Mle sink for local delivery");

    // Verify the address is ISSI
    for m in &sink_msgs {
        if m.dest == TetraEntity::Mle {
            if let SapMsgInner::LcmcMleUnitdataReq(ref prim) = m.msg {
                assert_eq!(prim.main_address.ssi, 2000001);
                assert_eq!(prim.main_address.ssi_type, SsiType::Issi);
            }
        }
    }
}

#[test]
fn test_sds_brew_forward() {
    debug::setup_logging_verbose();

    let dltime = TdmaTime { h: 0, m: 1, f: 1, t: 1 };
    let mut config = ComponentTest::get_default_test_config(StackMode::Bs);
    config.brew = Some(CfgBrew {
        host: "test.local".into(),
        port: 3000,
        tls: false,
        username: None,
        password: None,
        reconnect_delay: Duration::from_secs(1),
        jitter_initial_latency_frames: 0,
        whitelisted_ssis: None,
    });
    let mut test = ComponentTest::from_config(config, Some(dltime));

    let components = vec![TetraEntity::Cmce];
    let sinks = vec![TetraEntity::Mle, TetraEntity::Brew];
    test.populate_entities(components, sinks);

    // Do NOT register dest ISSI — should forward to Brew
    let msg = build_u_sds_data_msg(dltime, 1000001, 5000001, 0x1234);
    test.submit_message(msg);
    test.run_stack(Some(1));

    let sink_msgs = test.dump_sinks();
    let brew_count = count_brew_sds(&sink_msgs);
    assert!(brew_count > 0, "Expected CmceSdsData at Brew sink for non-local ISSI");

    let d_sds_count = count_d_sds_data(&sink_msgs);
    assert_eq!(d_sds_count, 0, "Should not deliver locally when dest is not registered");
}

#[test]
fn test_sds_from_brew_to_local() {
    debug::setup_logging_verbose();

    let dltime = TdmaTime { h: 0, m: 1, f: 1, t: 1 };
    let mut test = ComponentTest::new(StackMode::Bs, Some(dltime));

    let components = vec![TetraEntity::Cmce];
    let sinks = vec![TetraEntity::Mle, TetraEntity::Brew];
    test.populate_entities(components, sinks);

    // Register dest ISSI in StackState
    register_subscriber(&mut test, 2000001);

    // Submit CmceSdsData from Brew on Control SAP
    let msg = SapMsg {
        sap: Sap::Control,
        src: TetraEntity::Brew,
        dest: TetraEntity::Cmce,
        dltime,
        msg: SapMsgInner::CmceSdsData(CmceSdsData {
            source_issi: 3000001,
            dest_issi: 2000001,
            short_data_type_identifier: 0,
            data: vec![0xCA, 0xFE],
            length_bits: 16,
        }),
    };
    test.submit_message(msg);
    test.run_stack(Some(1));

    let sink_msgs = test.dump_sinks();
    let d_sds_count = count_d_sds_data(&sink_msgs);
    assert_eq!(d_sds_count, 1, "Expected D-SDS-DATA at Mle sink from Brew");
}

#[test]
fn test_sds_from_brew_unregistered() {
    debug::setup_logging_verbose();

    let dltime = TdmaTime { h: 0, m: 1, f: 1, t: 1 };
    let mut test = ComponentTest::new(StackMode::Bs, Some(dltime));

    let components = vec![TetraEntity::Cmce];
    let sinks = vec![TetraEntity::Mle, TetraEntity::Brew];
    test.populate_entities(components, sinks);

    // Do NOT register dest ISSI
    let msg = SapMsg {
        sap: Sap::Control,
        src: TetraEntity::Brew,
        dest: TetraEntity::Cmce,
        dltime,
        msg: SapMsgInner::CmceSdsData(CmceSdsData {
            source_issi: 3000001,
            dest_issi: 9999999,
            short_data_type_identifier: 0,
            data: vec![0xDE, 0xAD],
            length_bits: 16,
        }),
    };
    test.submit_message(msg);
    test.run_stack(Some(1));

    let sink_msgs = test.dump_sinks();
    let d_sds_count = count_d_sds_data(&sink_msgs);
    assert_eq!(d_sds_count, 0, "Should not deliver D-SDS-DATA when dest is not registered");
}

#[test]
fn test_sds_group_delivery() {
    debug::setup_logging_verbose();

    let dltime = TdmaTime { h: 0, m: 1, f: 1, t: 1 };
    let mut test = ComponentTest::new(StackMode::Bs, Some(dltime));

    let components = vec![TetraEntity::Cmce];
    let sinks = vec![TetraEntity::Mle, TetraEntity::Brew];
    test.populate_entities(components, sinks);

    let gssi = 100;

    // Register 3 ISSIs and affiliate them with the GSSI in StackState
    for issi in [1000001, 1000002, 1000003] {
        register_subscriber(&mut test, issi);
        affiliate_subscriber(&mut test, issi, gssi);
    }

    // Send U-SDS-DATA to the GSSI
    let msg = build_u_sds_data_msg(dltime, 1000001, gssi, 0xBEEF);
    test.submit_message(msg);
    test.run_stack(Some(1));

    let sink_msgs = test.dump_sinks();
    let d_sds_count = count_d_sds_data(&sink_msgs);
    assert_eq!(d_sds_count, 1, "Expected exactly 1 GSSI-addressed D-SDS-DATA (not per-member)");

    // Verify the address is GSSI
    for m in &sink_msgs {
        if m.dest == TetraEntity::Mle {
            if let SapMsgInner::LcmcMleUnitdataReq(ref prim) = m.msg {
                assert_eq!(prim.main_address.ssi, gssi);
                assert_eq!(prim.main_address.ssi_type, SsiType::Gssi);
            }
        }
    }
}

#[test]
fn test_u_status_forwarded_as_d_status() {
    debug::setup_logging_verbose();

    let dltime = TdmaTime { h: 0, m: 1, f: 1, t: 1 };
    let mut test = ComponentTest::new(StackMode::Bs, Some(dltime));

    let components = vec![TetraEntity::Cmce];
    let sinks = vec![TetraEntity::Mle, TetraEntity::Brew];
    test.populate_entities(components, sinks);

    // Register both source and dest
    register_subscriber(&mut test, 1000001);
    register_subscriber(&mut test, 2000001);

    // Build a U-STATUS PDU from 1000001 to 2000001 with pre-coded status 0x8210
    let u_status = UStatus {
        area_selection: 0,
        called_party_type_identifier: PartyTypeIdentifier::Ssi,
        called_party_short_number_address: None,
        called_party_ssi: Some(2000001),
        called_party_extension: None,
        pre_coded_status: 0x8210,
        external_subscriber_number: None,
        dm_ms_address: None,
    };

    let mut sdu = BitBuffer::new_autoexpand(80);
    u_status.to_bitbuf(&mut sdu).expect("Failed to serialize U-STATUS");
    sdu.seek(0);

    let msg = SapMsg {
        sap: Sap::LcmcSap,
        src: TetraEntity::Mle,
        dest: TetraEntity::Cmce,
        dltime,
        msg: SapMsgInner::LcmcMleUnitdataInd(LcmcMleUnitdataInd {
            sdu,
            handle: 1,
            endpoint_id: 1,
            link_id: 1,
            received_tetra_address: TetraAddress::new(1000001, SsiType::Issi),
            chan_change_resp_req: false,
            chan_change_handle: None,
        }),
    };
    test.submit_message(msg);
    test.run_stack(Some(1));

    let sink_msgs = test.dump_sinks();

    // Should produce exactly 1 D-STATUS at Mle sink
    let mle_msgs: Vec<_> = sink_msgs
        .iter()
        .filter(|m| m.dest == TetraEntity::Mle && matches!(&m.msg, SapMsgInner::LcmcMleUnitdataReq(_)))
        .collect();
    assert_eq!(mle_msgs.len(), 1, "Expected 1 D-STATUS at Mle sink");

    // Verify addressed to 2000001
    if let SapMsgInner::LcmcMleUnitdataReq(ref prim) = mle_msgs[0].msg {
        assert_eq!(prim.main_address.ssi, 2000001);
        assert_eq!(prim.main_address.ssi_type, SsiType::Issi);
    }
}

#[test]
fn test_u_status_unregistered_dest_dropped() {
    debug::setup_logging_verbose();

    let dltime = TdmaTime { h: 0, m: 1, f: 1, t: 1 };
    let mut test = ComponentTest::new(StackMode::Bs, Some(dltime));

    let components = vec![TetraEntity::Cmce];
    let sinks = vec![TetraEntity::Mle, TetraEntity::Brew];
    test.populate_entities(components, sinks);

    // Only register source, NOT dest
    register_subscriber(&mut test, 1000001);

    let u_status = UStatus {
        area_selection: 0,
        called_party_type_identifier: PartyTypeIdentifier::Ssi,
        called_party_short_number_address: None,
        called_party_ssi: Some(9999999),
        called_party_extension: None,
        pre_coded_status: 0x8210,
        external_subscriber_number: None,
        dm_ms_address: None,
    };

    let mut sdu = BitBuffer::new_autoexpand(80);
    u_status.to_bitbuf(&mut sdu).expect("Failed to serialize U-STATUS");
    sdu.seek(0);

    let msg = SapMsg {
        sap: Sap::LcmcSap,
        src: TetraEntity::Mle,
        dest: TetraEntity::Cmce,
        dltime,
        msg: SapMsgInner::LcmcMleUnitdataInd(LcmcMleUnitdataInd {
            sdu,
            handle: 1,
            endpoint_id: 1,
            link_id: 1,
            received_tetra_address: TetraAddress::new(1000001, SsiType::Issi),
            chan_change_resp_req: false,
            chan_change_handle: None,
        }),
    };
    test.submit_message(msg);
    test.run_stack(Some(1));

    let sink_msgs = test.dump_sinks();
    let d_status_count = count_d_sds_data(&sink_msgs);
    assert_eq!(d_status_count, 0, "Should not deliver D-STATUS when dest is not registered");
}
