use std::collections::{HashMap, HashSet, VecDeque};
use std::panic;

use crate::{MessageQueue, TetraEntityTrait};
use tetra_config::bluestation::SharedConfig;
use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{BitBuffer, Layer2Service, Sap, SsiType, TdmaTime, TetraAddress, TxReporter, unimplemented_log};
use tetra_saps::lcmc::enums::alloc_type::ChanAllocType;
use tetra_saps::lcmc::enums::ul_dl_assignment::UlDlAssignment;
use tetra_saps::lcmc::fields::chan_alloc_req::CmceChanAllocReq;
use tetra_saps::tla::{TlaTlDataIndBl, TlaTlUnitdataIndBl};
use tetra_saps::tma::TmaUnitdataReq;
use tetra_saps::{SapMsg, SapMsgInner};

use crate::llc::components::fcs;
use tetra_pdus::llc::consts::consts::N252_BL_MAX_TLSDU_RETRANSMITS_ACKED;
use tetra_pdus::llc::consts::timers::T251_SENDER_RETRY_TIMER;
use tetra_pdus::llc::enums::llc_pdu_type::LlcPduType;
use tetra_pdus::llc::pdus::bl_ack::BlAck;
use tetra_pdus::llc::pdus::bl_adata::BlAdata;
use tetra_pdus::llc::pdus::bl_data::BlData;
use tetra_pdus::llc::pdus::bl_udata::BlUdata;

/// Struct that maintains state expected acknowledgement data for a transmitted message.
/// Aka, we still expect an ack for this.
pub struct ExpectedInAck {
    /// Timeslot on which the original message was sent
    pub ts: u8,
    /// Address to which the message was sent
    pub addr: TetraAddress,

    /// Expected ack sequence number for the original message
    pub ns: u8,

    pub bl_type: Layer2Service,

    /// Time this message was received from the MLE
    pub t_first: TdmaTime,
    /// Time this message was actually passed down to the Umac. If a previous message on the basic link is already
    /// submitted, the message has to wait until that previous message was sent and acknowledged, or lost.
    pub t_submitted_to_umac: Option<TdmaTime>,
    /// Time the RxReporter signalled the message was fully transmitted. Also set if the Umac discarded the message
    /// This helps attempting to retransmit the message after a brief delay.
    pub t_umac_done: Option<TdmaTime>,
    /// TxReporter struct. Used by Umac to signal Tx time to Llc, so llc can do retransmissions if needed.
    /// Also used by Llc to signal Ack to upper layer (if appliccable)
    pub tx_reporter: TxReporter,

    // Optional retransmission buffer, to allow for automatic retransmission of the PDU if no acknowledgement is received
    pub retransmission_buf: SapMsg,
    /// Number of retransmissions performed so far
    pub retransmit_count: u8,
}

/// Struct that maintains state for an ACK we still need to send back.
pub struct ScheduledOutAck {
    pub addr: TetraAddress,
    pub t_start: TdmaTime,
    /// Received sequence number
    pub nr: u8,
    /// Timeslot on which the original message was received
    pub ts: u8,
}

pub struct Llc {
    dltime: TdmaTime,
    config: SharedConfig,

    /// When we receive a message, and it needs to be acknowledged, we store it here for later
    /// integration into a response message, or we will make a separate BL-ACK for it.
    scheduled_out_acks: VecDeque<ScheduledOutAck>,

    /// Outbound messages, that are either already submitted to the Umac, and wait for ack,
    /// or, messages that can't be sent until previous messages for the same SSI have been
    /// acknowledged, first.
    outbound_messages: VecDeque<ExpectedInAck>,
    outbound_udata_messages: VecDeque<SapMsg>,

    /// Per-link send sequence variable per SSI. Alternates between 0 and 1.
    link_send_seq: HashMap<u32, u8>,
}

impl Llc {
    pub fn new(config: SharedConfig) -> Self {
        Self {
            dltime: TdmaTime::default(),
            config,
            scheduled_out_acks: VecDeque::new(),
            outbound_messages: VecDeque::new(),
            outbound_udata_messages: VecDeque::new(),
            link_send_seq: HashMap::new(),
        }
    }

