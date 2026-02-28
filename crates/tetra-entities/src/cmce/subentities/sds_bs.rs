use crate::MessageQueue;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tetra_config::bluestation::SharedConfig;
use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{BitBuffer, Sap, TdmaTime, TetraAddress, address::SsiType, unimplemented_log};
use tetra_pdus::cmce::{enums::cmce_pdu_type_ul::CmcePduTypeUl, pdus::d_sds_data::DSdsData, pdus::u_sds_data::USdsData};
use tetra_saps::lcmc::LcmcMleUnitdataReq;
use tetra_saps::{SapMsg, SapMsgInner};

// Local-only SDS delivery support for LST-like operation.
//
// In many terminals, delivery reports (PID=0x04) are addressed to SwMI rather than the
// originating MS. When we operate without backhaul, we must locally re-route those
// reports to the origin.
const PID04_REPORT_TIMEOUT: Duration = Duration::from_secs(8);

fn sds_pid04_key(dest_ssi: u32, mr: u8) -> u64 {
    ((dest_ssi as u64) << 8) | (mr as u64)
}

fn pid04_msg_type(payload: &[u8]) -> Option<u8> {
    payload.get(1).map(|b| (b & 0xF0) >> 4)
}

fn is_pid04_status_pdu(payload: &[u8]) -> bool {
    payload.len() >= 4 && payload.first().copied() == Some(0x04) && matches!(pid04_msg_type(payload), Some(1) | Some(2))
}

fn parse_pid04_status(payload: &[u8]) -> Option<(u8, u8, u8)> {
    // Returns (msg_type, status, mr)
    if payload.len() < 4 || payload[0] != 0x04 {
        return None;
    }
    let mt = pid04_msg_type(payload)?;
    if mt != 1 && mt != 2 {
        return None;
    }
    Some((mt, payload[2], payload[3]))
}

// Data observed: 04 <ctrl> <mr> ...
fn parse_pid04_mr_from_data(payload: &[u8]) -> Option<u8> {
    if payload.len() >= 3 && payload[0] == 0x04 {
        if is_pid04_status_pdu(payload) {
            return None;
        }
        return Some(payload[2]);
    }
    None
}

fn build_pid04_report(mr: u8, status: u8) -> [u8; 4] {
    [0x04, 0x10, status, mr]
}

fn build_pid04_ack(mr: u8, status: u8) -> [u8; 4] {
    [0x04, 0x20, status, mr]
}

// ─── Minimal SDS‑TL helpers (practical interop) ───────────────────

fn sds_tl_msg_type(payload: &[u8]) -> Option<u8> {
    payload.get(1).map(|b| (b & 0xF0) >> 4)
}

fn parse_sds_tl_transfer_pid_mr(payload: &[u8]) -> Option<(u8, u8)> {
    if payload.len() < 3 {
        return None;
    }
    if sds_tl_msg_type(payload) != Some(0) {
        return None;
    }
    let pid = payload[0];
    let mr = payload[2];
    Some((pid, mr))
}

fn build_sds_tl_ack(pid: u8, status: u8, mr: u8) -> [u8; 4] {
    [pid, 0x20, status, mr]
}

#[derive(Debug, Clone)]
struct PendingSdsPid04Route {
    origin_ssi: u32,
    mr: u8,
    since: Instant,
}

/// Clause 13 Short Data Service CMCE sub-entity (BS side)
///
/// BS primarily receives **U-SDS-DATA** on the air interface and forwards the
/// SDS Type-4 payload (including PID octet) to Brew/Core.
///
/// NOTE: We DO NOT rewrite SDS-TL Message Reference (MR). MR is used end-to-end
/// (e.g. delivery reports) and must be preserved per ETSI.
pub struct SdsBsSubentity {
    config: SharedConfig,
    pending_pid04_reports: HashMap<u64, PendingSdsPid04Route>,
    next_sds_dl_ts1: Option<TdmaTime>,
}

