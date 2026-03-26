use tetra_config::bluestation::SharedConfig;
use tetra_core::{BitBuffer, Sap, SsiType, TdmaTime, TetraAddress, tetra_entities::TetraEntity};
use tetra_pdus::mle::{enums::mle_protocol_discriminator::MleProtocolDiscriminator, pdus::d_nwrk_broadcast::DNwrkBroadcast};
use tetra_saps::{SapMsg, SapMsgInner, tla::TlaTlUnitdataReqBl};

use crate::{MessageQueue, mle::components::network_time};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BroadcastType {
    /// Initial value and value when no broadcast types are enabled
    None,
    NetworkTime,
}

pub struct MleBroadcast {
    config: SharedConfig,
    last_broadcast_type: BroadcastType,
    time_broadcast: Option<String>,
}

impl MleBroadcast {
    pub fn new(config: SharedConfig) -> Self {
        let time_broadcast = config.config().cell.timezone.clone();
        Self {
            config,
            last_broadcast_type: BroadcastType::None,
            time_broadcast,
        }
    }

    /// Send the next broadcast message based on the configured broadcast types and internal state.
    pub fn send_broadcast(&mut self, queue: &mut MessageQueue, ts: TdmaTime) {
        let broadcast_type = self.determine_next_broadcast_type();
        self.last_broadcast_type = broadcast_type;

        match broadcast_type {
            BroadcastType::NetworkTime => {
                self.send_d_nwrk_broadcast(queue, ts);
            }
            BroadcastType::None => {
                // No broadcast to send
            }
        }
    }

    /// Deterines the next type for the next broadcast message
    fn determine_next_broadcast_type(&self) -> BroadcastType {
        match self.last_broadcast_type {
            BroadcastType::None => {
                if self.time_broadcast.is_some() {
                    BroadcastType::NetworkTime
                } else {
                    BroadcastType::None
                }
            }
            BroadcastType::NetworkTime => BroadcastType::NetworkTime,
        }
    }

    fn send_d_nwrk_broadcast(&self, queue: &mut MessageQueue, ts: TdmaTime) {
        // Timezone is validated at config parse time, so encode cannot fail here
        let tz = self.time_broadcast.as_deref().unwrap();
        let time_value = network_time::encode_tetra_network_time(tz).unwrap();

        let pdu = DNwrkBroadcast {
            cell_re_select_parameters: 0,
            cell_load_ca: 0,
            tetra_network_time: Some(time_value),
            number_of_ca_neighbour_cells: Some(0),
            neighbour_cell_information_for_ca: None,
        };

        // Serialize the PDU (includes 3-bit MLE PDU type)
        let mut pdu_buf = BitBuffer::new(128);
        if let Err(e) = pdu.to_bitbuf(&mut pdu_buf) {
            tracing::warn!("Failed to serialize D-NWRK-BROADCAST: {:?}", e);
            return;
        }
        let pdu_len = pdu_buf.get_pos();
        pdu_buf.seek(0);

        // Prepend 3-bit MLE protocol discriminator
        let mut tl_sdu = BitBuffer::new(3 + pdu_len);
        tl_sdu.write_bits(MleProtocolDiscriminator::Mle.into_raw(), 3);
        tl_sdu.copy_bits(&mut pdu_buf, pdu_len);
        tl_sdu.seek(0);

        let sapmsg = SapMsg {
            sap: Sap::TlaSap,
            src: TetraEntity::Mle,
            dest: TetraEntity::Llc,
            dltime: ts,
            msg: SapMsgInner::TlaTlUnitdataReqBl(TlaTlUnitdataReqBl {
                main_address: TetraAddress {
                    ssi: 0xFFFFFF,
                    ssi_type: SsiType::Gssi, // TODO FIXME is this actually the SMI?
                    encrypted: false,
                },
                link_id: 0,
                endpoint_id: 0,
                tl_sdu,
                stealing_permission: false,
                subscriber_class: 0,
                fcs_flag: false,
                air_interface_encryption: None,
                // scrambling_code: todo!(), // TODO should be added here to make sysinfo/sync in mle
                // pdu_prio: todo!(),
                // data_prio: todo!(),
                packet_data_flag: false,
                n_tlsdu_repeats: 0,
                // scheduled_data_status: todo!(),
                // max_schedule_interval: todo!(),
                data_class_info: None,
                req_handle: 0,
                chan_alloc: None,
                tx_reporter: None,
            }),
        };
        queue.push_back(sapmsg);
        tracing::info!("D-NWRK-BROADCAST sent (tz={}, time=0x{:012X})", tz, time_value);
    }
}