    /// Schedule an ACK to be sent at a later time
    pub fn schedule_outgoing_ack(&mut self, dltime: TdmaTime, addr: TetraAddress, ns: u8) {
        self.scheduled_out_acks.push_back(ScheduledOutAck {
            t_start: dltime,
            nr: ns,
            addr,
            ts: dltime.t,
        });
    }

    /// Returns details for outstanding to-be-sent ACK, if any. Returned u8 is the sequence number
    fn get_out_ack_seq_if_any(&mut self, tn: u8, addr: TetraAddress) -> Option<u8> {
        for i in 0..self.scheduled_out_acks.len() {
            if self.scheduled_out_acks[i].t_start.t == tn && self.scheduled_out_acks[i].addr.ssi == addr.ssi {
                let n = self.scheduled_out_acks[i].nr;
                self.scheduled_out_acks.remove(i);
                return Some(n);
            }
        }
        None
    }

    /// Returns the next send sequence number V(S) for this link, then toggles it.
    /// Each link independently starts at 0 and alternates 0,1,0,1,...
    fn get_next_send_seq(&mut self, addr: &TetraAddress) -> u8 {
        let vs = self.link_send_seq.entry(addr.ssi).or_insert(0);
        let ns = *vs;
        *vs ^= 1;
        ns
    }

    /// Returns and removes the expected ACK entry for the given SSI, if any
    fn take_expected_ack_for_ssi(&mut self, ssi: u32) -> Option<ExpectedInAck> {
        for i in 0..self.outbound_messages.len() {
            let msg = &self.outbound_messages[i];
            if msg.addr.ssi == ssi && msg.t_submitted_to_umac.is_some() {
                return self.outbound_messages.remove(i);
            }
        }
        None
    }

    /// Process incoming ACK per ETSI 22.3.2.3(k).
    /// Matches by SSI and N(R) so that retransmitted BL-DATA entries are matched correctly.
    fn process_incoming_ack(&mut self, addr: TetraAddress, nr: u8) {
        // Get the expected ACK entry
        let Some(expected_ack) = self.take_expected_ack_for_ssi(addr.ssi) else {
            tracing::warn!("received unexpected ACK for SSI {} N(R) {}", addr.ssi, nr);
            return;
        };

        // Check it was indeed already transmitted by the Umac
        if expected_ack.t_umac_done.is_none() {
            // This may be an old retransmission of an ack for the before-last basic link message
            // Let's push the ack back into the head of the queue (not tail)..
            tracing::warn!(
                "received ACK for SSI {} N(R) {} that was not yet transmitted by Umac. Ignoring",
                addr.ssi,
                nr
            );
            self.outbound_messages.push_front(expected_ack);
            return;
        }

        // Check N(R)
        if expected_ack.ns == nr {
            // Successful ACK: N(R) matches N(S)
            tracing::debug!("received ACK for SSI {} N(R) {}", addr.ssi, expected_ack.ns);
            expected_ack.tx_reporter.mark_acknowledged();
            return;
        } else {
            // N(R) mismatch — per ETSI 22.3.2.3(k), not a successful ACK. Maybe a retransmission?
            // Let's push it back into the queue head (not the tail) and see if an ack arrives later
            tracing::warn!(
                "received unexpected ACK for SSI {}: N(R)={}, expected N(S)={}. Ignoring",
                addr.ssi,
                nr,
                expected_ack.ns
            );
            self.outbound_messages.push_front(expected_ack);
            return;
        }

        // The expected_ack is confirmed as matched and goes out of scope here
    }

    fn rx_tma_prim(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        tracing::trace!("rx_tma_prim");
        match message.msg {
            SapMsgInner::TmaUnitdataInd(_) => {
                self.rx_tma_unitdata_ind(queue, message);
            }
            SapMsgInner::TmaReportInd(_) => {
                self.rx_tma_report_ind(queue, message);
            }
            _ => {
                panic!();
            }
        }
    }

