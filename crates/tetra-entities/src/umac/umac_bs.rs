use std::panic;

use tetra_config::SharedConfig;
use tetra_core::freqs::FreqInfo;
use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{BitBuffer, Direction, PhyBlockNum, Sap, SsiType, TdmaTime, TetraAddress, Todo, assert_warn, unimplemented_log};
use tetra_pdus::mle::fields::bs_service_details::BsServiceDetails;
use tetra_pdus::mle::pdus::d_mle_sync::DMleSync;
use tetra_pdus::mle::pdus::d_mle_sysinfo::DMleSysinfo;
use tetra_pdus::umac::enums::mac_pdu_type::MacPduType;
use tetra_pdus::umac::enums::sysinfo_opt_field_flag::SysinfoOptFieldFlag;
use tetra_pdus::umac::fields::channel_allocation::ChanAllocElement;
use tetra_pdus::umac::fields::sysinfo_default_def_for_access_code_a::SysinfoDefaultDefForAccessCodeA;
use tetra_pdus::umac::fields::sysinfo_ext_services::SysinfoExtendedServices;
use tetra_pdus::umac::pdus::mac_access::MacAccess;
use tetra_pdus::umac::pdus::mac_data::MacData;
use tetra_pdus::umac::pdus::mac_end_hu::MacEndHu;
use tetra_pdus::umac::pdus::mac_end_ul::MacEndUl;
use tetra_pdus::umac::pdus::mac_frag_ul::MacFragUl;
use tetra_pdus::umac::pdus::mac_resource::MacResource;
use tetra_pdus::umac::pdus::mac_sync::MacSync;
use tetra_pdus::umac::pdus::mac_sysinfo::MacSysinfo;
use tetra_pdus::umac::pdus::mac_u_blck::MacUBlck;
use tetra_pdus::umac::pdus::mac_u_signal::MacUSignal;
use tetra_saps::control::call_control::{CallControl, Circuit};
use tetra_saps::lcmc::enums::alloc_type::ChanAllocType;
use tetra_saps::lcmc::enums::ul_dl_assignment::UlDlAssignment;
use tetra_saps::lcmc::fields::chan_alloc_req::CmceChanAllocReq;
use tetra_saps::tma::{TmaReport, TmaReportInd, TmaUnitdataInd};
use tetra_saps::tmv::TmvConfigureReq;
use tetra_saps::tmv::enums::logical_chans::LogicalChannel;
use tetra_saps::{SapMsg, SapMsgInner};

use crate::lmac::components::scrambler;
use crate::umac::subcomp::bs_sched::{BsChannelScheduler, PrecomputedUmacPdus, TCH_S_CAP};
use crate::umac::subcomp::fillbits;
use crate::{MessagePrio, MessageQueue, TetraEntityTrait};

use super::subcomp::bs_defrag::BsDefrag;

pub struct UmacBs {
    self_component: TetraEntity,
    config: SharedConfig,
    dltime: TdmaTime,
    system_wide_services: bool,

    /// This MAC's endpoint ID, for addressing by the higher layers
    /// When using only a single base radio, we can set this to a fixed value
    endpoint_id: u32,

    /// Subcomponents
    defrag: BsDefrag,
    // event_label_store: EventLabelStore,
    /// Contains UL/DL scheduling logic
    /// Access to this field is used only by testing code
    pub channel_scheduler: BsChannelScheduler,
    // ulrx_scheduler: UlScheduler,
}

impl UmacBs {
    pub fn new(config: SharedConfig) -> Self {
        let c = config.config();
        let scrambling_code = scrambler::tetra_scramb_get_init(c.net.mcc, c.net.mnc, c.cell.colour_code);
        let system_wide_services = Self::get_system_wide_services_state(&config);
        let precomps = Self::generate_precomps(&config);
        Self {
            self_component: TetraEntity::Umac,
            config,
            dltime: TdmaTime::default(),
            system_wide_services,
            endpoint_id: 1,
            defrag: BsDefrag::new(),
            // event_label_store: EventLabelStore::new(),
            channel_scheduler: BsChannelScheduler::new(scrambling_code, precomps),
        }
    }

    /// Precomputes SYNC, SYSINFO messages (and subfield variants) for faster TX msg building
    /// Precomputed PDUs are passed to scheduler
    /// Needs to be re-invoked if any network parameter changes
    pub fn generate_precomps(config: &SharedConfig) -> PrecomputedUmacPdus {
        let c = config.config();

        // TODO FIXME make more/all parameters configurable
        let ext_services = SysinfoExtendedServices {
            auth_required: false,
            class1_supported: true,
            class2_supported: true,
            class3_supported: false,
            sck_n: Some(0),
            dck_retrieval_during_cell_select: None,
            dck_retrieval_during_cell_reselect: None,
            linked_gck_crypto_periods: None,
            short_gck_vn: None,
            sdstl_addressing_method: 2,
            gck_supported: false,
            section: 0,
            section_data: 0,
        };

        let def_access = SysinfoDefaultDefForAccessCodeA {
            imm: 8,
            wt: 5,
            nu: 5,
            fl_factor: false,
            ts_ptr: 0,
            min_pdu_prio: 0,
        };

        let sysinfo1 = MacSysinfo {
            main_carrier: c.cell.main_carrier,
            freq_band: c.cell.freq_band,
            freq_offset_index: FreqInfo::freq_offset_hz_to_id(c.cell.freq_offset_hz).unwrap(),
            duplex_spacing: c.cell.duplex_spacing_id,
            reverse_operation: c.cell.reverse_operation,
            num_of_csch: 0,
            ms_txpwr_max_cell: 5,
            rxlev_access_min: 3,
            access_parameter: 7,
            radio_dl_timeout: 3,
            cck_id: None,
            hyperframe_number: Some(0),
            option_field: SysinfoOptFieldFlag::DefaultDefForAccCodeA,
            ts_common_frames: None,
            default_access_code: Some(def_access),
            ext_services: None,
        };

        let sysinfo2 = MacSysinfo {
            main_carrier: sysinfo1.main_carrier,
            freq_band: sysinfo1.freq_band,
            freq_offset_index: sysinfo1.freq_offset_index,
            duplex_spacing: sysinfo1.duplex_spacing,
            reverse_operation: sysinfo1.reverse_operation,
            num_of_csch: sysinfo1.num_of_csch,
            ms_txpwr_max_cell: sysinfo1.ms_txpwr_max_cell,
            rxlev_access_min: sysinfo1.rxlev_access_min,
            access_parameter: sysinfo1.access_parameter,
            radio_dl_timeout: sysinfo1.radio_dl_timeout,
            cck_id: None,
            hyperframe_number: Some(0), // Updated dynamically in scheduler
            option_field: SysinfoOptFieldFlag::ExtServicesBroadcast,
            ts_common_frames: None,
            default_access_code: None,
            ext_services: Some(ext_services),
        };

        let system_wide_services = Self::get_system_wide_services_state(config);
        let mle_sysinfo_pdu = DMleSysinfo {
            location_area: c.cell.location_area,
            subscriber_class: 65535, // All subscriber classes allowed
            bs_service_details: BsServiceDetails {
                registration: c.cell.registration,
                deregistration: c.cell.deregistration,
                priority_cell: c.cell.priority_cell,
                no_minimum_mode: c.cell.no_minimum_mode,
                migration: c.cell.migration,
                system_wide_services,
                voice_service: true,
                circuit_mode_data_service: false,
                sndcp_service: false,
                aie_service: false,
                advanced_link: false,
            },
        };

        let mac_sync_pdu = MacSync {
            system_code: 1,
            colour_code: c.cell.colour_code,
            time: TdmaTime::default(),
            sharing_mode: 0, // Continuous transmission
            ts_reserved_frames: 0,
            u_plane_dtx: false,
            frame_18_ext: false,
        };

        let mle_sync_pdu = DMleSync {
            mcc: c.net.mcc,
            mnc: c.net.mnc,
            neighbor_cell_broadcast: 2, // Broadcast supported, but enquiry not supported
            cell_load_ca: 0,
            late_entry_supported: c.cell.late_entry_supported,
        };

        PrecomputedUmacPdus {
            mac_sysinfo1: sysinfo1,
            mac_sysinfo2: sysinfo2,
            mle_sysinfo: mle_sysinfo_pdu,
            mac_sync: mac_sync_pdu,
            mle_sync: mle_sync_pdu,
        }
    }

