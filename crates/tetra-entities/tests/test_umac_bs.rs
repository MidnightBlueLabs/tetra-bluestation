mod common;

use common::{ComponentTest, default_test_config};
use tetra_config::StackMode;
use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{BitBuffer, PhyBlockNum, Sap, SsiType, TdmaTime, TetraAddress, debug};
use tetra_saps::lmm::LmmMleUnitdataReq;
use tetra_saps::sapmsg::{SapMsg, SapMsgInner};
use tetra_saps::tmv::{TmvUnitdataInd, enums::logical_chans::LogicalChannel};

#[test]
fn test_in_fragmented_sch_hu_and_sch_f() {
    // Receive SCH/HU containing MAC-ACCESS with fragmentation start
    // Then receive SCH-F containing MAC-END (UL)
    debug::setup_logging_verbose();
    let test_vec1 = "00000000111111000001001111110111000100011001011100111000000011111100001000010000000000000000";
    let test_vec2 = "0110001110000000000010010000000000000000000000000100010000000000000000000000000110010000000000000000000000001000001000000111111000001001111110000000010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
    let time_vec1 = TdmaTime::default().add_timeslots(2); // Uplink time: 0/1/1/1, dl time 0/1/1/3
    let test_prim1 = TmvUnitdataInd {
        pdu: BitBuffer::from_bitstr(test_vec1),
        block_num: PhyBlockNum::Block1,
        logical_channel: LogicalChannel::SchHu,
        crc_pass: true,
        scrambling_code: 864282631,
    };
    let test_sapmsg1 = SapMsg {
        sap: Sap::TmvSap,
        src: TetraEntity::Lmac,
        dest: TetraEntity::Umac,
        dltime: time_vec1,
        msg: SapMsgInner::TmvUnitdataInd(test_prim1),
    };
    let test_prim2 = TmvUnitdataInd {
        pdu: BitBuffer::from_bitstr(test_vec2),
        block_num: PhyBlockNum::Both,
        logical_channel: LogicalChannel::SchF,
        crc_pass: true,
        scrambling_code: 864282631,
    };
    let test_sapmsg2 = SapMsg {
        sap: Sap::TmvSap,
        src: TetraEntity::Lmac,
        dest: TetraEntity::Umac,
        dltime: time_vec1.add_timeslots(4), // Uplink time: 0/1/2/1
        msg: SapMsgInner::TmvUnitdataInd(test_prim2),
    };

    // Setup testing stack
    let config = default_test_config(StackMode::Bs);
    let mut test = ComponentTest::new(config, Some(time_vec1));
    let components = vec![TetraEntity::Umac, TetraEntity::Llc, TetraEntity::Mle];
    let sinks: Vec<TetraEntity> = vec![
        // TetraEntity::Lmac, // Simply discard
        TetraEntity::Mm,
    ];
    test.populate_entities(components, sinks);

    // Submit and process message
    test.submit_message(test_sapmsg1);
    test.run_stack(Some(4));
    test.submit_message(test_sapmsg2);
    test.run_stack(Some(1));
    let sink_msgs = test.dump_sinks();

    // Evaluate results. We should have an MM message in the sink
    assert_eq!(sink_msgs.len(), 1);
    tracing::info!("We have the expected MM message, but full validation of result not implemented");
}