    fn rx_tla_tlunitdata_req_bl(&mut self, _queue: &mut MessageQueue, message: SapMsg) {
        tracing::trace!("rx_tla_tlunitdata_req_bl");
        let SapMsgInner::TlaTlUnitdataReqBl(mut prim) = message.msg else {
            panic!()
        };

        let mut pdu_buf = BitBuffer::new_autoexpand(32);
        let pdu = BlUdata { has_fcs: false };
        pdu.to_bitbuf(&mut pdu_buf);
        let sdu_len = prim.tl_sdu.get_len_remaining();
        pdu_buf.copy_bits(&mut prim.tl_sdu, sdu_len);
        pdu_buf.seek(0);
        tracing::debug!("-> {:?} sdu {}", pdu, pdu_buf.dump_bin());

        let sapmsg = SapMsg {
            sap: Sap::TmaSap,
            src: self.entity(),
            dest: TetraEntity::Umac,
            dltime: message.dltime,
            msg: SapMsgInner::TmaUnitdataReq(TmaUnitdataReq {
                req_handle: prim.req_handle,
                pdu: pdu_buf,
                main_address: prim.main_address,
                endpoint_id: prim.endpoint_id,
                stealing_permission: prim.stealing_permission,
                subscriber_class: prim.subscriber_class,
                air_interface_encryption: prim.air_interface_encryption,
                stealing_repeats_flag: None, // fixme
                data_category: prim.data_class_info,
                chan_alloc: prim.chan_alloc,
                tx_reporter: prim.tx_reporter.take(),
            }),
        };

        // Put into transmit queue
        self.outbound_udata_messages.push_back(sapmsg);
    }

    /// Schedules a message that was not acked in time for a retransmission
    fn submit_for_acknowledged_transmission(queue: &mut MessageQueue, ack: &mut ExpectedInAck, dltime: TdmaTime) {
        // Clone the sapmsg, with update dltime
        let mut sapmsg = ack.retransmission_buf.clone();
        sapmsg.dltime = dltime;

        // Make sure we set (or for retransmission: reset) timers properly
        ack.t_submitted_to_umac = Some(dltime);
        ack.t_umac_done = None;
        ack.tx_reporter.reset();

        // Send the message
        queue.push_back(sapmsg);
    }

    /// See Clause 22.3.2.3 for Acknowledged data transmission in basic link
    fn rx_tla_tldata_req_bl(&mut self, _queue: &mut MessageQueue, message: SapMsg) {
        tracing::trace!("rx_tla_tldata_req_bl");
        let SapMsgInner::TlaTlDataReqBl(mut prim) = message.msg else {
            panic!()
        };

        if prim.stealing_permission {
            panic!("Can't send BL-DATA for STCH message");
        }
        if prim.main_address.ssi_type == SsiType::Gssi {
            panic!("Can't send BL-DATA for GSSI-addressed message. ");
        }

        // If an ack still needs to be sent, get the relevant expected sequence number
        let out_ack_n = self.get_out_ack_seq_if_any(message.dltime.t, prim.main_address);

        // Get per-link send sequence number N(S) = V(S), then toggle V(S)
        let ns = self.get_next_send_seq(&prim.main_address);

        // Construct PDU, write header
        let mut pdu_buf = BitBuffer::new_autoexpand(32);

        // Determine message type and build
        if let Some(out_ack_n) = out_ack_n {
            // BL-ADATA (acknowledged, with or without FCS)
            let pdu = BlAdata {
                has_fcs: prim.fcs_flag,
                nr: out_ack_n,
                ns,
            };
            pdu.to_bitbuf(&mut pdu_buf);
            // Append SDU
            let sdu_len = prim.tl_sdu.get_len_remaining();
            pdu_buf.copy_bits(&mut prim.tl_sdu, sdu_len);
            pdu_buf.seek(0);
            tracing::debug!(ts=%self.dltime, "-> {:?} sdu {}", pdu, pdu_buf.dump_bin());
        } else {
            // BL-DATA (acknowledged, with or without FCS) — ETSI Clause 22.3.2.3
            let pdu = BlData {
                has_fcs: prim.fcs_flag,
                ns,
            };
            pdu.to_bitbuf(&mut pdu_buf);
            // Append SDU
            let sdu_len = prim.tl_sdu.get_len_remaining();
            pdu_buf.copy_bits(&mut prim.tl_sdu, sdu_len);
            pdu_buf.seek(0);
            tracing::debug!(ts=%self.dltime, "-> {:?} sdu {}", pdu, pdu_buf.dump_bin());
        }

        // Either take tx_reporter passed down or create a new one
        let tx_reporter = prim.tx_reporter.take().unwrap_or_else(|| TxReporter::new());

        let sapmsg = SapMsg {
            sap: Sap::TmaSap,
            src: self.entity(),
            dest: TetraEntity::Umac,
            dltime: message.dltime,
            msg: SapMsgInner::TmaUnitdataReq(TmaUnitdataReq {
                req_handle: prim.req_handle,
                pdu: pdu_buf,
                main_address: prim.main_address,
                endpoint_id: prim.endpoint_id,
                stealing_permission: prim.stealing_permission,
                subscriber_class: prim.subscriber_class,
                air_interface_encryption: prim.air_interface_encryption,
                stealing_repeats_flag: prim.stealing_repeats_flag,
                data_category: prim.data_class_info,
                chan_alloc: prim.chan_alloc,
                tx_reporter: Some(tx_reporter.clone()),
            }),
        };

        // Register that we expect an ACK for this message
        self.outbound_messages.push_back(ExpectedInAck {
            ns,
            addr: prim.main_address,
            ts: sapmsg.dltime.t, // TODO FIXME
            bl_type: Layer2Service::Acknowledged,
            tx_reporter,
            t_first: sapmsg.dltime,
            t_submitted_to_umac: None,
            t_umac_done: None,
            retransmission_buf: sapmsg, // Clone the message to keep a copy for potential retransmission
            retransmit_count: 0,
        });

        // The message will now be picked up for transmission at end-of-tick, if the ssi does not yet have
        // a pending message waiting for an ack.
    }

