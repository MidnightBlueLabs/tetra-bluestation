use tetra_core::{BitBuffer, Sap, SsiType, TdmaTime, TetraAddress, tetra_entities::TetraEntity};
use tetra_saps::{SapMsg, SapMsgInner, lmm::LmmMleUnitdataReq};

use tetra_pdus::mm::{enums::mm_pdu_type_ul::MmPduTypeUl, pdus::mm_pdu_function_not_supported::MmPduFunctionNotSupported};

pub fn make_ul_mm_pdu_function_not_supported(
    handle: u32,
    pdu_type: MmPduTypeUl,
    pdu_subtype: Option<(usize, u64)>,
    issi: u32,
    dl_time: TdmaTime,
) -> (SapMsg, String) {
    // Create PDU
    let pdu = MmPduFunctionNotSupported {
        not_supported_pdu_type: pdu_type as u8,
        not_supported_sub_pdu_type: pdu_subtype,
    };

    // Convert pdu to bits
    let mut sdu = BitBuffer::new_autoexpand(14);
    pdu.to_bitbuf(&mut sdu).unwrap(); // we want to know when this happens
    sdu.seek(0);

    let debug_str = format!("{:?} sdu {}", pdu, sdu.dump_bin());

    // Package
    let addr = TetraAddress {
        encrypted: false,
        ssi_type: SsiType::Ssi,
        ssi: issi,
    };
    let msg = SapMsg {
        sap: Sap::LmmSap,
        src: TetraEntity::Mm,
        dest: TetraEntity::Mle,
        dltime: dl_time,
        msg: SapMsgInner::LmmMleUnitdataReq(LmmMleUnitdataReq {
            sdu,
            handle,
            address: addr,
            layer2service: 0,
            stealing_permission: false,
            stealing_repeats_flag: false,
            encryption_flag: false,
            is_null_pdu: false,
        }),
    };
    (msg, debug_str)
}
