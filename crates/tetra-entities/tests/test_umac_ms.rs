mod common;

use common::{ComponentTest, default_test_config};
use tetra_config::StackMode;
use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{BitBuffer, PhyBlockNum, Sap, TdmaTime, debug};
use tetra_saps::sapmsg::{SapMsg, SapMsgInner};
use tetra_saps::tmv::{TmvUnitdataInd, enums::logical_chans::LogicalChannel};

#[test]
/// A test containing a single Lmac frame, containing a MAC-RESOURCE with no SDU, and a NULL pdu
fn test_umac_ms() {
    debug::setup_logging_verbose();
    let config = default_test_config(StackMode::Ms);
    let mut test = ComponentTest::new(config, None);

    let components = vec![TetraEntity::Umac];
    let sinks: Vec<TetraEntity> = vec![];
    test.populate_entities(components, sinks);

    let m = SapMsg {
        sap: Sap::TmvSap,
        src: TetraEntity::Lmac,
        dest: TetraEntity::Umac,
        dltime: TdmaTime::default(),
        msg: SapMsgInner::TmvUnitdataInd(TmvUnitdataInd {
            pdu: BitBuffer::from_bitstr(
                "0010001000110001011010110000101010001010000100000000110000010000100000000000000000000000000000000000000000000000000000000000",
            ),
            block_num: PhyBlockNum::Block1,
            logical_channel: LogicalChannel::SchHd,
            crc_pass: true,
            scrambling_code: 0,
        }),
    };

    // Submit and process message
    test.submit_message(m);
    test.deliver_all_messages();
    let sink_msgs = test.dump_sinks();

    // Evaluate results
    assert_eq!(sink_msgs.len(), 0);
    tracing::warn!("Validation of result not implemented");
}

#[test]
/// A test containing a 3-fragment message, which is reassembled by the UMAC
/// The message ultimately contains an SDS message, which is reconstructed in the CMCE.
/// Also tests the in-between LLC and MLE.  
fn test_umac_frag() {
    debug::setup_logging_verbose();
    let config = default_test_config(StackMode::Ms);
    let mut test = ComponentTest::new(config, None);

    let components = vec![TetraEntity::Umac, TetraEntity::Llc, TetraEntity::Mle, TetraEntity::Cmce];
    let sinks = vec![];
    test.populate_entities(components, sinks);

    // NDB 56/18/1/000 type1: 0000000111111001011010110000101001100011000000110100111101011010111110000100110000110000100100011000000000001100010101000000
    // NDB 57/01/1/000 type1: 0111000100110000000000010011001000110000001101000010110000110001010000000000110000010000100000000000000000000000000000000000
    let m = SapMsg {
        sap: Sap::TmvSap,
        src: TetraEntity::Lmac,
        dest: TetraEntity::Umac,
        dltime: TdmaTime { h: 0, m: 56, f: 18, t: 1 },
        msg: SapMsgInner::TmvUnitdataInd(TmvUnitdataInd {
            pdu: BitBuffer::from_bitstr(
                "0000000111111001011010110000101001100011000000110100111101011010111110000100110000110000100100011000000000001100010101000000",
            ),
            block_num: PhyBlockNum::Block1,
            logical_channel: LogicalChannel::SchHd,
            crc_pass: true,
            scrambling_code: 0,
        }),
    };
    test.submit_message(m);
    test.deliver_all_messages();

    let m = SapMsg {
        sap: Sap::TmvSap,
        src: TetraEntity::Lmac,
        dest: TetraEntity::Umac,
        dltime: TdmaTime { h: 0, m: 56, f: 18, t: 1 },
        msg: SapMsgInner::TmvUnitdataInd(TmvUnitdataInd {
            pdu: BitBuffer::from_bitstr(
                "0111000100110000000000010011001000110000001101000010110000110001010000000000110000010000100000000000000000000000000000000000",
            ),
            block_num: PhyBlockNum::Block1,
            logical_channel: LogicalChannel::SchHd,
            crc_pass: true,
            scrambling_code: 0,
        }),
    };

    test.submit_message(m);
    test.deliver_all_messages();
    let msgs = test.dump_sinks();
    for msg in msgs.iter() {
        tracing::info!("\nSink message: {:?}", msg);
    }

    tracing::warn!("Validation of result not implemented");
}