    fn rx_tla_prim(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        tracing::trace!("rx_tla_prim");
        match &message.msg {
            SapMsgInner::TlaTlDataReqBl(_) => {
                self.rx_tla_tldata_req_bl(queue, message);
            }
            SapMsgInner::TlaTlUnitdataReqBl(_) => {
                self.rx_tla_tlunitdata_req_bl(queue, message);
            }
            _ => panic!(),
        }
    }

    fn rx_tma_report_ind(&mut self, _queue: &mut MessageQueue, mut _message: SapMsg) {
        tracing::trace!("rx_tma_report_ind, ignoring");
    }

    /// Clause 20.4.1.1.4 TMA-UNITDATA primitive
    /// TMA-UNITDATA indication: this primitive shall be used by the MAC to deliver a received TM-SDU. This primitive
    /// may also be used with no TM-SDU if the MAC needs to inform the higher layers of a channel allocation received
    /// without an associated TM-SDU.
    fn rx_tma_unitdata_ind(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
        tracing::trace!("rx_tma_unitdata_ind");

        // Determine which type of TL-SDU we have
        let pdu_type = if let SapMsgInner::TmaUnitdataInd(prim) = &mut message.msg {
            let Some(pdu) = prim.pdu.as_ref() else {
                panic!("no pdu");
            };
            let Some(bits) = pdu.peek_bits(4) else {
                tracing::warn!("insufficient bits: {}", pdu.dump_bin());
                return;
            };
            let Ok(pdu_type) = LlcPduType::try_from(bits) else {
                tracing::warn!("invalid pdu type: {} in {}", bits, pdu.dump_bin());
                return;
            };

            pdu_type
        } else {
            panic!();
        };

        // Call handler function
        match pdu_type {
            // All Basic Link types can be handled by the same function
            LlcPduType::BlAdata
            | LlcPduType::BlAdataFcs
            | LlcPduType::BlData
            | LlcPduType::BlDataFcs
            | LlcPduType::BlUdata
            | LlcPduType::BlUdataFcs
            | LlcPduType::BlAck
            | LlcPduType::BlAckFcs => {
                self.rx_tma_unitdata_ind_bl(queue, message);
            }

            LlcPduType::AlSetup
            | LlcPduType::AlDataAlFinal
            | LlcPduType::AlAlUdataAlUfinal
            | LlcPduType::AlAckAlRnr
            | LlcPduType::AlReconnect
            | LlcPduType::AlDisc => {
                unimplemented_log!("LlcPduType Advanced Link: {}", pdu_type);
            }

            _ => {
                panic!();
            }
        }
    }

