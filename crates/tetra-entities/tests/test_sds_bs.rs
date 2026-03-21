mod common;

use std::time::Duration;

use tetra_config::bluestation::{CfgBrew, CfgHomeModeDisplay, HomeModeDisplaySdsTextCodingScheme, StackMode};
use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{BitBuffer, Sap, SsiType, TdmaTime, TetraAddress, debug};
use tetra_pdus::cmce::enums::party_type_identifier::PartyTypeIdentifier;
use tetra_pdus::cmce::enums::pre_coded_status::PreCodedStatus;
use tetra_pdus::cmce::pdus::d_sds_data::DSdsData;
use tetra_pdus::cmce::pdus::u_sds_data::USdsData;
use tetra_pdus::cmce::pdus::u_status::UStatus;
use tetra_saps::control::enums::sds_user_data::SdsUserData;
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
        user_defined_data: SdsUserData::Type1(payload),
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

fn home_mode_cfg(
    source_issi: u32,
    interval_multiframes: u32,
    protocol_id: u8,
    text_coding_scheme: HomeModeDisplaySdsTextCodingScheme,
    text: &str,
) -> CfgHomeModeDisplay {
    CfgHomeModeDisplay {
        source_issi,
        interval_multiframes,
        protocol_id,
        text_coding_scheme,
        text: text.to_string(),
    }
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
        feature_sds_enabled: true,
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
            user_defined_data: SdsUserData::Type1(0xCAFE),
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
            user_defined_data: SdsUserData::Type1(0xDEAD),
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
fn test_periodic_home_mode_display_sds_broadcast() {
    debug::setup_logging_verbose();

    let dltime = TdmaTime { h: 0, m: 1, f: 1, t: 1 };
    let mut config = ComponentTest::get_default_test_config(StackMode::Bs);
    config.cell.home_mode_display = Some(home_mode_cfg(
        1000001,
        2, // every 144 ticks (2 multiframes)
        220,
        HomeModeDisplaySdsTextCodingScheme::LATIN,
        "HOME MODE",
    ));
    let mut test = ComponentTest::from_config(config, Some(dltime));

    let components = vec![TetraEntity::Cmce];
    let sinks = vec![TetraEntity::Mle, TetraEntity::Brew];
    test.populate_entities(components, sinks);

    // Startup delay is hardcoded to 90 frames = 360 slots.
    test.run_stack(Some(360));
    let sink_msgs = test.dump_sinks();
    let delayed_mle_msgs: Vec<_> = sink_msgs
        .iter()
        .filter(|m| m.dest == TetraEntity::Mle && matches!(&m.msg, SapMsgInner::LcmcMleUnitdataReq(_)))
        .collect();
    assert_eq!(delayed_mle_msgs.len(), 0, "Expected no Home Mode SDS before startup delay expires");

    // First transmission at delay boundary.
    test.run_stack(Some(1));
    let sink_msgs = test.dump_sinks();
    let mut mle_msgs: Vec<_> = sink_msgs
        .iter()
        .filter(|m| m.dest == TetraEntity::Mle && matches!(&m.msg, SapMsgInner::LcmcMleUnitdataReq(_)))
        .collect();
    assert_eq!(
        mle_msgs.len(),
        1,
        "Expected first periodic D-SDS-DATA broadcast after startup delay"
    );

    // Second transmission after periodic interval (2 multiframes = 144 slots).
    test.run_stack(Some(144));
    let sink_msgs = test.dump_sinks();
    let second_mle_msgs: Vec<_> = sink_msgs
        .iter()
        .filter(|m| m.dest == TetraEntity::Mle && matches!(&m.msg, SapMsgInner::LcmcMleUnitdataReq(_)))
        .collect();
    assert_eq!(second_mle_msgs.len(), 1, "Expected second periodic D-SDS-DATA broadcast");
    mle_msgs.extend(second_mle_msgs);

    let mut seen_msg_refs = Vec::new();
    for msg in mle_msgs {
        let SapMsgInner::LcmcMleUnitdataReq(ref prim) = msg.msg else {
            panic!("Expected LcmcMleUnitdataReq");
        };

        assert_eq!(prim.main_address.ssi, 0x00FF_FFFF);
        assert_eq!(prim.main_address.ssi_type, SsiType::Gssi);

        let mut sdu = BitBuffer::from_bitbuffer(&prim.sdu);
        let parsed = DSdsData::from_bitbuf(&mut sdu).expect("Failed to parse periodic D-SDS-DATA");
        assert_eq!(parsed.calling_party_address_ssi, Some(1000001));
        match parsed.user_defined_data {
            SdsUserData::Type4(bits, data) => {
                assert_eq!(bits, 104);
                assert_eq!(data[0], 220); // Protocol identifier
                assert_eq!(data[1], 0x00); // SDS-TRANSFER + no delivery report + short form recommended + no S/F info
                seen_msg_refs.push(data[2]); // Message reference
                assert_eq!(data[3], 1); // Text coding scheme: ISO-8859-1 8-bit alphabet
                assert_eq!(&data[4..], b"HOME MODE");
            }
            other => panic!("Expected SDS Type4 payload, got {:?}", other),
        }
    }

    seen_msg_refs.sort_unstable();
    assert_eq!(seen_msg_refs, vec![0, 1], "Expected incrementing SDS-TL message references");
}

#[test]
fn test_periodic_home_mode_display_sds_custom_protocol_id() {
    debug::setup_logging_verbose();

    let dltime = TdmaTime { h: 0, m: 1, f: 1, t: 1 };
    let mut config = ComponentTest::get_default_test_config(StackMode::Bs);
    config.cell.home_mode_display = Some(home_mode_cfg(1000001, 18, 201, HomeModeDisplaySdsTextCodingScheme::LATIN, "HM"));
    let mut test = ComponentTest::from_config(config, Some(dltime));

    let components = vec![TetraEntity::Cmce];
    let sinks = vec![TetraEntity::Mle, TetraEntity::Brew];
    test.populate_entities(components, sinks);

    // 90-frame startup delay + 1 boundary tick for first send.
    test.run_stack(Some(361));

    let sink_msgs = test.dump_sinks();
    let mle_msgs: Vec<_> = sink_msgs
        .iter()
        .filter(|m| m.dest == TetraEntity::Mle && matches!(&m.msg, SapMsgInner::LcmcMleUnitdataReq(_)))
        .collect();
    assert_eq!(mle_msgs.len(), 1, "Expected 1 periodic D-SDS-DATA broadcast");

    let SapMsgInner::LcmcMleUnitdataReq(ref prim) = mle_msgs[0].msg else {
        panic!("Expected LcmcMleUnitdataReq");
    };

    let mut sdu = BitBuffer::from_bitbuffer(&prim.sdu);
    let parsed = DSdsData::from_bitbuf(&mut sdu).expect("Failed to parse periodic D-SDS-DATA");
    match parsed.user_defined_data {
        SdsUserData::Type4(bits, data) => {
            assert_eq!(bits, 48);
            assert_eq!(data[0], 201); // Configured protocol identifier
            assert_eq!(data[1], 0x00); // SDS-TRANSFER + no delivery report
            assert_eq!(data[2], 0); // First message reference
            assert_eq!(data[3], 0x01); // Text coding scheme: ISO-8859-1
            assert_eq!(&data[4..], b"HM");
        }
        other => panic!("Expected SDS Type4 payload, got {:?}", other),
    }
}

#[test]
fn test_periodic_home_mode_display_sds_respects_startup_delay() {
    debug::setup_logging_verbose();

    let dltime = TdmaTime { h: 0, m: 1, f: 1, t: 1 };
    let mut config = ComponentTest::get_default_test_config(StackMode::Bs);
    config.cell.home_mode_display = Some(home_mode_cfg(
        1000001,
        18,
        220,
        HomeModeDisplaySdsTextCodingScheme::LATIN,
        "HOME MODE",
    ));
    let mut test = ComponentTest::from_config(config, Some(dltime));

    let components = vec![TetraEntity::Cmce];
    let sinks = vec![TetraEntity::Mle, TetraEntity::Brew];
    test.populate_entities(components, sinks);

    // Startup delay is hardcoded to 90 frames = 360 slots.
    // 360 ticks from startup still leaves elapsed age below 360 slots in this harness.
    test.run_stack(Some(360));
    let sink_msgs = test.dump_sinks();
    let mle_msgs: Vec<_> = sink_msgs
        .iter()
        .filter(|m| m.dest == TetraEntity::Mle && matches!(&m.msg, SapMsgInner::LcmcMleUnitdataReq(_)))
        .collect();
    assert_eq!(mle_msgs.len(), 0, "Expected no Home Mode SDS before startup delay expires");

    // Next tick crosses the delay threshold and should trigger first send.
    test.run_stack(Some(1));
    let sink_msgs = test.dump_sinks();
    let mle_msgs: Vec<_> = sink_msgs
        .iter()
        .filter(|m| m.dest == TetraEntity::Mle && matches!(&m.msg, SapMsgInner::LcmcMleUnitdataReq(_)))
        .collect();
    assert_eq!(mle_msgs.len(), 1, "Expected first Home Mode SDS at startup delay boundary");
}

#[test]
fn test_periodic_home_mode_display_sds_utf16be_encoding() {
    debug::setup_logging_verbose();

    let dltime = TdmaTime { h: 0, m: 1, f: 1, t: 1 };
    let mut config = ComponentTest::get_default_test_config(StackMode::Bs);
    config.cell.home_mode_display = Some(home_mode_cfg(1000001, 18, 220, HomeModeDisplaySdsTextCodingScheme::UTF16, "Ā"));
    let mut test = ComponentTest::from_config(config, Some(dltime));

    let components = vec![TetraEntity::Cmce];
    let sinks = vec![TetraEntity::Mle, TetraEntity::Brew];
    test.populate_entities(components, sinks);

    test.run_stack(Some(361));

    let sink_msgs = test.dump_sinks();
    let mle_msgs: Vec<_> = sink_msgs
        .iter()
        .filter(|m| m.dest == TetraEntity::Mle && matches!(&m.msg, SapMsgInner::LcmcMleUnitdataReq(_)))
        .collect();
    assert_eq!(mle_msgs.len(), 1, "Expected 1 periodic D-SDS-DATA broadcast");

    let SapMsgInner::LcmcMleUnitdataReq(ref prim) = mle_msgs[0].msg else {
        panic!("Expected LcmcMleUnitdataReq");
    };

    let mut sdu = BitBuffer::from_bitbuffer(&prim.sdu);
    let parsed = DSdsData::from_bitbuf(&mut sdu).expect("Failed to parse periodic D-SDS-DATA");
    match parsed.user_defined_data {
        SdsUserData::Type4(bits, data) => {
            assert_eq!(bits, 48);
            assert_eq!(data[0], 220); // Protocol identifier
            assert_eq!(data[1], 0x00); // SDS-TRANSFER + no delivery report
            assert_eq!(data[2], 0); // First message reference
            assert_eq!(data[3], 0x1A); // Text coding scheme: UCS-2/UTF-16BE
            assert_eq!(&data[4..], &[0x01, 0x00]); // "Ā" in UTF-16BE
        }
        other => panic!("Expected SDS Type4 payload, got {:?}", other),
    }
}

#[test]
fn test_periodic_home_mode_display_sds_latin1_lossy_replacement() {
    debug::setup_logging_verbose();

    let dltime = TdmaTime { h: 0, m: 1, f: 1, t: 1 };
    let mut config = ComponentTest::get_default_test_config(StackMode::Bs);
    config.cell.home_mode_display = Some(home_mode_cfg(
        1000001,
        18,
        220,
        HomeModeDisplaySdsTextCodingScheme::LATIN,
        "AĀB", // 'Ā' cannot be represented in Latin-1
    ));
    let mut test = ComponentTest::from_config(config, Some(dltime));

    let components = vec![TetraEntity::Cmce];
    let sinks = vec![TetraEntity::Mle, TetraEntity::Brew];
    test.populate_entities(components, sinks);

    test.run_stack(Some(361));

    let sink_msgs = test.dump_sinks();
    let mle_msgs: Vec<_> = sink_msgs
        .iter()
        .filter(|m| m.dest == TetraEntity::Mle && matches!(&m.msg, SapMsgInner::LcmcMleUnitdataReq(_)))
        .collect();
    assert_eq!(mle_msgs.len(), 1, "Expected 1 periodic D-SDS-DATA broadcast");

    let SapMsgInner::LcmcMleUnitdataReq(ref prim) = mle_msgs[0].msg else {
        panic!("Expected LcmcMleUnitdataReq");
    };

    let mut sdu = BitBuffer::from_bitbuffer(&prim.sdu);
    let parsed = DSdsData::from_bitbuf(&mut sdu).expect("Failed to parse periodic D-SDS-DATA");
    match parsed.user_defined_data {
        SdsUserData::Type4(bits, data) => {
            assert_eq!(bits, 56);
            assert_eq!(data[0], 220); // Protocol identifier
            assert_eq!(data[1], 0x00); // SDS-TRANSFER + no delivery report
            assert_eq!(data[2], 0); // First message reference
            assert_eq!(data[3], 0x01); // Text coding scheme: ISO-8859-1
            assert_eq!(&data[4..], b"A?B"); // unsupported chars are replaced with '?'
        }
        other => panic!("Expected SDS Type4 payload, got {:?}", other),
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
        pre_coded_status: PreCodedStatus::try_from(0x8210).unwrap(),
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
fn test_u_status_brew_forward() {
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
        feature_sds_enabled: true,
        whitelisted_ssis: None,
    });
    let mut test = ComponentTest::from_config(config, Some(dltime));

    let components = vec![TetraEntity::Cmce];
    let sinks = vec![TetraEntity::Mle, TetraEntity::Brew];
    test.populate_entities(components, sinks);

    // Only register source, NOT dest — should forward to Brew
    register_subscriber(&mut test, 1000001);

    let u_status = UStatus {
        area_selection: 0,
        called_party_type_identifier: PartyTypeIdentifier::Ssi,
        called_party_short_number_address: None,
        called_party_ssi: Some(5000001),
        called_party_extension: None,
        pre_coded_status: PreCodedStatus::from(0x8210),
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

    // Should forward to Brew as CmceSdsData with Type1 payload
    let brew_count = count_brew_sds(&sink_msgs);
    assert_eq!(brew_count, 1, "Expected 1 CmceSdsData at Brew sink for U-STATUS");

    // Verify the payload is Type1 with the original pre-coded status value
    let brew_msg = sink_msgs.iter().find(|m| m.dest == TetraEntity::Brew).unwrap();
    if let SapMsgInner::CmceSdsData(ref sds) = brew_msg.msg {
        assert_eq!(sds.source_issi, 1000001);
        assert_eq!(sds.dest_issi, 5000001);
        assert_eq!(sds.user_defined_data, SdsUserData::Type1(0x8210));
    } else {
        panic!("Expected CmceSdsData message at Brew sink");
    }

    // Should NOT deliver locally
    let d_sds_count = count_d_sds_data(&sink_msgs);
    assert_eq!(d_sds_count, 0, "Should not deliver locally when dest is not registered");
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
        pre_coded_status: PreCodedStatus::from(0x8210),
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