    /// Retrieve currently set value of system-wide services. If SwMI is active, this governs connection state
    /// Otherwise, value from config is used.
    fn get_system_wide_services_state(config: &SharedConfig) -> bool {
        let cfg = config.config();
        if cfg.brew.is_some() {
            config.state_read().network_connected
        } else {
            cfg.cell.system_wide_services
        }
    }

    fn refresh_system_wide_services(&mut self) {
        let is_effective = Self::get_system_wide_services_state(&self.config);
        if is_effective != self.system_wide_services {
            self.system_wide_services = is_effective;
            self.channel_scheduler.set_system_wide_services_state(is_effective);
            if is_effective {
                tracing::info!("UmacBs: system_wide_services ENABLED");
            } else {
                tracing::warn!("UmacBs: system_wide_services DISABLED");
            }
        }
    }

    fn cmce_to_mac_chanalloc(chan_alloc: &CmceChanAllocReq, carrier_num: u16) -> ChanAllocElement {
        // We grant clch permission for Replace and Additional allocations on the uplink
        let clch_permission = (chan_alloc.alloc_type == ChanAllocType::Replace || chan_alloc.alloc_type == ChanAllocType::Additional)
            && (chan_alloc.ul_dl_assigned == UlDlAssignment::Ul || chan_alloc.ul_dl_assigned == UlDlAssignment::Both);
        ChanAllocElement {
            alloc_type: chan_alloc.alloc_type,
            ts_assigned: chan_alloc.timeslots,
            ul_dl_assigned: chan_alloc.ul_dl_assigned,
            clch_permission,
            cell_change_flag: false,
            carrier_num,
            ext: None,
            mon_pattern: 0,
            frame18_mon_pattern: Some(0),
        }
    }

    /// Convenience function to send a TMA-REPORT.ind
    fn send_tma_report_ind(queue: &mut MessageQueue, dltime: TdmaTime, handle: Todo, report: TmaReport) {
        let tma_report_ind = TmaReportInd {
            req_handle: handle,
            report,
        };
        let msg = SapMsg {
            sap: Sap::TmaSap,
            src: TetraEntity::Umac,
            dest: TetraEntity::Llc,
            dltime,
            msg: SapMsgInner::TmaReportInd(tma_report_ind),
        };
        queue.push_back(msg);
    }

    fn rx_tmv_prim(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        tracing::trace!("rx_tmv_prim");
        match message.msg {
            SapMsgInner::TmvUnitdataInd(_) => {
                self.rx_tmv_unitdata_ind(queue, message);
            }
            _ => {
                panic!();
            }
        }
    }

    pub fn rx_tmv_unitdata_ind(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
        let SapMsgInner::TmvUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };
        tracing::trace!("rx_tmv_unitdata_ind: {:?}", prim.logical_channel);