    fn rx_tma_unitdata_ind_bl(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
        tracing::trace!("rx_tma_unitdata_ind_bl");

        // Get header bits (again) and prepare MLE message
        let SapMsgInner::TmaUnitdataInd(prim) = &mut message.msg else {
            panic!();
        };
        let Some(mut pdu) = prim.pdu.take() else {
            panic!("no pdu");
        };
        let Some(bits) = pdu.peek_bits(4) else {
            tracing::warn!("insufficient bits: {}", pdu.dump_bin());
            return;
        };
        let Ok(pdu_type) = LlcPduType::try_from(bits) else {
            tracing::warn!("invalid pdu type: {} in {}", bits, pdu.dump_bin());
            return;
        };

        let (has_fcs, ns, nr) = match pdu_type {
            LlcPduType::BlAdata | LlcPduType::BlAdataFcs => match BlAdata::from_bitbuf(&mut pdu) {
                Ok(pdu) => {
                    tracing::debug!(ts=%self.dltime, "<- {:?}", pdu);
                    (pdu.has_fcs, Some(pdu.ns), Some(pdu.nr))
                }
                Err(e) => {
                    tracing::warn!("Failed parsing BlAdata: {:?} {}", e, pdu.dump_bin());
                    return;
                }
            },

            LlcPduType::BlData | LlcPduType::BlDataFcs => match BlData::from_bitbuf(&mut pdu) {
                Ok(pdu) => {
                    tracing::debug!(ts=%self.dltime, "<- {:?}", pdu);
                    (pdu.has_fcs, Some(pdu.ns), None)
                }
                Err(e) => {
                    tracing::warn!("Failed parsing BlData: {:?} {}", e, pdu.dump_bin());
                    return;
                }
            },
            LlcPduType::BlAck | LlcPduType::BlAckFcs => match BlAck::from_bitbuf(&mut pdu) {
                Ok(pdu) => {
                    tracing::debug!(ts=%self.dltime, "<- {:?}", pdu);
                    (pdu.has_fcs, None, Some(pdu.nr))
                }
                Err(e) => {
                    tracing::warn!("Failed parsing BlAck: {:?} {}", e, pdu.dump_bin());
                    return;
                }
            },
            LlcPduType::BlUdata | LlcPduType::BlUdataFcs => match BlUdata::from_bitbuf(&mut pdu) {
                Ok(pdu) => {
                    tracing::debug!(ts=%self.dltime, "<- {:?}", pdu);
                    (pdu.has_fcs, None, None)
                }
                Err(e) => {
                    tracing::warn!("Failed parsing BlUdata: {:?} {}", e, pdu.dump_bin());
                    return;
                }
            },
            _ => {
                panic!();
            }
        };

        // If FCS is present, check it. If wrong, we bail here
        if has_fcs && !fcs::check_fcs(&pdu) {
            tracing::warn!("FCS check failed");
            return;
        }

        // If ns is present, we need to send an ACK
        if let Some(ns) = ns {
            // Send ACK
            self.schedule_outgoing_ack(message.dltime, prim.main_address, ns);
        }

        // if nr is present, we have received an ACK on a previous message
        if let Some(nr) = nr {
            self.process_incoming_ack(prim.main_address, nr);
        }

        if pdu_type == LlcPduType::BlAck || pdu_type == LlcPduType::BlAckFcs {
            // No payload, no need to do anything further
            if pdu.get_len_remaining() > 4 {
                tracing::warn!("BL-ACK PDU with unexpected payload, ignoring extra bits: {}", pdu.dump_bin());
            }
            return;
        }

        // If unacknowledged data transfer service, we send a TL-UNITDATA indication
        // to MLE. If acknowledged data transfer service, we send a TL-DATA indication
        pdu.set_raw_start(pdu.get_raw_pos());
        let s = if pdu_type == LlcPduType::BlUdata || pdu_type == LlcPduType::BlUdataFcs {
            // Unacknowledged data transfer service
            let m = TlaTlUnitdataIndBl {
                // address_type: 0, // TODO FIXME
                main_address: prim.main_address,
                link_id: message.dltime.add_timeslots(-2).t as u32,
                endpoint_id: prim.endpoint_id,
                new_endpoint_id: prim.new_endpoint_id,
                css_endpoint_id: prim.css_endpoint_id,
                tl_sdu: if pdu.get_len_remaining() > 0 { Some(pdu) } else { None },
                scrambling_code: prim.scrambling_code,
                fcs_flag: has_fcs,
                air_interface_encryption: prim.air_interface_encryption,
                chan_change_resp_req: prim.chan_change_response_req,
                chan_change_handle: prim.chan_change_handle,
                chan_info: prim.chan_info,
                report: None, // TODO FIXME
            };
            SapMsg {
                sap: Sap::TlaSap,
                src: TetraEntity::Llc,
                dest: TetraEntity::Mle,
                dltime: message.dltime,
                msg: SapMsgInner::TlaTlUnitdataIndBl(m),
            }
        } else {
            // Acknowledged data transfer service
            let m = TlaTlDataIndBl {
                // address_type: 0, // TODO FIXME
                main_address: prim.main_address,
                link_id: message.dltime.add_timeslots(-2).t as u32,
                endpoint_id: prim.endpoint_id,
                new_endpoint_id: prim.new_endpoint_id,
                css_endpoint_id: prim.css_endpoint_id,
                tl_sdu: if pdu.get_len_remaining() > 0 { Some(pdu) } else { None },
                scrambling_code: prim.scrambling_code,
                fcs_flag: has_fcs,
                air_interface_encryption: prim.air_interface_encryption,
                chan_change_resp_req: prim.chan_change_response_req,
                chan_change_handle: prim.chan_change_handle,
                chan_info: prim.chan_info,
                req_handle: 0, // TODO FIXME
            };
            SapMsg {
                sap: Sap::TlaSap,
                src: TetraEntity::Llc,
                dest: TetraEntity::Mle,
                dltime: message.dltime,
                msg: SapMsgInner::TlaTlDataIndBl(m),
            }
        };

        queue.push_back(s);
    }