impl SdsBsSubentity {
    pub fn new(config: SharedConfig) -> Self {
        Self {
            config,
            pending_pid04_reports: HashMap::new(),
            next_sds_dl_ts1: None,
        }
    }

    pub fn set_config(&mut self, config: SharedConfig) {
        self.config = config;
    }

    fn should_handle_locally(&self, dst_ssi: u32) -> bool {
        let cfg = self.config.config();

        // Local-only build: always handle SDS when Brew is not configured.
        // When Brew is configured, only handle SDS for locally-routable SSI ranges.
        if cfg.brew.is_none() {
            return true;
        }
        cfg.cell.local_ssi_ranges.contains(dst_ssi)
    }

        // 2) Brew configured but disconnected => LST-like local operation.
        // Only handle SDS for locally-routable SSI ranges.
        // This avoids flooding MCCH/TS1 with synthetic ACKs for SDS that would normally be routed via the SwMI.
        if !self.config.state_read().network_connected {
            return cfg.cell.local_ssi_ranges.contains(dst_ssi);
        }

        // 3) Brew connected, but destination range is configured as local.
        cfg.cell.local_ssi_ranges.contains(dst_ssi)
    }

    fn reserve_sds_dl_ts1(&mut self, base: TdmaTime) -> TdmaTime {
        let candidate = base.forward_to_timeslot(1);
        let next = match self.next_sds_dl_ts1 {
            None => candidate,
            Some(cur) => {
                if cur.to_int() < candidate.to_int() {
                    candidate
                } else {
                    cur
                }
            }
        };
        self.next_sds_dl_ts1 = Some(next.add_timeslots(4));
        next
    }

    fn expire_pid04_routes(&mut self) {
        let now = Instant::now();
        let mut expired = Vec::new();
        for (k, v) in self.pending_pid04_reports.iter() {
            if now.duration_since(v.since) > PID04_REPORT_TIMEOUT {
                expired.push(*k);
            }
        }
        for k in expired {
            self.pending_pid04_reports.remove(&k);
        }
    }

    fn push_cmce_unitdata_req(&mut self, queue: &mut MessageQueue, base_time: TdmaTime, address: TetraAddress, mut sdu: BitBuffer, repeats: bool) {
        sdu.seek(0);
        queue.push_back(SapMsg {
            sap: Sap::LcmcSap,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Mle,
            dltime: self.reserve_sds_dl_ts1(base_time),
            msg: SapMsgInner::LcmcMleUnitdataReq(LcmcMleUnitdataReq {
                sdu,
                handle: 0,
                endpoint_id: 0,
                link_id: 0,
                layer2service: 0,
                pdu_prio: 0,
                layer2_qos: 0,
                stealing_permission: false,
                stealing_repeats_flag: repeats,
                chan_alloc: None,
                main_address: address,
                tx_reporter: None,
            }),
        });
    }

    fn send_dsds_type4(&mut self, queue: &mut MessageQueue, base_time: TdmaTime, dest: TetraAddress, sender_ssi: u32, bit_len: u16, payload: Vec<u8>, repeats: bool) {
        let pdu = DSdsData {
            calling_party_type_identifier: 1,
            calling_party_address_ssi: Some(sender_ssi as u64),
            calling_party_extension: None,
            short_data_type_identifier: 3,
            user_defined_data_1: None,
            user_defined_data_2: None,
            user_defined_data_3: None,
            length_indicator: Some(bit_len),
            user_defined_data_4: Some(payload),
            external_subscriber_number: None,
            dm_ms_address: None,
        };

        let mut sdu = BitBuffer::new_autoexpand(64 + (bit_len as usize));
        if let Err(e) = pdu.to_bitbuf(&mut sdu) {
            tracing::warn!("SDS local relay: DSdsData encode failed: {:?}", e);
            return;
        }

        self.push_cmce_unitdata_req(queue, base_time, dest, sdu, repeats);
    }
    fn rx_u_sds_data(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
        tracing::trace!("rx_u_sds_data");

        let SapMsgInner::LcmcMleUnitdataInd(prim) = &mut message.msg else {
            panic!();
        };

        let pdu = match USdsData::from_bitbuf(&mut prim.sdu) {
            Ok(pdu) => pdu,
            Err(e) => {
                tracing::warn!("Failed parsing USdsData: {:?} {}", e, prim.sdu.dump_bin());
                return;
            }
        };

        // We currently forward only Type-4 (SDTI=3) since it carries a variable-length payload.
        if pdu.short_data_type_identifier != 3 {
            tracing::debug!("USdsData sdti={} (not type-4); ignoring", pdu.short_data_type_identifier);
            return;
        }

        let Some(bit_len) = pdu.length_indicator else {
            tracing::warn!("USdsData type-4 missing length_indicator");
            return;
        };
        let Some(payload) = pdu.user_defined_data_4 else {
            tracing::warn!("USdsData type-4 missing user_defined_data_4");
            return;
        };

        // Destination SSI
        let (dst_ssi, dst_type) = if let Some(ssi) = pdu.called_party_ssi {
            (ssi as u32, SsiType::Ssi)
        } else if let Some(short) = pdu.called_party_short_number_address {
            (short as u32, SsiType::Unknown)
        } else {
            tracing::warn!("USdsData missing called_party address");
            return;
        };

        let src_addr = prim.received_tetra_address;
        let src_ssi = src_addr.ssi;
        let dst_addr = tetra_core::TetraAddress::new(dst_ssi, dst_type);

        self.expire_pid04_routes();

        let local = self.should_handle_locally(dst_ssi);

        tracing::info!(
            "SDS RX head: from={} dst={} local={} head={:02x?}",
            src_addr,
            dst_addr,
            local,
            payload.get(0..payload.len().min(8)).unwrap_or(&[])
        );
        // Interop: do not synthesize SDS-TL REPORTs here.
        // Many terminals accept only the destination-generated SDS-REPORT.
        // We relay destination reports end-to-end in BrewEntity.
        if local {
            // Local/LST handling: relay to destination (if applicable) and perform minimal
            // delivery report re-routing (PID=0x04) when terminals address status to SwMI.

            // 1) PID=0x04 status from destination → synthesize downlink report to origin.
            if let Some((mt, status, mr)) = parse_pid04_status(&payload) {
                let key = sds_pid04_key(src_ssi, mr);
                if let Some(route) = self.pending_pid04_reports.remove(&key) {
                    let out = match mt {
                        2 => build_pid04_ack(mr, status),
                        _ => build_pid04_report(mr, status),
                    };
                    tracing::info!(
                        "SDS PID04 {} from={} mr=0x{:02x} status=0x{:02x} -> origin={}",
                        if mt == 2 { "ACK" } else { "REPORT" },
                        src_ssi,
                        mr,
                        status,
                        route.origin_ssi
                    );
                    self.send_dsds_type4(
                        queue,
                        message.dltime,
                        TetraAddress::new(route.origin_ssi, SsiType::Ssi),
                        src_ssi,
                        32,
                        out.to_vec(),
                        true,
                    );
                } else {
                    tracing::debug!(
                        "SDS PID04 status from={} mr=0x{:02x} had no pending route (dropping)",
                        src_ssi,
                        mr
                    );
                }
                return;
            }

            let can_relay = dst_type == SsiType::Ssi && dst_ssi != src_ssi;

            // 2) Remember origin for PID=0x04 user data (needed for later REPORT/ACK).
            // Only meaningful for locally-routed MS↔MS SDS.
            if can_relay {
                if let Some(mr) = parse_pid04_mr_from_data(&payload) {
                    let key = sds_pid04_key(dst_ssi, mr);
                    self.pending_pid04_reports.insert(
                        key,
                        PendingSdsPid04Route {
                            origin_ssi: src_ssi,
                            mr,
                            since: Instant::now(),
                        },
                    );
                }
            }

            // 3) Many terminals require an SDS‑TL ACK to clear the sender UI.
            // Only send this for locally-routed MS↔MS SDS to avoid MCCH/TS1 flooding in LST.
            if can_relay {
                if let Some((pid, mr)) = parse_sds_tl_transfer_pid_mr(&payload) {
                    let ack = build_sds_tl_ack(pid, 0x00, mr);
                    self.send_dsds_type4(
                        queue,
                        message.dltime,
                        TetraAddress::new(src_ssi, SsiType::Ssi),
                        dst_ssi,
                        32,
                        ack.to_vec(),
                        true,
                    );
                }
            }

            // 4) Relay actual payload downlink to destination SSI.
            if can_relay {
                self.send_dsds_type4(
                    queue,
                    message.dltime,
                    TetraAddress::new(dst_ssi, SsiType::Ssi),
                    src_ssi,
                    bit_len,
                    payload,
                    false,
                );
            } else {
                tracing::debug!("SDS local: not relayed (dst_type={:?} dst_ssi={})", dst_type, dst_ssi);
            }
            return;
        }

        // Network/SwMI routing is intentionally not implemented in the local-only PR.
        tracing::debug!("SDS RX: dst not local; dropping src={} dst={}", src_addr, dst_addr);
    }

    #[allow(dead_code)]
    fn send_sds_tl_report_to_source(
        &mut self,
        queue: &mut MessageQueue,
        dltime: tetra_core::TdmaTime,
        handle: u32,
        endpoint_id: u32,
        link_id: u32,
        source_addr: TetraAddress,
        report_sender_ssi: u32,
        raw4: [u8; 4],
    ) {
        // Build a minimal D-SDS-DATA Type-4 with 32-bit SDS-TL status payload (PID + type + status + MR)
        let pdu = DSdsData {
            calling_party_type_identifier: 1,
            calling_party_address_ssi: Some(report_sender_ssi as u64),
            calling_party_extension: None,
            short_data_type_identifier: 3,
            user_defined_data_1: None,
            user_defined_data_2: None,
            user_defined_data_3: None,
            length_indicator: Some(32),
            user_defined_data_4: Some(raw4.to_vec()),
            external_subscriber_number: None,
            dm_ms_address: None,
        };

        let mut sdu = BitBuffer::new_autoexpand(96);
        if let Err(e) = pdu.to_bitbuf(&mut sdu) {
            tracing::warn!("SDS TX REPORT: encode failed: {:?}", e);
            return;
        }
        sdu.seek(0);

        tracing::info!(
            "SDS TX REPORT: to={} from_ssi={} raw4={:02x?}",
            source_addr,
            report_sender_ssi,
            raw4
        );

        queue.push_back(SapMsg {
            sap: Sap::LcmcSap,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Mle,
            dltime,
            msg: SapMsgInner::LcmcMleUnitdataReq(LcmcMleUnitdataReq {
                sdu,
                handle,
                endpoint_id,
                link_id,
                layer2service: 0,
                pdu_prio: 0,
                layer2_qos: 0,
                stealing_permission: false,
                stealing_repeats_flag: false,
                chan_alloc: None,
                main_address: source_addr,
                tx_reporter: None,
            }),
        });
    }


    /// Poor man's rx_prim, as this is a subcomponent and not governed by the MessageRouter.
    pub fn route_xx_deliver(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
        tracing::trace!("route_xx_deliver");

        let SapMsgInner::LcmcMleUnitdataInd(prim) = &mut message.msg else {
            panic!();
        };
        let Some(bits) = prim.sdu.peek_bits(5) else {
            tracing::warn!("insufficient bits: {}", prim.sdu.dump_bin());
            return;
        };

        let Ok(pdu_type) = CmcePduTypeUl::try_from(bits) else {
            tracing::warn!("invalid pdu type: {} in {}", bits, prim.sdu.dump_bin());
            return;
        };

        match pdu_type {
            CmcePduTypeUl::USdsData => self.rx_u_sds_data(queue, message),
            _ => unimplemented_log!("SdsBsSubentity route_xx_deliver {:?}", pdu_type),
        }
    }
}