#[test]
/// A test containing a SYSINFO frame, parsed by UMAC and MLE
fn test_sysinfo() {
    debug::setup_logging_verbose();
    let config = default_test_config(StackMode::Ms);
    let mut test = ComponentTest::new(config, None);

    let components = vec![TetraEntity::Umac, TetraEntity::Llc, TetraEntity::Mle];
    let sinks = vec![
        // TetraComponent::Mle
    ];
    test.populate_entities(components, sinks);

    // Sysinfo test
    let m = SapMsg {
        sap: Sap::TmvSap,
        src: TetraEntity::Lmac,
        dest: TetraEntity::Umac,
        dltime: TdmaTime::default(),

        msg: SapMsgInner::TmvUnitdataInd(TmvUnitdataInd {
            // mac_block: BitBuffer::from_bitstr("1000001100101010010000000000110001101001011100000000001110001111100000100000000000010111100001100000111111000000110101100111"),
            pdu: BitBuffer::from_bitstr(
                "1000010000111111010001000000100001101001111100000000000000011101000011100000000000000000000000101111111111100101110101110111",
            ),
            block_num: PhyBlockNum::Block2,
            logical_channel: LogicalChannel::Bnch,
            crc_pass: true,
            scrambling_code: 0,
        }),
    };
    test.submit_message(m);
    test.deliver_all_messages();
    let msgs = test.dump_sinks();
    for msg in msgs.iter() {
        tracing::info!("\nSink message: {:?}", msg);
    }

    tracing::warn!("Validation of result not implemented");
}

#[test]
/// A test containing a SYNC frame, parsed by UMAC and MLE
fn test_sync() {
    debug::setup_logging_verbose();
    let config = default_test_config(StackMode::Ms);
    let mut test = ComponentTest::new(config, None);

    let components = vec![TetraEntity::Umac, TetraEntity::Llc, TetraEntity::Mle];
    let sinks = vec![TetraEntity::Lmac];
    test.populate_entities(components, sinks);

    // SB1 09/11/4/000 type1: 000100000111010110010010000000001101001000000100010101110011
    // TMB-SAP SYNC CC 000001(0x01) TN 11(4) FN 01011(11) MN 001001( 9) MCC 0110100100(420) MNC 00001000101011(555)
    let m = SapMsg {
        sap: Sap::TmvSap,
        src: TetraEntity::Lmac,
        dest: TetraEntity::Umac,
        dltime: TdmaTime::default(),
        msg: SapMsgInner::TmvUnitdataInd(TmvUnitdataInd {
            pdu: BitBuffer::from_bitstr("000100000111010110010010000000001101001000000100010101110011"),
            // pdu: BitBuffer::from_bitstr("000100000111100100111110000000000110011000000000000101111001"),
            block_num: PhyBlockNum::Block1,
            logical_channel: LogicalChannel::Bsch,
            crc_pass: true,
            scrambling_code: 0,
        }),
    };
    test.submit_message(m);
    test.deliver_all_messages();
    let msgs = test.dump_sinks();
    for msg in msgs.iter() {
        tracing::info!("\nSink message: {:?}", msg);
    }

    tracing::warn!("Validation of result not implemented");
}

#[test]
fn test_resource() {
    debug::setup_logging_verbose();
    let config = default_test_config(StackMode::Ms);
    let mut test = ComponentTest::new(config, None);

    let components = vec![TetraEntity::Umac, TetraEntity::Llc, TetraEntity::Mle, TetraEntity::Cmce];
    let sinks = vec![];
    test.populate_entities(components, sinks);

    let m = SapMsg {
        sap: Sap::TmvSap,
        src: TetraEntity::Lmac,
        dest: TetraEntity::Umac,
        dltime: TdmaTime::default(),

        msg: SapMsgInner::TmvUnitdataInd(TmvUnitdataInd {
            pdu: BitBuffer::from_bitstr(
                "0010000010001110000000000000000001100101110110001000100110001001010001101100100100011110001110010011000000000001001100111110000000001000000000000001000010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
            ),
            block_num: PhyBlockNum::Both,
            logical_channel: LogicalChannel::SchF,
            crc_pass: true,
            scrambling_code: 0,
        }),
    };
    test.submit_message(m);
    test.deliver_all_messages();
    let msgs = test.dump_sinks();
    for msg in msgs.iter() {
        tracing::info!("\nSink message: {:?}", msg);
    }

    tracing::warn!("Validation of result not implemented");
}