    fn submit_retransmissions_to_umac(&mut self, queue: &mut MessageQueue) -> bool {
        let mut had_activity = false;
        let dltime = self.dltime;
        let mut removals: Option<Vec<u32>> = None;

        // if !self.outbound_messages.is_empty() {
        //     tracing::error!("{}", Self::format_expected_ack_list(&self.outbound_messages));
        // }

        for ack in self.outbound_messages.iter_mut() {
            // First, check which have newly been txed, or discarded by Umac. If so, start t_umac_done.
            if ack.t_umac_done.is_none() && (ack.tx_reporter.is_transmitted() || ack.tx_reporter.is_discarded()) {
                // TxReporter has now marked it as txed or dropped, so we can set t_umac_done
                ack.t_umac_done = Some(self.dltime);
                tracing::trace!("schedule_retransmissions: {} umac_done at {}", ack.addr.ssi, dltime);
            }

            // If we don't have a t_umac_done, there is no need for a retransmission in any case
            let Some(t_umac_done) = ack.t_umac_done else {
                continue;
            };

            // Retransmit scenario 1: it was transmitted but no ack received within the expected window (ETSI T.251 / N.252)
            // Retransmission scenario 2: it has been dropped by Umac due to congestion. Retransmit after same window
            let age = dltime.diff(t_umac_done); // Never fails
            if age as u32 >= T251_SENDER_RETRY_TIMER {
                // Time for either retransmitting or giving up
                if ack.retransmit_count < N252_BL_MAX_TLSDU_RETRANSMITS_ACKED {
                    // Retransmit
                    ack.retransmit_count += 1;
                    tracing::info!(
                        "retransmitting SSI {} N(S) {} attempt {}",
                        ack.addr.ssi,
                        ack.ns,
                        ack.retransmit_count
                    );

                    Self::submit_for_acknowledged_transmission(queue, ack, self.dltime.forward_to_timeslot(ack.t_first.t));
                    had_activity = true;
                } else {
                    // Exhausted retransmissions, flag for discard
                    removals.get_or_insert(Vec::new()).push(ack.addr.ssi);
                }
            }
        }

        // Remove any expired entries
        if let Some(removals) = removals {
            for ssi in removals {
                let ack = self.take_expected_ack_for_ssi(ssi).unwrap(); // Never fails
                tracing::warn!(
                    "schedule_retransmissions: SSI {} N(S) {} exhausted retransmissions",
                    ack.addr.ssi,
                    ack.ns
                );
                ack.tx_reporter.mark_lost();
            }
            // The ack expires here
        }

        had_activity
    }