        match prim.logical_channel {
            LogicalChannel::SchF => {
                // Full slot signalling
                assert!(
                    prim.block_num == PhyBlockNum::Both,
                    "{:?} can't have block_num {:?}",
                    prim.logical_channel,
                    prim.block_num
                );
                self.rx_tmv_sch(queue, message);
            }
            LogicalChannel::Stch | LogicalChannel::SchHu => {
                // Half slot signalling
                assert!(
                    matches!(prim.block_num, PhyBlockNum::Block1 | PhyBlockNum::Block2),
                    "{:?} can't have block_num {:?}",
                    prim.logical_channel,
                    prim.block_num
                );
                self.rx_tmv_sch(queue, message);
            }
            _ => unreachable!("invalid channel: {:?}", prim.logical_channel),
        }
    }

    /// Receive signalling (SCH, or STCH / BNCH)
    pub fn rx_tmv_sch(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
        tracing::trace!("rx_tmv_sch");

        // Iterate until no more messages left in mac block
        loop {
            // Extract info from inner block
            let SapMsgInner::TmvUnitdataInd(prim) = &message.msg else {
                panic!()
            };
            let Some(bits) = prim.pdu.peek_bits(3) else {
                tracing::warn!("insufficient bits: {}", prim.pdu.dump_bin());
                return;
            };
            let orig_start = prim.pdu.get_raw_start();
            let lchan = prim.logical_channel;

            // Clause 21.4.1; handling differs between SCH_HU and others
            match lchan {
                LogicalChannel::SchF | LogicalChannel::Stch => {
                    // First two bits are MAC PDU type
                    let Ok(pdu_type) = MacPduType::try_from(bits >> 1) else {
                        tracing::warn!("invalid pdu type: {}", bits >> 1);
                        return;
                    };

                    match pdu_type {
                        MacPduType::MacResourceMacData => {
                            self.rx_mac_data(queue, &mut message);
                        }
                        MacPduType::MacFragMacEnd => {
                            // Also need third bit; designates mac-frag versus mac-end
                            if bits & 1 == 0 {
                                self.rx_mac_frag_ul(queue, &mut message);
                            } else {
                                self.rx_mac_end_ul(queue, &mut message);
                            }
                        }
                        MacPduType::SuppMacUSignal => {
                            // STCH determines which subtype is relevant
                            if lchan == LogicalChannel::Stch {
                                self.rx_ul_mac_u_signal(queue, &mut message);
                            } else {
                                // Supplementary MAC PDU type
                                if bits & 1 == 0 {
                                    self.rx_ul_mac_u_blck(queue, &mut message);
                                } else {
                                    tracing::warn!("unexpected supplementary PDU type")
                                }
                            }
                        }
                        _ => {
                            tracing::warn!("unknown pdu type: {}", pdu_type);
                        }
                    }
                }
                LogicalChannel::SchHu => {
                    // Need only 1 bit for a single subtype distinction
                    let pdu_type = (bits >> 2) & 1;
                    match pdu_type {
                        0 => self.rx_mac_access(queue, &mut message),
                        1 => self.rx_mac_end_hu(queue, &mut message),
                        _ => panic!(),
                    }
                }

                _ => {
                    tracing::warn!("unknown logical channel: {:?}", lchan);
                }
            }

            // Check if end of message reached by re-borrowing inner
            // If start was not updated, we also consider it end of message
            // If 16 or more bits remain (len of null pdu), we continue parsing
            if let SapMsgInner::TmvUnitdataInd(prim) = &message.msg {
                if prim.pdu.get_raw_start() != orig_start && prim.pdu.get_len() >= 16 {
                    tracing::trace!("orig {} now {}", orig_start, prim.pdu.get_raw_start());
                    tracing::trace!(
                        "rx_tmv_unitdata_ind_sch: Remaining {} bits: {:?}",
                        prim.pdu.get_len_remaining(),
                        prim.pdu.dump_bin_full(true)
                    );
                } else {
                    tracing::trace!("rx_tmv_unitdata_ind_sch: End of message reached");
                    break;
                }
            }
        }
    }

    fn rx_mac_data(&mut self, queue: &mut MessageQueue, message: &mut SapMsg) {
        tracing::trace!("rx_mac_data");
        let SapMsgInner::TmvUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };
        assert!(prim.pdu.get_pos() == 0); // We should be at the start of the MAC PDU

        let pdu = match MacData::from_bitbuf(&mut prim.pdu) {
            Ok(pdu) => {
                tracing::debug!("<- {:?}", pdu);
                pdu
            }
            Err(e) => {
                tracing::warn!("Failed parsing MacData: {:?} {}", e, prim.pdu.dump_bin());
                return;
            }
        };

        // Get addr, either from pdu addr field or by resolving the event label
        if pdu.event_label.is_some() {
            unimplemented_log!("event labels not implemented");
            return;
        }
        let addr = pdu.addr.unwrap();

        // Compute len and extract flags
        let (mut pdu_len_bits, is_frag_start, second_half_stolen, is_null_pdu) = {
            if let Some(len_ind) = pdu.length_ind {
                // We have a lenght ind, either clear length, a stolen slot indication, or a fragmentation start
                match len_ind {
                    0b000000 => {
                        // Null PDU
                        (if pdu.event_label.is_some() { 23 } else { 37 }, false, false, true)
                    }

                    0b000010..0b111000 => {
                        // tracing::trace!("rx_mac_data: length_ind {}", len_ind);
                        (len_ind as usize * 8, false, false, false)
                    }
                    0b111110 => {
                        // Second half slot stolen in STCH
                        (prim.pdu.get_len(), false, true, false)
                    }
                    0b111111 => {
                        // Start of fragmentation
                        // tracing::trace!("rx_mac_data: frag_start");
                        (prim.pdu.get_len(), true, false, false)
                    }
                    _ => panic!("rx_mac_data: Invalid length_ind {}", len_ind),
                }
            } else {
                // We have a capacity request
                tracing::trace!(
                    "rx_mac_data: cap_req {}",
                    if pdu.frag_flag.unwrap() { "with frag_start" } else { "" }
                );
                (prim.pdu.get_len(), pdu.frag_flag.unwrap(), false, false)
            }
        };

        // Truncate len if past end (okay with standard)
        if pdu_len_bits > prim.pdu.get_len() {
            tracing::warn!("truncating MAC-DATA len from {} to {}", pdu_len_bits, prim.pdu.get_len());
            pdu_len_bits = prim.pdu.get_len() as usize;
        }

        // Strip fill bits. Maintain original end to allow for later parsing of a second mac block
        tracing::trace!("rx_mac_data: {}", prim.pdu.dump_bin_full(true));
        let num_fill_bits = {
            if pdu.fill_bits {
                fillbits::removal::get_num_fill_bits(&prim.pdu, pdu_len_bits, is_null_pdu)
            } else {
                0
            }
        };
        pdu_len_bits -= num_fill_bits;
        let orig_end = prim.pdu.get_raw_end();
        prim.pdu.set_raw_end(prim.pdu.get_raw_start() + pdu_len_bits);
        tracing::trace!(
            "rx_mac_data: pdu: {} sdu: {} fb: {}: {}",
            pdu_len_bits,
            prim.pdu.get_len_remaining(),
            num_fill_bits,
            prim.pdu.dump_bin_full(true)
        );

        if is_null_pdu {
            // TODO not sure if there is scenarios in which we want to pass a null pdu to the LLC
            // tracing::warn!("rx_mac_data: Null PDU not passed to LLC");
            return;
        }

        // Decrypt if needed
        if pdu.encrypted {
            unimplemented_log!("rx_mac_data: Encryption mode > 0");
            return;
        }

        // Handle reservation if present
        // let ul_time = message.dltime.add_timeslots(-2);
        if let Some(res_req) = &pdu.reservation_req {
            tracing::error!("rx_mac_data: time {:?}", message.dltime);
            let grant = self.channel_scheduler.ul_process_cap_req(message.dltime.t, addr, res_req);
            if let Some(grant) = grant {
                // Schedule grant
                self.channel_scheduler.dl_enqueue_grant(message.dltime.t, addr, grant);
            } else {
                tracing::warn!("rx_mac_data: No grant for reservation request {:?}", res_req);
            }
        };

        tracing::debug!("rx_mac_data: {}", prim.pdu.dump_bin_full(true));
        if is_frag_start {
            // Fragmentation start, add to defragmenter
            self.defrag.insert_first(&mut prim.pdu, message.dltime, addr, None);
        } else if second_half_stolen {
            // Signal LMAC that Block2 is also stolen (STCH, not TCH).
            // Must be Immediate priority so LMAC sees it before processing Block2.
            let m = SapMsg {
                sap: Sap::TmvSap,
                src: self.self_component,
                dest: TetraEntity::Lmac,
                dltime: message.dltime,
                msg: SapMsgInner::TmvConfigureReq(TmvConfigureReq {
                    second_half_stolen: Some(true),
                    ..Default::default()
                }),
            };
            queue.push_prio(m, MessagePrio::Immediate);
        } else {
            // Pass directly to LLC
            let sdu = {
                // if prim.pdu.get_len_remaining() == 0 {
                //     None // No more data in this block
                // } else {
                // TODO FIXME should not copy here but take ownership
                // Copy inner part, without MAC header or fill bits
                Some(BitBuffer::from_bitbuffer_pos(&prim.pdu))
                // }
            };

            // Try to grant reservation requirement if present
            if let Some(_reservation_requirement) = pdu.reservation_req {
                // let grant = self.ulrx_scheduler.process_granting_request(pdu.addr, &reservation_requirement);
                // if let Some(grant) = grant {
                //     self.channel_scheduler.schedule_grant(pdu.addr, grant);
                // }
                unimplemented_log!("rx_mac_data: Reservation requests not implemented");
            }

            if sdu.is_some() {
                // We have an SDU for the LLC, deliver it.
                let m = SapMsg {
                    sap: Sap::TmaSap,
                    src: TetraEntity::Umac,
                    dest: TetraEntity::Llc,
                    dltime: message.dltime,

                    msg: SapMsgInner::TmaUnitdataInd(TmaUnitdataInd {
                        pdu: sdu,
                        main_address: addr,
                        scrambling_code: prim.scrambling_code,
                        endpoint_id: 0,        // TODO FIXME
                        new_endpoint_id: None, // TODO FIXME
                        css_endpoint_id: None, // TODO FIXME
                        air_interface_encryption: pdu.encrypted as Todo,
                        chan_change_response_req: false,
                        chan_change_handle: None,
                        chan_info: None,
                    }),
                };
                queue.push_back(m);
            } else {
                // Either this is a null pdu or we are at the end of the block
                // For now, we don't deliver this. However, important data may need to be signalled upwards
                tracing::warn!("rx_mac_data: empty PDU not passed to LLC");
            }
        }

        // Since this is not a null pdu, more MAC PDUs may follow
        // This allows parent function to continue parsing
        prim.pdu.set_raw_end(orig_end);
        prim.pdu.set_raw_pos(prim.pdu.get_raw_start() + pdu_len_bits + num_fill_bits);
        prim.pdu.set_raw_start(prim.pdu.get_raw_pos());
    }

    fn rx_mac_access(&mut self, queue: &mut MessageQueue, message: &mut SapMsg) {
        tracing::trace!("rx_mac_access");
        let SapMsgInner::TmvUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };
        assert!(prim.pdu.get_pos() == 0); // We should be at the start of the MAC PDU

        let pdu = match MacAccess::from_bitbuf(&mut prim.pdu) {
            Ok(pdu) => {
                tracing::debug!("<- {:?}", pdu);
                pdu
            }
            Err(e) => {
                tracing::warn!("Failed parsing MacAccess: {:?} {}", e, prim.pdu.dump_bin());
                return;
            }
        };

        // Resolve event label (if supplied)
        let addr = if let Some(_label) = pdu.event_label {
            tracing::warn!("event labels not implemented");
            return;
            // let ret = self.event_label_store.get_addr_by_label(label);
            // if let Some(ssi) = ret {
            //     ssi
            // } else {
            //     tracing::warn!("Could not resolve event label for {}", label);
            //     return;
            // }
        } else if let Some(addr) = pdu.addr {
            addr
        } else {
            panic!()
        };

        // Compute len and extract flags
        let mut pdu_len_bits;
        if let Some(length_ind) = pdu.length_ind {
            if length_ind == 0 {
                // Null PDU
                if pdu.event_label.is_some() {
                    // Short event label present
                    pdu_len_bits = 22; // 22 bits for event label
                } else {
                    // SSI
                    pdu_len_bits = 36;
                }
            } else {
                // Full length ind
                pdu_len_bits = length_ind as usize * 8;
            }
        } else {
            // No length ind, we have capacity request. Fill slot.
            pdu_len_bits = prim.pdu.get_len();
        }
        if pdu_len_bits > prim.pdu.get_len() {
            tracing::warn!("truncating MAC-ACCESS len from {} to {}", pdu_len_bits, prim.pdu.get_len());
            pdu_len_bits = prim.pdu.get_len();
        }

        // Strip fill bits. Maintain original end to allow for later parsing of a second mac block
        // tracing::trace!("rx_mac_access: {}", prim.pdu.dump_bin_full(true));
        let num_fill_bits = if pdu.fill_bits {
            fillbits::removal::get_num_fill_bits(&prim.pdu, pdu_len_bits, pdu.is_null_pdu())
        } else {
            0
        };
        pdu_len_bits -= num_fill_bits;
        let orig_end = prim.pdu.get_raw_end();
        prim.pdu.set_raw_end(prim.pdu.get_raw_start() + pdu_len_bits);
        tracing::trace!(
            "rx_mac_access: pdu: {} sdu: {} fb: {}: {}",
            pdu_len_bits,
            prim.pdu.get_len_remaining(),
            num_fill_bits,
            prim.pdu.dump_bin_full(true)
        );

        if pdu.is_null_pdu() {
            // tracing::warn!("rx_mac_access: Null PDU not passed to LLC");
            return;
        }

        // Schedule acknowledgement of this message
        // let ul_time = message.dltime.add_timeslots(-2);
        self.channel_scheduler.dl_enqueue_random_access_ack(message.dltime.t, addr);

        // Decrypt if needed
        if pdu.encrypted {
            unimplemented_log!("rx_mac_access: Encryption mode > 0");
            return;
        }

        // Handle reservation if present
        if let Some(res_req) = &pdu.reservation_req {
            let grant = self.channel_scheduler.ul_process_cap_req(message.dltime.t, addr, res_req);
            if let Some(grant) = grant {
                // Schedule grant
                self.channel_scheduler.dl_enqueue_grant(message.dltime.t, addr, grant);
            } else {
                tracing::warn!("rx_mac_access: No grant for reservation request {:?}", res_req);
            }
        };

        // tracing::debug!("rx_mac_access: {}", prim.pdu.dump_bin_full(true));
        if pdu.is_frag_start() {
            // Fragmentation start, add to defragmenter
            self.defrag.insert_first(&mut prim.pdu, message.dltime, addr, None);
        } else {
            // Pass directly to LLC
            if prim.pdu.get_len_remaining() == 0 {
                // Either this is a null pdu or we are at the end of the block
                // For now, we don't deliver this. However, important data may need to be signalled upwards
                tracing::warn!("rx_mac_access: empty PDU not passed to LLC");
                return;
            };

            // Pass directly to LLC
            let sdu = {
                if prim.pdu.get_len_remaining() == 0 {
                    None // No more data in this block
                } else {
                    // TODO FIXME check if there is a reasonable way to avoid copying here by taking ownership
                    Some(BitBuffer::from_bitbuffer_pos(&prim.pdu))
                }
            };

            if sdu.is_some() {
                // We have an SDU for the LLC, deliver it.
                let m = SapMsg {
                    sap: Sap::TmaSap,
                    src: TetraEntity::Umac,
                    dest: TetraEntity::Llc,
                    dltime: message.dltime,

                    msg: SapMsgInner::TmaUnitdataInd(TmaUnitdataInd {
                        pdu: sdu,
                        main_address: addr,
                        scrambling_code: prim.scrambling_code,
                        endpoint_id: 0,        // TODO FIXME
                        new_endpoint_id: None, // TODO FIXME
                        css_endpoint_id: None, // TODO FIXME
                        air_interface_encryption: pdu.encrypted as Todo,
                        chan_change_response_req: false,
                        chan_change_handle: None,
                        chan_info: None,
                    }),
                };
                queue.push_back(m);
            } else {
                // Either this is a null pdu or we are at the end of the block
                // For now, we don't deliver this. However, important data may need to be signalled upwards
                tracing::warn!("rx_mac_data: empty PDU not passed to LLC");
            }
        }

        // Since this is not a null pdu, more MAC PDUs may follow
        // This allows parent function to continue parsing
        prim.pdu.set_raw_end(orig_end);
        prim.pdu.set_raw_pos(prim.pdu.get_raw_start() + pdu_len_bits + num_fill_bits);
        prim.pdu.set_raw_start(prim.pdu.get_raw_pos());
    }

    fn rx_mac_frag_ul(&mut self, _queue: &mut MessageQueue, message: &mut SapMsg) {
        tracing::trace!("rx_mac_frag_ul");
        let SapMsgInner::TmvUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };
        assert!(prim.pdu.get_pos() == 0); // We should be at the start of the MAC PDU

        // Parse header and optional ChanAlloc
        let pdu = match MacFragUl::from_bitbuf(&mut prim.pdu) {
            Ok(pdu) => {
                tracing::debug!("<- {:?}", pdu);
                pdu
            }
            Err(e) => {
                tracing::warn!("Failed parsing MacFragUl: {:?} {}", e, prim.pdu.dump_bin());
                return;
            }
        };

        // Strip fill bits. This message is known to fill the slot.
        let mut pdu_len_bits = prim.pdu.get_len();
        let num_fill_bits = {
            if pdu.fill_bits {
                fillbits::removal::get_num_fill_bits(&prim.pdu, pdu_len_bits, false)
            } else {
                0
            }
        };
        pdu_len_bits -= num_fill_bits;
        prim.pdu.set_raw_end(prim.pdu.get_raw_start() + pdu_len_bits);
        tracing::debug!("rx_mac_frag_ul: pdu_len_bits: {} fill_bits: {}", pdu_len_bits, num_fill_bits);

        // Get slot owner from schedule, decrypt if needed
        // let ul_time = message.dltime.add_timeslots(-2);
        let Some(slot_owner) = self.channel_scheduler.ul_get_slot_owner(message.dltime, prim.block_num) else {
            tracing::warn!("rx_mac_frag_ul: Received MAC-FRAG-UL for unassigned block {:?}", prim.block_num);
            self.channel_scheduler.dump_ul_schedule_full(true);
            return;
        };
        if let Some(_aie_info) = self.defrag.get_aie_info(slot_owner, message.dltime) {
            unimplemented_log!("rx_mac_frag_ul: Encryption not supported");
            return;
        }

        // Insert into defragmenter
        self.defrag.insert_next(&mut prim.pdu, slot_owner, message.dltime);
    }

    fn rx_mac_end_ul(&mut self, queue: &mut MessageQueue, message: &mut SapMsg) {
        tracing::trace!("rx_mac_end_ul");
        let SapMsgInner::TmvUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };
        assert!(prim.pdu.get_pos() == 0); // We should be at the start of the MAC PDU

        // Parse header and optional ChanAlloc
        let pdu = match MacEndUl::from_bitbuf(&mut prim.pdu) {
            Ok(pdu) => {
                tracing::debug!("<- {:?}", pdu);
                pdu
            }
            Err(e) => {
                tracing::warn!("Failed parsing MacEndUl: {:?} {}", e, prim.pdu.dump_bin());
                return;
            }
        };

        // Will have either length_ind or reservation_req, never none or both
        let mut pdu_len_bits = if let Some(length_ind) = pdu.length_ind {
            length_ind as usize * 8
        } else {
            // No length ind, we have capacity request. Fill slot.
            prim.pdu.get_len()
        };
        if pdu_len_bits > prim.pdu.get_len() {
            tracing::warn!("truncating MAC-END-UL len from {} to {}", pdu_len_bits, prim.pdu.get_len());
            pdu_len_bits = prim.pdu.get_len();
        }

        // Strip fill bits if any
        let num_fill_bits = {
            if pdu.fill_bits {
                fillbits::removal::get_num_fill_bits(&prim.pdu, pdu_len_bits, false)
            } else {
                0
            }
        };
        pdu_len_bits -= num_fill_bits;
        let orig_end = prim.pdu.get_raw_end();
        prim.pdu.set_raw_end(prim.pdu.get_raw_start() + pdu_len_bits);
        tracing::trace!(
            "rx_mac_end_ul: pdu: {} sdu: {} fb: {}: {}",
            pdu_len_bits,
            prim.pdu.get_len_remaining(),
            num_fill_bits,
            prim.pdu.dump_bin_full(true)
        );

        // Get slot owner from schedule, decrypt if needed
        // let ul_time = message.dltime.add_timeslots(-2);
        let Some(slot_owner) = self.channel_scheduler.ul_get_slot_owner(message.dltime, prim.block_num) else {
            tracing::warn!("rx_mac_end_ul: Received MAC-END-UL for unassigned block {:?}", prim.block_num);
            self.channel_scheduler.dump_ul_schedule_full(true);
            return;
        };
        if let Some(_aie_info) = self.defrag.get_aie_info(slot_owner, message.dltime) {
            unimplemented!("rx_mac_end_ul: Encryption not supported");
        }

        // Insert last fragment and retrieve finalized block
        let defragbuf = self.defrag.insert_last(&mut prim.pdu, slot_owner, message.dltime);
        let Some(defragbuf) = defragbuf else {
            tracing::warn!("rx_mac_end_ul: could not obtain defragged buf");
            return;
        };

        // Handle reservation if present
        if let Some(res_req) = &pdu.reservation_req {
            let grant = self.channel_scheduler.ul_process_cap_req(message.dltime.t, defragbuf.addr, res_req);
            if let Some(grant) = grant {
                // Schedule grant
                self.channel_scheduler.dl_enqueue_grant(message.dltime.t, defragbuf.addr, grant);
            } else {
                tracing::warn!("rx_mac_end_ul: No grant for reservation request {:?}", res_req);
            }
        };

        // Pass completed block to LLC
        tracing::debug!("rx_mac_end_ul: sdu: {:?}", defragbuf.buffer.dump_bin());

        let m = SapMsg {
            sap: Sap::TmaSap,
            src: TetraEntity::Umac,
            dest: TetraEntity::Llc,
            dltime: message.dltime,

            msg: SapMsgInner::TmaUnitdataInd(TmaUnitdataInd {
                pdu: Some(defragbuf.buffer),
                main_address: defragbuf.addr,
                scrambling_code: prim.scrambling_code,
                endpoint_id: 0,              // TODO FIXME
                new_endpoint_id: None,       // TODO FIXME
                css_endpoint_id: None,       // TODO FIXME
                air_interface_encryption: 0, // TODO FIXME implement
                chan_change_response_req: false,
                chan_change_handle: None,
                chan_info: None,
            }),
        };
        queue.push_back(m);

        // Since this is not a null pdu, more MAC PDUs may follow
        // This allows parent function to continue parsing
        prim.pdu.set_raw_end(orig_end);
        prim.pdu.set_raw_pos(prim.pdu.get_raw_start() + pdu_len_bits + num_fill_bits);
        prim.pdu.set_raw_start(prim.pdu.get_raw_pos());
    }

    fn rx_mac_end_hu(&mut self, queue: &mut MessageQueue, message: &mut SapMsg) {
        tracing::trace!("rx_mac_end_hu");
        let SapMsgInner::TmvUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };
        assert!(prim.pdu.get_pos() == 0); // We should be at the start of the MAC PDU

        // Parse header and optional ChanAlloc
        let pdu = match MacEndHu::from_bitbuf(&mut prim.pdu) {
            Ok(pdu) => {
                tracing::debug!("<- {:?}", pdu);
                pdu
            }
            Err(e) => {
                tracing::warn!("Failed parsing MacEndHu: {:?} {}", e, prim.pdu.dump_bin());
                return;
            }
        };

        // Will have either length_ind or reservation_req, never none or both
        let mut pdu_len_bits = if let Some(length_ind) = pdu.length_ind {
            if length_ind == 0 {
                assert_warn!(false, "rx_mac_end_hu: PDU has length ind 0");
                return;
            }
            let len = length_ind as usize * 8;
            if len > prim.pdu.get_len() { prim.pdu.get_len() } else { len }
        } else {
            // No length ind, we have capacity request. Fill slot.
            prim.pdu.get_len()
        };
        if pdu_len_bits > prim.pdu.get_len() {
            tracing::warn!("truncating MAC-END-HU len from {} to {}", pdu_len_bits, prim.pdu.get_len());
            pdu_len_bits = prim.pdu.get_len();
        }

        // Strip fill bits if any
        let num_fill_bits = {
            if pdu.fill_bits {
                fillbits::removal::get_num_fill_bits(&prim.pdu, pdu_len_bits, false)
            } else {
                0
            }
        };
        pdu_len_bits -= num_fill_bits;
        let orig_end = prim.pdu.get_raw_end();
        prim.pdu.set_raw_end(prim.pdu.get_raw_start() + pdu_len_bits);
        // tracing::error!("rx_mac_end_hu: orig_end {} raw_start {} num_fill_bits {} curr_pos {}", orig_end, prim.pdu.get_raw_start(), num_fill_bits, prim.pdu.get_raw_pos());
        // set to trace
        tracing::trace!(
            "rx_mac_end_hu: pdu: {} sdu: {} fb: {}: {}",
            pdu_len_bits,
            prim.pdu.get_len_remaining(),
            num_fill_bits,
            prim.pdu.dump_bin_full(true)
        );

        // Get slot owner from schedule, decrypt if needed
        // let ul_time = message.dltime.add_timeslots(-2);
        let Some(slot_owner) = self.channel_scheduler.ul_get_slot_owner(message.dltime, prim.block_num) else {
            tracing::warn!("rx_mac_end_hu: Received MAC-END-HU for unassigned block {:?}", prim.block_num);
            self.channel_scheduler.dump_ul_schedule_full(true);
            return;
        };
        if let Some(_aie_info) = self.defrag.get_aie_info(slot_owner, message.dltime) {
            unimplemented!("rx_mac_end_hu: Encryption not supported");
        }

        // Insert last fragment and retrieve finalized block
        let defragbuf = self.defrag.insert_last(&mut prim.pdu, slot_owner, message.dltime);
        let Some(defragbuf) = defragbuf else {
            tracing::warn!("rx_mac_end_hu: could not obtain defragged buf");
            return;
        };

        // Handle reservation if present
        if let Some(res_req) = &pdu.reservation_req {
            let grant = self.channel_scheduler.ul_process_cap_req(message.dltime.t, defragbuf.addr, res_req);
            if let Some(grant) = grant {
                // Schedule grant
                self.channel_scheduler.dl_enqueue_grant(message.dltime.t, defragbuf.addr, grant);
            } else {
                tracing::warn!("rx_mac_end_hu: No grant for reservation request {:?}", res_req);
            }
        };

        // Pass completed block to LLC
        tracing::debug!("rx_mac_end_hu: sdu: {:?}", defragbuf.buffer.dump_bin());

        let m = SapMsg {
            sap: Sap::TmaSap,
            src: TetraEntity::Umac,
            dest: TetraEntity::Llc,
            dltime: message.dltime,

            msg: SapMsgInner::TmaUnitdataInd(TmaUnitdataInd {
                pdu: Some(defragbuf.buffer),
                main_address: defragbuf.addr,
                scrambling_code: prim.scrambling_code,
                endpoint_id: 0,              // TODO FIXME
                new_endpoint_id: None,       // TODO FIXME
                css_endpoint_id: None,       // TODO FIXME
                air_interface_encryption: 0, // TODO FIXME implement
                chan_change_response_req: false,
                chan_change_handle: None,
                chan_info: None,
            }),
        };
        queue.push_back(m);

        // Since this is not a null pdu, more MAC PDUs may follow
        // This allows parent function to continue parsing
        // tracing::trace!("rx_mac_end_hu: orig_end {} raw_start {} num_fill_bits {} curr_pos {}", orig_end, prim.pdu.get_raw_start(), num_fill_bits, prim.pdu.get_raw_pos());
        prim.pdu.set_raw_end(orig_end);
        prim.pdu.set_raw_pos(prim.pdu.get_raw_start() + pdu_len_bits + num_fill_bits);
        prim.pdu.set_raw_start(prim.pdu.get_raw_pos());
    }

    /// UL MAC-U-SIGNAL on STCH: extract TM-SDU and forward to LLC → MLE → CMCE.
    /// This carries signaling like U-TX CEASED / U-TX DEMAND on the traffic channel.
    fn rx_ul_mac_u_signal(&self, queue: &mut MessageQueue, message: &mut SapMsg) {
        tracing::trace!("rx_ul_mac_u_signal");

        // Extract sdu and parse pdu
        let SapMsgInner::TmvUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };

        let pdu = match MacUSignal::from_bitbuf(&mut prim.pdu) {
            Ok(pdu) => {
                tracing::debug!("<- {:?}", pdu);
                pdu
            }
            Err(e) => {
                tracing::warn!("Failed parsing MacUSignal: {:?} {}", e, prim.pdu.dump_bin());
                return;
            }
        };

        if pdu.second_half_stolen {
            tracing::warn!("rx_ul_mac_u_signal: second_half_stolen not implemented");
            return;
        }

        // The remaining bits after the MAC-U-SIGNAL header are the TM-SDU (LLC PDU)
        if prim.pdu.get_len_remaining() == 0 {
            tracing::trace!("rx_ul_mac_u_signal: empty TM-SDU");
            return;
        }

        let sdu = BitBuffer::from_bitbuffer_pos(&prim.pdu);
        tracing::debug!("rx_ul_mac_u_signal: forwarding {} bit TM-SDU to LLC", sdu.get_len());

        // Forward to LLC via TMA-SAP, same path as MAC-DATA.
        // Address is not known from MAC-U-SIGNAL (no address field); use a placeholder.
        // The CMCE layer identifies the call by call_identifier in the PDU, not by address.
        let m = SapMsg {
            sap: Sap::TmaSap,
            src: TetraEntity::Umac,
            dest: TetraEntity::Llc,
            dltime: message.dltime,
            msg: SapMsgInner::TmaUnitdataInd(TmaUnitdataInd {
                pdu: Some(sdu),
                main_address: TetraAddress::new(0, SsiType::Ssi), // Address unknown from MAC-U-SIGNAL
                scrambling_code: prim.scrambling_code,
                endpoint_id: 0,
                new_endpoint_id: None,
                css_endpoint_id: None,
                air_interface_encryption: 0,
                chan_change_response_req: false,
                chan_change_handle: None,
                chan_info: None,
            }),
        };
        queue.push_back(m);
    }

    /// TMA-SAP MAC-U-BLCK
    fn rx_ul_mac_u_blck(&self, _queue: &mut MessageQueue, message: &mut SapMsg) {
        tracing::trace!("rx_ul_mac_u_blck");

        // Extract sdu and parse pdu
        let SapMsgInner::TmvUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };

        let _pdu = match MacUBlck::from_bitbuf(&mut prim.pdu) {
            Ok(pdu) => {
                tracing::debug!("<- {:?}", pdu);
                pdu
            }
            Err(e) => {
                tracing::warn!("Failed parsing MacUBlck: {:?} {}", e, prim.pdu.dump_bin());
                return;
            }
        };

        // Handle reservation if present
        // TODO implement slightly different handling since enum is not the same.
        unimplemented!();
    }

    fn rx_ul_tma_unitdata_req(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        tracing::trace!("rx_ul_tma_unitdata_req");

        // Extract sdu
        let SapMsgInner::TmaUnitdataReq(prim) = message.msg else { panic!() };
        let mut sdu = prim.pdu;

        // ── FACCH/Stealing path ──────────────────────────────────────────
        // stealing_permission → STCH on traffic channel for time-critical signaling
        // (D-TX CEASED, D-TX GRANTED) per EN 300 392-2, clause 23.5.
        // CRITICAL: DL STCH uses MAC-RESOURCE (124-bit half-slot), NOT MAC-U-SIGNAL (UL-only).
        if prim.stealing_permission {
            // Determine the target traffic timeslot for FACCH stealing.
            // If chan_alloc specifies a timeslot, use it; otherwise fall back to first active DL circuit.
            let traffic_ts = prim
                .chan_alloc
                .as_ref()
                .and_then(|ca| ca.timeslots.iter().enumerate().find(|&(_, &set)| set).map(|(i, _)| (i + 1) as u8))
                .or_else(|| (2..=4u8).find(|&t| self.channel_scheduler.circuit_is_active(Direction::Dl, t)));

            if let Some(ts) = traffic_ts {
                // Build MAC-RESOURCE PDU for the STCH half-slot (124 type1 bits).
                // Same format as MCCH signaling, just in 124 bits instead of 268.
                const STCH_CAP: usize = 124;

                let is_random_access_response = prim.main_address.ssi_type != SsiType::Gssi;
                let mut mac_pdu = MacResource {
                    fill_bits: false,
                    pos_of_grant: 0,
                    encryption_mode: 0,
                    random_access_flag: is_random_access_response,
                    length_ind: 0,
                    addr: Some(prim.main_address),
                    event_label: None,
                    usage_marker: None,
                    power_control_element: None,
                    slot_granting_element: None,
                    chan_alloc_element: None,
                };
                mac_pdu.update_len_and_fill_ind(sdu.get_len());

                let mut stch_block = BitBuffer::new(STCH_CAP);
                mac_pdu.to_bitbuf(&mut stch_block);

                // Copy LLC PDU (BL-DATA) directly — no conversion needed.
                // Both BL-DATA and BL-UDATA are valid D-LLC-PDU types per the spec.
                sdu.seek(0);
                let sdu_len = sdu.get_len();
                stch_block.copy_bits(&mut sdu, sdu_len);
                // Remaining bits beyond length_ind are ignored by the receiver.

                tracing::info!(
                    "rx_ul_tma_unitdata_req: FACCH stealing on ts {} (MAC-RESOURCE + {} SDU bits → {} STCH bits)",
                    ts,
                    sdu_len,
                    stch_block.get_len()
                );

                self.channel_scheduler.dl_enqueue_stealing(ts, stch_block);
                Self::send_tma_report_ind(queue, message.dltime, prim.req_handle, TmaReport::SuccessDownlinked);
                return;
            } else {
                tracing::warn!("rx_ul_tma_unitdata_req: stealing requested but no active DL circuit, falling back to MCCH");
                // Fall through to normal MCCH path below
            }
        }

        // ── Normal signaling path (MCCH / SCH/F) ────────────────────────
        let (usage_marker, mac_chan_alloc) = if let Some(chan_alloc) = prim.chan_alloc {
            (
                chan_alloc.usage,
                Some(Self::cmce_to_mac_chanalloc(&chan_alloc, self.config.config().cell.main_carrier)),
            )
        } else {
            (None, None)
        };

        // Build MAC-RESOURCE optimistically (as if it would always fit in one slot)
        // random_access_flag: true for SSI-addressed (responses to random access requests),
        // false for GSSI-addressed (unsolicited group signaling like D-SETUP).
        // A radio will reject a random-access-flagged message if it didn't initiate one.
        let is_random_access_response = prim.main_address.ssi_type != SsiType::Gssi;
        let mut pdu = MacResource {
            fill_bits: false, // Updated later
            pos_of_grant: 0,
            encryption_mode: 0,
            random_access_flag: is_random_access_response,
            length_ind: 0, // Updated later
            addr: Some(prim.main_address),
            event_label: None,
            usage_marker,
            power_control_element: None,
            slot_granting_element: None,
            chan_alloc_element: mac_chan_alloc,
        };
        pdu.update_len_and_fill_ind(sdu.get_len());

        // Add to scheduler: Group signaling (GSSI) → TS1 (MCCH) for idle radios.
        // Individual signaling (SSI) → current TS, avoiding active traffic circuits.
        let enqueue_ts = if prim.main_address.ssi_type == SsiType::Gssi {
            1 // Group signaling always on MCCH (TS1)
        } else if self.channel_scheduler.circuit_is_active(Direction::Dl, message.dltime.t) {
            1 // Redirect individual signaling away from traffic TS
        } else {
            message.dltime.t
        };

        // TODO: repeat_count for group call D-SETUP needs to be determined from ETSI spec
        let repeat_count: u8 = 0;
        self.channel_scheduler.dl_enqueue_tma(enqueue_ts, pdu, sdu, repeat_count);

        // TODO FIXME I'm not so sure whether we should send this now, or send it once the message is on its way
        Self::send_tma_report_ind(queue, message.dltime, prim.req_handle, TmaReport::SuccessDownlinked);
    }

    fn rx_tma_prim(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        tracing::trace!("rx_tma_prim");
        match message.msg {
            SapMsgInner::TmaUnitdataReq(_) => {
                self.rx_ul_tma_unitdata_req(queue, message);
            }
            _ => panic!(),
        }
    }

    fn rx_tlmb_prim(&mut self, _queue: &mut MessageQueue, _message: SapMsg) {
        tracing::trace!("rx_tlmb_prim");
        panic!()
    }

    fn rx_tmd_prim(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        tracing::trace!("rx_tmd_prim");
        let dltime = message.dltime;
        let src = message.src;
        match message.msg {
            // DL voice from Brew/upper layer → schedule for DL transmission
            SapMsgInner::TmdCircuitDataReq(prim) => {
                let ts = prim.ts;
                if self.channel_scheduler.circuit_is_active(Direction::Dl, ts) {
                    self.channel_scheduler.dl_schedule_tmd(ts, prim.data);
                } else {
                    tracing::warn!(
                        "rx_tmd_prim: dropping DL voice on inactive circuit ts={} src={:?} dltime={}",
                        ts,
                        src,
                        dltime
                    );
                }
            }
            // UL voice from LMAC → forward to Brew + optional loopback to DL
            SapMsgInner::TmdCircuitDataInd(prim) => {
                let ts = prim.ts;
                let data = prim.data;

                // Forward UL voice to Brew (User plane) if loaded
                if self.config.config().brew.is_some() {
                    if self.channel_scheduler.circuit_is_active(Direction::Ul, ts) {
                        let msg = SapMsg {
                            sap: Sap::TmdSap,
                            src: TetraEntity::Umac,
                            dest: TetraEntity::Brew,
                            dltime,
                            msg: SapMsgInner::TmdCircuitDataInd(tetra_saps::tmd::TmdCircuitDataInd { ts, data: data.clone() }),
                        };
                        queue.push_back(msg);
                    } else {
                        tracing::trace!("rx_tmd_prim: no active UL circuit on ts={}, dropping UL voice to Brew", ts);
                    }
                }

                // Loopback only if there's an active DL circuit on this timeslot
                if self.channel_scheduler.circuit_is_active(Direction::Dl, ts) {
                    tracing::trace!("rx_tmd_prim: loopback UL voice on ts={}", ts);
                    if let Some(packed) = pack_ul_acelp_bits(&data) {
                        self.channel_scheduler.dl_schedule_tmd(ts, packed);
                    } else {
                        tracing::warn!(
                            "rx_tmd_prim: unsupported UL voice length {} on ts={}, skipping loopback",
                            data.len(),
                            ts
                        );
                    }
                } else {
                    tracing::trace!("rx_tmd_prim: no active DL circuit on ts={}, skipping loopback", ts);
                }
            }
            _ => {
                tracing::warn!("rx_tmd_prim: unexpected message type");
            }
        }
    }

    // fn signal_lmac_circuit_setup(&self, queue: &mut MessageQueue, circuit: Circuit) {
    //     let cmd = SapMsg {
    //         sap: Sap::Control,
    //         src: TetraEntity::Umac,
    //         dest: TetraEntity::Lmac,
    //         dltime: self.dltime,
    //         msg: SapMsgInner::CmceCallControl(
    //             CallControl::Open(circuit)
    //         ),
    //     };
    //     queue.push_back(cmd);
    // }

    // fn signal_lmac_circuit_close(&self, queue: &mut MessageQueue, dir: Direction, ts: u8) {
    //     let s = SapMsg {
    //         sap: Sap::Control,
    //         src: TetraEntity::Umac,
    //         dest: TetraEntity::Lmac,
    //         dltime: self.dltime,
    //         msg: SapMsgInner::CmceCallControl(
    //             CallControl::Close(dir, ts)
    //         ),
    //     };
    //     tracing::trace!("signaling LMAC circuit close for ts {}", ts);
    //     queue.push_back(s);
    // }

    fn rx_control_circuit_open(&mut self, _queue: &mut MessageQueue, prim: CallControl) {
        let CallControl::Open(circuit) = prim else { panic!() };
        let ts = circuit.ts;
        let dir = circuit.direction;

        // Direction::Both needs to be split into separate DL and UL operations
        // because the UMAC circuit manager tracks them independently.
        let dirs: Vec<Direction> = match dir {
            Direction::Both => vec![Direction::Dl, Direction::Ul],
            d @ (Direction::Dl | Direction::Ul) => vec![d],
            Direction::None => {
                tracing::warn!("rx_control_circuit_open: Direction::None, ignoring");
                return;
            }
        };

        for d in dirs {
            // See if pre-existing circuit somehow needs to be closed
            if self.channel_scheduler.circuit_is_active(d, ts) {
                tracing::warn!("rx_control_circuit_open: Circuit already exists for {:?} {}, closing first", d, ts);
                self.channel_scheduler.close_circuit(d, ts);
            }

            let c = Circuit {
                direction: d,
                ts: circuit.ts,
                usage: circuit.usage,
                circuit_mode: circuit.circuit_mode,
                speech_service: circuit.speech_service,
                etee_encrypted: circuit.etee_encrypted,
            };
            self.channel_scheduler.create_circuit(d, c);
            tracing::debug!("  rx_control_circuit_open: Setup {:?} circuit for ts {}", d, ts);
        }
    }

    fn rx_control_circuit_close(&mut self, _queue: &mut MessageQueue, prim: CallControl) {
        let CallControl::Close(dir, ts) = prim else { panic!() };

        // Direction::Both needs to be split into separate DL and UL close operations
        let dirs: Vec<Direction> = match dir {
            Direction::Both => vec![Direction::Dl, Direction::Ul],
            d @ (Direction::Dl | Direction::Ul) => vec![d],
            Direction::None => {
                tracing::warn!("rx_control_circuit_close: Direction::None, ignoring");
                return;
            }
        };

        for d in dirs {
            match self.channel_scheduler.close_circuit(d, ts) {
                Some(_) => {
                    tracing::info!("  rx_control_circuit_close: Closed {:?} circuit for ts {}", d, ts);
                }
                None => {
                    tracing::warn!("  rx_control_circuit_close: No {:?} circuit to close for ts {}", d, ts);
                }
            }
        }
    }

    fn rx_control(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        tracing::trace!("rx_control");
        let SapMsgInner::CmceCallControl(prim) = message.msg else {
            panic!()
        };

        match prim {
            CallControl::Open(_) => {
                self.rx_control_circuit_open(queue, prim);
            }
            CallControl::Close(_, _) => {
                self.rx_control_circuit_close(queue, prim);
            }
            // Floor-control signals drive traffic↔signalling transitions during hangtime.
            CallControl::FloorReleased { ts, .. } => {
                self.channel_scheduler.set_hangtime(ts, true);
            }
            CallControl::FloorGranted { ts, .. } => {
                self.channel_scheduler.set_hangtime(ts, false);
            }
            CallControl::CallEnded { ts, .. } => {
                self.channel_scheduler.set_hangtime(ts, false);
            }

            // NetworkCall* are for CMCE ↔ Brew, not UMAC (for now)
            CallControl::NetworkCallStart { .. } | CallControl::NetworkCallReady { .. } | CallControl::NetworkCallEnd { .. } => {
                tracing::trace!("rx_control: ignoring CMCE-Brew notification (not for UMAC)");
            }
        }
    }
}

