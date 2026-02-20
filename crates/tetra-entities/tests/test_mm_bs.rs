mod common;

use common::{ComponentTest, default_test_config};
use tetra_config::StackMode;
use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{BitBuffer, Sap, SsiType, TdmaTime, TetraAddress, debug};
use tetra_saps::lmm::LmmMleUnitdataInd;
use tetra_saps::sapmsg::{SapMsg, SapMsgInner};

#[test]
fn test_unsupported_u_mm_status() {
    // Motorola requesting power management
    debug::setup_logging_verbose();
    let test_vec1 = "00110000010010";
    let time_vec1 = TdmaTime::default().add_timeslots(2); // Uplink time: 0/1/1/1, dl time 0/1/1/3
    let test_prim1 = LmmMleUnitdataInd {
        sdu: BitBuffer::from_bitstr(test_vec1),
        handle: 0,
        received_address: TetraAddress {
            encrypted: false,
            ssi_type: SsiType::Issi,
            ssi: 2040814,
        },
    };
    let test_sapmsg1 = SapMsg {
        sap: Sap::LmmSap,
        src: TetraEntity::Mle,
        dest: TetraEntity::Mm,
        dltime: time_vec1,
        msg: SapMsgInner::LmmMleUnitdataInd(test_prim1),
    };

    // Setup testing stack
    let config = default_test_config(StackMode::Bs);
    let mut test = ComponentTest::new(config, Some(time_vec1));
    let components = vec![TetraEntity::Mm];
    let sinks: Vec<TetraEntity> = vec![TetraEntity::Mle];
    test.populate_entities(components, sinks);

    // Submit and process message
    test.submit_message(test_sapmsg1);
    test.run_stack(Some(1));
    let sink_msgs = test.dump_sinks();

    // Evaluate results. We should have an MM message in the sink
    assert_eq!(sink_msgs.len(), 1);
    tracing::info!("We have the expected MM message, but full validation of result not implemented");
}