    fn submit_free_messages_to_umac(&mut self, queue: &mut MessageQueue) -> bool {
        let mut had_activity = false;
        let mut ssi_blocked: HashSet<u32> = HashSet::new();
        for ack in self.outbound_messages.iter_mut() {
            // Check if already submitted to umac
            if ack.t_submitted_to_umac.is_some() {
                // This ssi currently waits for an ack, and is thus blocked
                ssi_blocked.insert(ack.addr.ssi);
                continue;
            }

            // Not submitted; check if blocked
            if ssi_blocked.contains(&ack.addr.ssi) {
                // SSI already has another message waiting for ack, so we cannot submit this one yet
                tracing::debug!(
                    "SSI {} N(S) {} still blocked by previous message, cannot submit next message",
                    ack.addr.ssi,
                    ack.ns
                );
                continue;
            }

            // Not submitted and not blocked. We can submit it now.
            // tracing::debug!("submitting message for SSI {} N(S) {} to umac", ack.addr.ssi, ack.ns);
            tracing::debug!(
                "submitting message for SSI {} N(S) {} to umac: {:?}",
                ack.addr.ssi,
                ack.ns,
                ack.retransmission_buf.msg
            );
            Self::submit_for_acknowledged_transmission(queue, ack, self.dltime.forward_to_timeslot(ack.t_first.t));
            ssi_blocked.insert(ack.addr.ssi);
            had_activity = true;
        }

        had_activity
    }

    /// Pops all elements from the scheduled_out_acks queue, prepares BL-ACK messages, and send them down
    fn submit_ack_replies_to_umac(&mut self, queue: &mut MessageQueue) -> bool {
        let had_activity = !self.scheduled_out_acks.is_empty();
        while let Some(ack) = self.scheduled_out_acks.pop_front() {
            tracing::debug!("auto-ack for ssi: {}, n: {}, ts: {}", ack.addr.ssi, ack.nr, ack.ts);

            // Send BL-ACK via FACCH (stealing) on the traffic timeslot if the original
            // message arrived on a traffic channel (TS2-4), otherwise via MCCH (TS1).
            let steal = matches!(ack.ts, 2..=4);
            let mut pdu_buf = BitBuffer::new_autoexpand(5);
            let pdu = BlAck {
                has_fcs: false,
                nr: ack.nr,
            };
            pdu.to_bitbuf(&mut pdu_buf);
            pdu_buf.seek(0);
            tracing::debug!(ts=%self.dltime, "-> {:?} {}", pdu, pdu_buf.dump_bin());

            // We're sending an ACK for a received uplink message, however, we don't have that message here
            // Since DL is two slots ahead of UL, we will correct that. We now have the dltime for reception
            // of the original message.
            let dltime = self.dltime.add_timeslots(-2);
            let chan_alloc = match steal {
                true => {
                    let mut timeslots = [false; 4];
                    timeslots[(ack.ts - 1) as usize] = true;
                    Some(CmceChanAllocReq {
                        usage: None,
                        timeslots,
                        alloc_type: ChanAllocType::Replace,
                        ul_dl_assigned: UlDlAssignment::Both,
                        carrier: None,
                    })
                }
                false => None,
            };
            let sapmsg = SapMsg {
                sap: Sap::TmaSap,
                src: TetraEntity::Llc,
                dest: TetraEntity::Umac,
                dltime,
                msg: SapMsgInner::TmaUnitdataReq(TmaUnitdataReq {
                    req_handle: 0, // TODO FIXME
                    pdu: pdu_buf,
                    main_address: ack.addr,
                    endpoint_id: 0, // todo fixme
                    stealing_permission: steal,
                    subscriber_class: 0,            // TODO FIXME
                    air_interface_encryption: None, // TODO FIXME
                    stealing_repeats_flag: None,    // TODO FIXME
                    data_category: None,            // TODO FIXME
                    chan_alloc,
                    tx_reporter: None, // By definition, no higher layer entity is interested
                }),
            };
            queue.push_back(sapmsg);
        }
        had_activity
    }