impl TetraEntityTrait for UmacBs {
    fn entity(&self) -> TetraEntity {
        TetraEntity::Umac
    }

    fn set_config(&mut self, config: SharedConfig) {
        self.config = config;
    }

    fn rx_prim(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        // tracing::debug!("rx_prim: {:?}", message);
        // tracing::debug!(ts=%message.dltime, "rx_prim: {:?}", message);

        match message.sap {
            Sap::TmvSap => {
                self.rx_tmv_prim(queue, message);
            }
            Sap::TmaSap => {
                self.rx_tma_prim(queue, message);
            }
            Sap::TmdSap => {
                self.rx_tmd_prim(queue, message);
            }
            Sap::TlmbSap => {
                self.rx_tlmb_prim(queue, message);
            }
            Sap::TlmcSap => {
                unimplemented!();
            }
            Sap::Control => {
                self.rx_control(queue, message);
            }
            _ => {
                panic!()
            }
        }
    }

    fn tick_start(&mut self, queue: &mut MessageQueue, ts: TdmaTime) {
        self.dltime = ts;
        self.refresh_system_wide_services();

        if self.channel_scheduler.cur_dltime != ts && self.channel_scheduler.cur_dltime == (TdmaTime { t: 0, f: 0, m: 0, h: 0 }) {
            // Upon start of the system, we need to set the dl time for the channel scheduler
            self.channel_scheduler.set_dl_time(ts);
        } else {
            // When running, we adopt the new time and check for desync
            self.channel_scheduler.tick_start(ts);
        }

        // Collect/construct traffic that should be sent down to the LMAC
        // This is basically the _previous_ timeslot
        let elem = self.channel_scheduler.finalize_ts_for_tick();
        let s = SapMsg {
            sap: Sap::TmvSap,
            src: self.self_component,
            dest: TetraEntity::Lmac,
            dltime: ts.add_timeslots(-1),
            msg: SapMsgInner::TmvUnitdataReq(elem),
        };
        tracing::trace!("UmacBs tick: Pushing finalized timeslot to LMAC: {:?}", s);
        queue.push_back(s);
    }
}

/// Pack UL ACELP voice bits (274 bits, one-bit-per-byte) into packed byte array for DL transmission.
/// Handles both already-packed (35 bytes) and unpacked (274 bytes) formats.
fn pack_ul_acelp_bits(bits: &[u8]) -> Option<Vec<u8>> {
    const PACKED_TCH_S_BYTES: usize = (TCH_S_CAP + 7) / 8;

    // Already packed format — pass through
    if bits.len() == PACKED_TCH_S_BYTES {
        return Some(bits.to_vec());
    }
    // Insufficient data
    if bits.len() < TCH_S_CAP {
        return None;
    }

    // Pack 274 one-bit-per-byte into 35 bytes (last byte has 2 padding bits)
    let mut out = Vec::with_capacity(PACKED_TCH_S_BYTES);
    for chunk_idx in 0..PACKED_TCH_S_BYTES {
        let mut byte = 0u8;
        for bit in 0..8 {
            let bit_idx = chunk_idx * 8 + bit;
            if bit_idx < TCH_S_CAP {
                byte |= (bits[bit_idx] & 1) << (7 - bit);
            }
        }
        out.push(byte);
    }
    Some(out)
}