#[test]
fn test_in_fragmented_sch_hu_and_sch_hu() {
    // Receive SCH/HU containing MAC-ACCESS with fragmentation start
    // Then receive SCH-HU containing MAC-END-HU
    // Message ultimately contains CMCE SDS message
    debug::setup_logging_verbose();
    let test_vec1 = "00000000111110010001111101110111000000010010011110000010000001100010001001001111100001010100";
    let test_vec2 = "10011000000101000110000000000000000000000000000000000000000000000000111111111111110100000010";
    let time_vec1 = TdmaTime::default().add_timeslots(2); // Uplink time: 0/1/1/1, dl time 0/1/1/3
    let test_prim1 = TmvUnitdataInd {
        pdu: BitBuffer::from_bitstr(test_vec1),
        block_num: PhyBlockNum::Block1,
        logical_channel: LogicalChannel::SchHu,
        crc_pass: true,
        scrambling_code: 864282631,
    };
    let test_sapmsg1 = SapMsg {
        sap: Sap::TmvSap,
        src: TetraEntity::Lmac,
        dest: TetraEntity::Umac,
        dltime: time_vec1,
        msg: SapMsgInner::TmvUnitdataInd(test_prim1),
    };
    let test_prim2 = TmvUnitdataInd {
        pdu: BitBuffer::from_bitstr(test_vec2),
        block_num: PhyBlockNum::Block1,
        logical_channel: LogicalChannel::SchHu,
        crc_pass: true,
        scrambling_code: 864282631,
    };
    let test_sapmsg2 = SapMsg {
        sap: Sap::TmvSap,
        src: TetraEntity::Lmac,
        dest: TetraEntity::Umac,
        dltime: time_vec1.add_timeslots(4), // Uplink time: 0/1/2/1
        msg: SapMsgInner::TmvUnitdataInd(test_prim2),
    };

    // Setup testing stack
    let config = default_test_config(StackMode::Bs);
    let mut test = ComponentTest::new(config, Some(time_vec1));
    let components = vec![TetraEntity::Umac, TetraEntity::Llc, TetraEntity::Mle];
    let sinks: Vec<TetraEntity> = vec![
        // TetraEntity::Lmac, // Simply discard
        TetraEntity::Cmce,
    ];
    test.populate_entities(components, sinks);

    // Submit and process message
    test.submit_message(test_sapmsg1);
    test.run_stack(Some(4));
    test.submit_message(test_sapmsg2);
    test.run_stack(Some(1));

    // Evaluate results. We should have an CMCE message in the sink
    let sink_msgs = test.dump_sinks();
    assert_eq!(sink_msgs.len(), 1);
    tracing::info!("We have the expected CMCE message, but full validation of result not implemented");
}

#[test]
fn test_out_fragmented_resource() {
    // Test for UMAC (and LLC/MLE)
    // The vector is an MM DAttachDetachGroupIdentityAcknowledgement which contains a lot of groups.
    // As it is very large, it needs to be fragmented at the MAC layer.
    debug::setup_logging_verbose();
    let test_vec = "10110011011100110100110001101011100000000000011101010011001110110100000000000111010100111111101101000000000001110101010000000011010000000000011101010100000010110100000000000111010101000001001101000000000001110101010000011011010000000000011101010100001000110100000000000111010101000010101101000000000001110101010000110011010000000000011101010100001110110100000000000111010101000100001101000000000001110101010001001011010000000000011101010100010100";
    let test_t = TdmaTime::default().add_timeslots(2); // Uplink time: 0/1/1/1, dl time 0/1/1/3
    let test_prim = LmmMleUnitdataReq {
        sdu: BitBuffer::from_bitstr(test_vec),
        handle: 0,
        address: TetraAddress {
            encrypted: false,
            ssi_type: SsiType::Ssi,
            ssi: 30128,
        },
        layer2service: 0,
        stealing_permission: false,
        stealing_repeats_flag: false,
        encryption_flag: false,
        is_null_pdu: false,
    };
    let test_sapmsg = SapMsg {
        sap: Sap::LmmSap,
        src: TetraEntity::Mm,
        dest: TetraEntity::Mle,
        dltime: test_t,
        msg: SapMsgInner::LmmMleUnitdataReq(test_prim),
    };

    // Setup testing stack
    let config = default_test_config(StackMode::Bs);
    let mut test = ComponentTest::new(config, Some(test_t));
    let components = vec![TetraEntity::Umac, TetraEntity::Llc, TetraEntity::Mle];
    let sinks: Vec<TetraEntity> = vec![TetraEntity::Lmac];
    test.populate_entities(components, sinks);

    // Submit and process message
    test.submit_message(test_sapmsg);
    test.run_stack(Some(8));

    tracing::info!("Validation of result not implemented");
}