    /// Pops all elements from the scheduled_out_acks queue, prepares BL-ACK messages, and send them down
    fn submit_udata_msgs_to_umac(&mut self, queue: &mut MessageQueue) -> bool {
        let had_activity = !self.outbound_udata_messages.is_empty();
        while let Some(msg) = self.outbound_udata_messages.pop_front() {
            tracing::debug!("submitting udata msg to umac: {:?}", msg.msg);
            queue.push_back(msg);
        }
        had_activity
    }

    fn format_expected_ack_list(ack_list: &VecDeque<ExpectedInAck>) -> String {
        let mut ret = String::new();
        ret.push_str("Expected in acks:\n");
        for ack in ack_list {
            ret.push_str(&format!(
                "  ssi: {}, n: {}, retransmissions: {}, t_first: {:?}, t_umac_done: {:?}, state: {:?}\n",
                ack.addr.ssi,
                ack.ns,
                ack.retransmit_count,
                ack.t_first,
                ack.t_umac_done,
                ack.tx_reporter.get_state()
            ));
        }
        ret
    }

    fn format_scheduled_ack_list(ack_list: &Vec<ScheduledOutAck>) -> String {
        let mut ret = String::new();
        ret.push_str("Scheduled out acks:\n");
        for ack in ack_list {
            ret.push_str(&format!("  t_start: {}, ssi: {}, n: {}\n", ack.t_start.t, ack.addr.ssi, ack.nr));
        }
        ret
    }
}

impl TetraEntityTrait for Llc {
    fn entity(&self) -> TetraEntity {
        TetraEntity::Llc
    }

    fn set_config(&mut self, config: SharedConfig) {
        self.config = config;
    }

    fn rx_prim(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        tracing::debug!("rx_prim: {:?}", message);
        // tracing::debug!(ts=%message.dltime, "rx_prim: {:?}", message);

        match message.sap {
            Sap::TmaSap => {
                self.rx_tma_prim(queue, message);
            }

            // TMB-SAP and TMC-SAP are skipped and passed straight between MAC and MLE
            Sap::TlaSap => {
                self.rx_tla_prim(queue, message);
            }
            _ => panic!(),
        }
    }

    fn tick_start(&mut self, _queue: &mut MessageQueue, ts: TdmaTime) {
        self.dltime = ts;
    }

    fn tick_end(&mut self, queue: &mut MessageQueue, _ts: TdmaTime) -> bool {
        let mut had_activity = false;

        // Step 1 / 4: Check if we have any transmitted messages that were not acked within the expected window
        // Schedule a retransmission if appropriate.
        had_activity |= self.submit_retransmissions_to_umac(queue);

        // Step 2 / 4: Check if there are any messages that were not yet sent down, that we can now send down the stack
        // Messages may be kept since the target SSI has not yet acked them . If the link is now free, we can send the message down and register that we expect an ACK for it.
        had_activity |= self.submit_free_messages_to_umac(queue);

        // Step 3 / 4: Check if any unsent ACKs are still here
        // Take oldest element from scheduled_out_acks, and remove it from the list
        had_activity |= self.submit_ack_replies_to_umac(queue);

        // Step 4 / 4: Send any U-DATA messages
        had_activity |= self.submit_udata_msgs_to_umac(queue);

        had_activity
    }
}
