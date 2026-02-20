mod common;

use common::{ComponentTest, default_test_config};
use tetra_config::StackMode;
use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{BitBuffer, Sap, SsiType, TdmaTime, TetraAddress, debug};
use tetra_saps::sapmsg::{SapMsg, SapMsgInner};
use tetra_saps::tma::TmaUnitdataInd;

#[test]
fn test_udata_with_broken_mm_payload() {
    // INCOMPLETE VECTOR replace with something more meaningful
    debug::setup_logging_verbose();

    // FIXME make proper vec here that can be passed onwards
    let test_vec = "00011001011100111000000011111100001000010000000000000000"; // INCOMPLETE
    let time_vec = TdmaTime::default().add_timeslots(2); // Uplink time: 0/1/1/1
    let test_prim = TmaUnitdataInd {
        pdu: Some(BitBuffer::from_bitstr(test_vec)),
        main_address: TetraAddress {
            ssi: 2065022,
            ssi_type: SsiType::Issi,
            encrypted: false,
        },
        scrambling_code: 864282631,
        endpoint_id: 0,
        new_endpoint_id: None,
        css_endpoint_id: None,
        air_interface_encryption: 0,
        chan_change_response_req: false,
        chan_change_handle: None,
        chan_info: None,
    };
    let test_sapmsg = SapMsg {
        sap: Sap::TmaSap,
        src: TetraEntity::Umac,
        dest: TetraEntity::Llc,
        dltime: time_vec,
        msg: SapMsgInner::TmaUnitdataInd(test_prim),
    };

    // Setup testing stack
    let config = default_test_config(StackMode::Bs);
    let mut test = ComponentTest::new(config, Some(time_vec));

    let components = vec![TetraEntity::Llc, TetraEntity::Mle, TetraEntity::Mm];
    let sinks: Vec<TetraEntity> = vec![TetraEntity::Umac];
    test.populate_entities(components, sinks);

    // Submit and process message
    test.submit_message(test_sapmsg);
    test.run_stack(Some(1));
    let sink_msgs = test.dump_sinks();

    // Evaluate results
    assert_eq!(sink_msgs.len(), 1);
    tracing::warn!("Validation of result not implemented");
}
