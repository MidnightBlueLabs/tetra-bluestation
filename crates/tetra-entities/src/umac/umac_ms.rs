use std::panic;

use tetra_config::SharedConfig;
use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{BitBuffer, PhyBlockNum, Sap, TdmaTime, Todo, unimplemented_log};
use tetra_saps::tlmb::TlmbSysinfoInd;
use tetra_saps::tma::TmaUnitdataInd;
use tetra_saps::tmv::TmvConfigureReq;
use tetra_saps::tmv::enums::logical_chans::LogicalChannel;
use tetra_saps::{SapMsg, SapMsgInner};

use tetra_pdus::umac::enums::broadcast_type::BroadcastType;
use tetra_pdus::umac::enums::mac_pdu_type::MacPduType;
use tetra_pdus::umac::pdus::access_assign::AccessAssign;
use tetra_pdus::umac::pdus::access_assign_fr18::AccessAssignFr18;
use tetra_pdus::umac::pdus::mac_end_dl::MacEndDl;
use tetra_pdus::umac::pdus::mac_frag_dl::MacFragDl;
use tetra_pdus::umac::pdus::mac_resource::MacResource;
use tetra_pdus::umac::pdus::mac_sync::MacSync;
use tetra_pdus::umac::pdus::mac_sysinfo::MacSysinfo;

use crate::umac::subcomp::fillbits;
use crate::umac::subcomp::ms_defrag::MsDefrag;
use crate::{MessagePrio, MessageQueue, TetraEntityTrait};

pub struct UmacMs {
    // config: Option<SharedConfig>,
    self_component: TetraEntity,
    config: SharedConfig,
    defrag: MsDefrag,

    /// Provided by MLE over TlmbSap, to compute scrambling code, which is passed to lmac
    mcc: Option<u16>,
    /// Provided by MLE over TlmbSap, to compute scrambling code, which is passed to lmac
    mnc: Option<u16>,
    /// Provided by MLE over TlmbSap, to compute scrambling code, which is passed to lmac
    cc: Option<u8>,
    /// Derived from mcc/mnc, and passed to lmac
    scrambling_code: Option<u32>,
}

impl UmacMs {
    pub fn new(config: SharedConfig) -> Self {
        Self {
            self_component: TetraEntity::Umac,
            config,
            defrag: MsDefrag::new(),

            mcc: None,
            mnc: None,
            cc: None,
            scrambling_code: None,
        }
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
            LogicalChannel::Aach => {
                self.rx_tmv_aach(queue, message);
            }

            LogicalChannel::Bsch => {
                self.rx_tmv_bsch(queue, message);
            }

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

            LogicalChannel::Bnch | LogicalChannel::Stch | LogicalChannel::SchHd => {
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
            let Ok(pdu_type) = MacPduType::try_from(bits >> 1) else {
                tracing::warn!("invalid pdu type: {}", bits >> 1);
                return;
            };
            let orig_start = prim.pdu.get_raw_start();
            let lchan = prim.logical_channel;

            match pdu_type {
                MacPduType::MacResourceMacData => {
                    self.rx_mac_resource(queue, &mut message);
                }
                MacPduType::MacFragMacEnd => {
                    // Also need third bit; designates mac-frag versus mac-end
                    if bits & 1 == 0 {
                        self.rx_mac_frag(queue, &mut message);
                    } else {
                        self.rx_mac_end(queue, &mut message);
                    }
                }
                MacPduType::Broadcast => {
                    self.rx_broadcast(queue, &mut message);
                }
                MacPduType::SuppMacUSignal => {
                    if lchan == LogicalChannel::Stch {
                        // U-SIGNAL since we're on the stealing channel
                        self.rx_usignal(queue, &mut message);
                    } else {
                        self.rx_supp(queue, &mut message);
                    }
                }
            }

            // Check if end of message reached by re-borrowing inner
            // If start was not updated, we also consider it end of message
            // If 16 or more bits remain (len of null pdu), we continue parsing
            if let SapMsgInner::TmvUnitdataInd(prim) = &message.msg {
                if prim.pdu.get_raw_start() != orig_start && prim.pdu.get_len() >= 16 {
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

    // message pos: start of broadcast frame
    // Will NOT advance pos but pass to underlying function
    fn rx_broadcast(&self, queue: &mut MessageQueue, message: &mut SapMsg) {
        tracing::trace!("rx_broadcast");

        let SapMsgInner::TmvUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };
        assert!(prim.pdu.peek_bits(2).unwrap() == MacPduType::Broadcast.into_raw()); // MAC PDU type

        let bits = prim.pdu.peek_bits_posoffset(2, 2).unwrap();
        let bcast_type = BroadcastType::try_from(bits).expect("invalid broadcast type");

        match bcast_type {
            BroadcastType::Sysinfo => {
                self.rx_broadcast_sysinfo(queue, message);
            }
            _ => {
                panic!();
            }
        }
    }

    // Parses the sysinfo pdu
    fn rx_broadcast_sysinfo(&self, queue: &mut MessageQueue, message: &mut SapMsg) {
        tracing::trace!("rx_broadcast_sysinfo");
        let SapMsgInner::TmvUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };

        // Parse SYSINFO header and optional data
        let pdu = match MacSysinfo::from_bitbuf(&mut prim.pdu) {
            Ok(pdu) => {
                tracing::debug!("<- {:?}", pdu);
                pdu
            }
            Err(e) => {
                tracing::warn!("Failed parsing MacSysinfo: {:?} {}", e, prim.pdu.dump_bin());
                return;
            }
        };

        // TODO FIXME adopt sysinfo info into global state

        if pdu.hyperframe_number.is_some() && pdu.hyperframe_number.unwrap() != message.dltime.h {
            // Send message to Phy about new hyperframe number
            let t = TdmaTime {
                t: message.dltime.t,
                f: message.dltime.f,
                m: message.dltime.m,
                h: pdu.hyperframe_number.unwrap(),
            };
            let m = SapMsg {
                sap: Sap::TmvSap,
                src: self.self_component,
                dest: TetraEntity::Lmac,
                dltime: message.dltime,
                msg: SapMsgInner::TmvConfigureReq(TmvConfigureReq {
                    time: Some(t),
                    ..Default::default()
                }),
            };
            tracing::info!("rx_broadcast_sysinfo: Updated TdmaTime: {:?} -> {:?}", message.dltime, t);
            queue.push_back(m);
        }

        let tlsdu = BitBuffer::from_bitbuffer_pos(&prim.pdu);
        let m = SapMsg {
            sap: Sap::TlmbSap,
            src: TetraEntity::Umac,
            dest: TetraEntity::Mle,
            dltime: message.dltime,
            msg: SapMsgInner::TlmbSysinfoInd(TlmbSysinfoInd {
                endpoint_id: 0,
                tl_sdu: tlsdu,
                mac_broadcast_info: None,
            }),
        };

        queue.push_back(m);
    }

    fn rx_mac_resource(&mut self, queue: &mut MessageQueue, message: &mut SapMsg) {
        tracing::trace!("rx_mac_resource");
        let SapMsgInner::TmvUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };
        assert!(prim.pdu.get_pos() == 0); // We should be at the start of the MAC PDU

        // Parse header and optional ChanAlloc
        let pdu = match MacResource::from_bitbuf(&mut prim.pdu) {
            Ok(pdu) => {
                tracing::debug!("<- {:?}", pdu);
                pdu
            }
            Err(e) => {
                tracing::warn!("Failed parsing MacResource: {:?} {}", e, prim.pdu.dump_bin());
                return;
            }
        };

        if pdu.encryption_mode > 0 {
            unimplemented_log!("rx_mac_resource: Encryption mode > 0, not implemented");
        }

        // Compute len
        let mut pdu_len_bits = {
            match pdu.length_ind {
                0b000001..0b111010 => {
                    // tracing::trace!("rx_mac_resource: length_ind {}", pdu.length_ind);
                    pdu.length_ind as usize * 8
                }
                0b111110 => {
                    // Second half slot stolen in STCH
                    unimplemented_log!("rx_mac_resource: SECOND HALF SLOT STOLEN IN STCH but signal not implemented");
                    prim.pdu.get_len()
                }
                0b111111 => {
                    // Start of fragmentation
                    // tracing::trace!("rx_mac_resource: frag start length_ind {}", pdu.length_ind);
                    prim.pdu.get_len()
                }
                _ => panic!("rx_mac_resource: Invalid length_ind {}", pdu.length_ind),
            }
        };

        if pdu_len_bits > prim.pdu.get_len() {
            // TODO FIXME: I sometimes encounter len = 0b100010 = 32
            // This does not fit, since it translates to 272 bits while it comes in a 268 bit slot
            // We'll correct for that by simply cropping to the end... But this is strange
            tracing::warn!(
                "rx_mac_resource: Strange length_ind {} in MAC resource, truncating from {} to {}",
                pdu.length_ind,
                pdu_len_bits,
                prim.pdu.get_len()
            );
            pdu_len_bits = prim.pdu.get_len();
        }

        // Strip fill bits. Maintain original end to allow for later parsing of a second mac block
        tracing::trace!("rx_mac_resource: {}", prim.pdu.dump_bin_full(true));
        let num_fill_bits = {
            if pdu.fill_bits {
                fillbits::removal::get_num_fill_bits(&prim.pdu, pdu_len_bits, pdu.is_null_pdu())
            } else {
                0
            }
        };
        pdu_len_bits -= num_fill_bits;
        let orig_end = prim.pdu.get_raw_end();
        prim.pdu.set_raw_end(prim.pdu.get_raw_start() + pdu_len_bits);
        tracing::trace!(
            "rx_mac_resource: pdu: {} sdu: {} fb: {}: {}",
            pdu_len_bits,
            prim.pdu.get_len_remaining(),
            num_fill_bits,
            prim.pdu.dump_bin_full(true)
        );

        if pdu.addr.is_none() {
            // TODO not sure if there is scenarios in which we want to pass a null pdu to the LLC
            // tracing::warn!("rx_mac_resource: Null PDU not passed to LLC");
            return;
        }

        // Decrypt if needed
        if pdu.encryption_mode > 0 {
            unimplemented_log!("rx_mac_resource: Encryption mode > 0");
            return;
            // TODO:
            // Check if key available
            // generate keystream
            // apply keystream to data
            // re-decode chanalloc
            // continue
        }

        tracing::debug!("rx_mac_resource: {}", prim.pdu.dump_bin_full(true));
        if pdu.length_ind == 0b111111 {
            // Fragmentation start, add to defragmenter
            self.defrag.insert_first(&mut prim.pdu, message.dltime, pdu.addr.unwrap(), None);
        } else if pdu.length_ind == 0b111110 {
            tracing::warn!("rx_mac_resource: SECOND HALF SLOT STOLEN IN STCH but not implemented");
        } else {
            // Pass directly to LLC
            let sdu = {
                if pdu.length_ind == 0 {
                    None // Null PDU
                } else if prim.pdu.get_len_remaining() == 0 {
                    None // No more data in this block
                } else {
                    // TODO FIXME should not copy here but take ownership
                    // Copy inner part, without MAC header or fill bits
                    Some(BitBuffer::from_bitbuffer_pos(&prim.pdu))
                }
            };
            // tracing::debug!("rx_mac_resource: sdu: {:?}", sdu.as_ref().unwrap().dump_bin_full(true));

            if sdu.is_some() {
                // We have an SDU for the LLC, deliver it.
                let m = SapMsg {
                    sap: Sap::TmaSap,
                    src: TetraEntity::Umac,
                    dest: TetraEntity::Llc,
                    dltime: message.dltime,

                    msg: SapMsgInner::TmaUnitdataInd(TmaUnitdataInd {
                        pdu: sdu,
                        main_address: pdu.addr.unwrap(),
                        scrambling_code: prim.scrambling_code,
                        endpoint_id: 0,        // TODO FIXME
                        new_endpoint_id: None, // TODO FIXME
                        css_endpoint_id: None, // TODO FIXME
                        air_interface_encryption: pdu.encryption_mode as Todo,
                        chan_change_response_req: false,
                        chan_change_handle: None,
                        chan_info: None,
                    }),
                };
                queue.push_back(m);
            } else {
                // Either this is a null pdu or we are at the end of the block
                // For now, we don't deliver this. However, important data may need to be signalled upwards
                tracing::info!("rx_mac_resource: empty PDU not passed to LLC");
            }
        }

        // Since this is not a null pdu, more MAC PDUs may follow
        // This allows parent function to continue parsing
        prim.pdu.set_raw_end(orig_end);
        prim.pdu.set_raw_pos(prim.pdu.get_raw_start() + pdu_len_bits + num_fill_bits);
        prim.pdu.set_raw_start(prim.pdu.get_raw_pos());
    }

    fn rx_mac_frag(&mut self, _queue: &mut MessageQueue, message: &mut SapMsg) {
        tracing::trace!("rx_mac_frag");
        let SapMsgInner::TmvUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };
        assert!(prim.pdu.get_pos() == 0); // We should be at the start of the MAC PDU

        // Parse header and optional ChanAlloc
        let pdu = match MacFragDl::from_bitbuf(&mut prim.pdu) {
            Ok(pdu) => {
                tracing::debug!("<- {:?}", pdu);
                pdu
            }
            Err(e) => {
                tracing::warn!("Failed parsing MacFragDl: {:?} {}", e, prim.pdu.dump_bin());
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
        tracing::debug!("rx_mac_frag: pdu_len_bits: {} fill_bits: {}", pdu_len_bits, num_fill_bits);

        // Decrypt if needed
        if let Some(_aie_info) = self.defrag.buffers[(message.dltime.t - 1) as usize].aie_info {
            // TODO FIXME implement
            unimplemented_log!("rx_mac_frag: Encryption not supported");
            return;
        }

        // Insert into defragmenter
        self.defrag.insert_next(&mut prim.pdu, message.dltime);
    }

    fn rx_mac_end(&mut self, queue: &mut MessageQueue, message: &mut SapMsg) {
        tracing::trace!("rx_mac_end");
        let SapMsgInner::TmvUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };
        assert!(prim.pdu.get_pos() == 0); // We should be at the start of the MAC PDU

        // Parse header and optional ChanAlloc
        let pdu = match MacEndDl::from_bitbuf(&mut prim.pdu) {
            Ok(pdu) => {
                tracing::debug!("<- {:?}", pdu);
                pdu
            }
            Err(e) => {
                tracing::warn!("Failed parsing MacEndDl: {:?} {}", e, prim.pdu.dump_bin());
                return;
            }
        };

        // Compute len
        assert!(pdu.length_ind != 0); // Reserved
        let mut pdu_len_bits = pdu.length_ind as usize * 8;

        // Strip fill bits. Maintain original end to allow for later parsing of a second mac block
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
        tracing::debug!("rx_mac_end: pdu_len_bits: {} fill_bits: {}", pdu_len_bits, num_fill_bits);

        // Decrypt if needed
        if let Some(_aie_info) = self.defrag.buffers[(message.dltime.t - 1) as usize].aie_info {
            // TODO FIXME implement
            unimplemented!("rx_mac_end: Encryption not supported");
            // TODO FIXME Also re-parse chanalloc
        }

        // Insert into defragmenter
        self.defrag.insert_last(&mut prim.pdu, message.dltime);

        // Fetch finalized block
        let defragbuf = self.defrag.take_defragged_buf(message.dltime);
        let Some(defragbuf) = defragbuf else {
            tracing::warn!("rx_mac_end: could not obtain defragged buf");
            return;
        };

        // Pass block directly to LLC
        tracing::debug!("rx_mac_end: sdu: {:?}", defragbuf.buffer.dump_bin());

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

    fn rx_usignal(&self, _queue: &mut MessageQueue, message: &mut SapMsg) {
        tracing::trace!("rx_usignal");
        let SapMsgInner::TmvUnitdataInd(_prim) = &mut message.msg else {
            panic!()
        };
        unimplemented!("rx_usignal");
    }

    fn rx_supp(&self, _queue: &mut MessageQueue, message: &mut SapMsg) {
        tracing::trace!("rx_supp");

        let SapMsgInner::TmvUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };
        // Check we're indeed on the right channel (Clause 21.4.1 Table 21.48)
        assert!(prim.logical_channel != LogicalChannel::Stch && prim.logical_channel != LogicalChannel::SchHd);
        unimplemented!("rx_supp");
    }

    pub fn rx_tmv_aach(&self, queue: &mut MessageQueue, mut message: SapMsg) {
        tracing::trace!("rx_tmv_aach");

        // TODO FIXME, more extensively store and process AACH state in both LMAC and UMAC
        // Then we send a msg down only if a change is needed, like we do for the scrambling code

        let SapMsgInner::TmvUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };

        let is_traffic = if message.dltime.f != 18 {
            let pdu = match AccessAssign::from_bitbuf(&mut prim.pdu) {
                Ok(pdu) => {
                    tracing::debug!("<- {:?}", pdu);
                    pdu
                }
                Err(e) => {
                    tracing::warn!("Failed parsing AccessAssign: {:?} {}", e, prim.pdu.dump_bin());
                    return;
                }
            };

            pdu.dl_usage.is_traffic()
        } else {
            let _pdu = match AccessAssignFr18::from_bitbuf(&mut prim.pdu) {
                Ok(pdu) => {
                    tracing::debug!("<- {:?}", pdu);
                    pdu
                }
                Err(e) => {
                    tracing::warn!("Failed parsing AccessAssignFr18: {:?} {}", e, prim.pdu.dump_bin());
                    return;
                }
            };

            false
        };

        let m = SapMsg {
            sap: Sap::TmvSap,
            src: TetraEntity::Umac,
            dest: TetraEntity::Lmac,
            dltime: message.dltime,
            msg: SapMsgInner::TmvConfigureReq(TmvConfigureReq {
                is_traffic: Some(is_traffic),
                // TODO FIXME we should set this based on the call
                // For now, we'll just assume TCH and ILD 1 in lmac
                tch_type_and_interleaving_depth: None,

                // Could update scrambling code here, but LMAC should already have it
                scrambling_code: None,
                ..Default::default()
            }),
        };
        // This message needs to be processed NOW since it affects the other blocks in this timeslot
        queue.push_prio(m, MessagePrio::Immediate);
    }

    pub fn rx_tmv_bsch(&mut self, _queue: &mut MessageQueue, mut message: SapMsg) {
        tracing::trace!("rx_tmv_bsch");
        let SapMsgInner::TmvUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };

        // Unpack and validate with expected state
        let _pdu = match MacSync::from_bitbuf(&mut prim.pdu) {
            Ok(pdu) => {
                tracing::debug!("<- {:?}", pdu);
                pdu
            }
            Err(e) => {
                tracing::warn!("Failed parsing MacSync: {:?} {}", e, prim.pdu.dump_bin());
                return;
            }
        };

        unimplemented_log!("can't update global state");

        // let netinfo_changed = {
        //     let config_r = self.config.read();
        //         mac_sync.system_code != config_r.la_info.system_code
        //             || mac_sync.sharing_mode != config_r.la_info.sharing_mode
        //             || mac_sync.ts_reserved_frames != config_r.la_info.ts_reserved_frames
        //             || mac_sync.u_plane_dtx != config_r.la_info.u_plane_dtx
        //             || mac_sync.frame_18_ext != config_r.la_info.frame_18_ext
        // };
        // // tracing::trace!("rx_tmv_bsch: netinfo_changed: {}, cc_changed: {}, tdma_time_changed: {}", netinfo_changed, cc_changed, tdma_time_changed);

        // // Update global state if needed
        // if netinfo_changed  {
        //     let mut config_w = self.config.write();
        //     config_w.la_info.system_code = mac_sync.system_code;
        //     // config_w.netinfo.colour_code = mac_sync.colour_code;
        //     config_w.la_info.sharing_mode = mac_sync.sharing_mode;
        //     config_w.la_info.ts_reserved_frames = mac_sync.ts_reserved_frames;
        //     config_w.la_info.u_plane_dtx = mac_sync.u_plane_dtx;
        //     config_w.la_info.frame_18_ext = mac_sync.frame_18_ext;
        //     tracing::info!("rx_tmv_bsch: Updated TetraGlobalState: {:?}", mac_sync);
        // }

        // if mac_sync.time.t != message.t_submit.t || mac_sync.time.f != message.t_submit.f || mac_sync.time.m != message.t_submit.m {
        //     // TODO warn/bail when really not in line with expected time
        //     let t = TdmaTime{
        //         t: mac_sync.time.t,
        //         f: mac_sync.time.f,
        //         m: mac_sync.time.m,
        //         h: message.t_submit.h,
        //     };
        //     let m = SapMsg {
        //         sap: Sap::TmvSap,
        //         src: self.self_component,
        //         dest: TetraComponent::Lmac,
        //         t_submit: message.t_submit,
        //         msg: SapMsgInner::TmvConfigureReq(
        //             TmvConfigureReq{
        //                 time: Some(t),
        //                 .. Default::default()
        //             }
        //         )
        //     };
        //     tracing::info!("rx_tmv_bsch: Updated TdmaTime: {:?} -> {:?}", message.t_submit, t);
        //     queue.push_back(m);
        // }

        // if Some(mac_sync.colour_code) != self.cc {
        //     // Update scrambling code
        //     tracing::info!("rx_tmv_bsch: Updated colour code: {:?} -> {:?}", self.cc, mac_sync.colour_code);
        //     self.cc = Some(mac_sync.colour_code);
        //     self.update_scrambing_and_submit_to_lmac(queue, &message);

        // } else {
        //     tracing::trace!("rx_tmv_bsch: Colour code unchanged: {:?}", self.cc);
        // }

        // // Take ownership of prim and sdu
        // let prim = if let SapMsgInner::TmvUnitdataInd(inner) = message.msg {
        //     inner
        // } else {
        //     panic!();
        // };
        // let tlsdu = prim.pdu;

        // let m = SapMsg {
        //     sap: Sap::TlmbSap,
        //     src: TetraComponent::Umac,
        //     dest: TetraComponent::Mle,
        //     t_submit: message.t_submit,

        //     msg: SapMsgInner::TlmbSyncInd(
        //         TlmbSyncInd {
        //             endpoint_id: 0,
        //             tl_sdu: tlsdu
        //         }
        //     )
        // };
        // tracing::info!("rx_tmv_bsch: {:?}", m.msg);
        // queue.push_back(m);
    }

    fn rx_tma_prim(&mut self, _queue: &mut MessageQueue, _message: SapMsg) {
        tracing::trace!("rx_tma_prim");
        unimplemented!();
    }

    fn rx_tlmb_prim(&mut self, _queue: &mut MessageQueue, _message: SapMsg) {
        tracing::trace!("rx_tlmb_prim");
        unimplemented!();
    }

    fn update_scrambing_and_submit_to_lmac(&mut self, queue: &mut MessageQueue, message: &SapMsg) {
        if let (Some(mcc), Some(mnc), Some(cc)) = (self.mcc, self.mnc, self.cc) {
            self.scrambling_code = Some((((cc as u32) | ((mnc as u32) << 6) | ((mcc as u32) << 20)) << 2) | 3);

            tracing::trace!(
                "compute_scrambling_and_submit_to_lmac cc {} mcc {} mnc {} scrambling_code: {}",
                cc,
                mcc,
                mnc,
                self.scrambling_code.unwrap()
            );

            let m = SapMsg {
                sap: Sap::TmvSap,
                src: self.self_component,
                dest: TetraEntity::Lmac,
                dltime: message.dltime,
                msg: SapMsgInner::TmvConfigureReq(TmvConfigureReq {
                    scrambling_code: self.scrambling_code,
                    ..Default::default()
                }),
            };
            queue.push_back(m);
        }
    }

    fn rx_tlmc_configure_req(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        tracing::trace!("rx_tlmc_configure_req");
        let SapMsgInner::TlmcConfigureReq(prim) = &message.msg else {
            panic!()
        };

        if let Some(valid_addresses) = &prim.valid_addresses {
            tracing::debug!("rx_tlmc_configure_req: valid_addresses: {:?}", valid_addresses);

            self.mcc = Some(valid_addresses.mcc);
            self.mnc = Some(valid_addresses.mnc);

            // Attempt to update scrambling code (if cc is also known)
            self.update_scrambing_and_submit_to_lmac(queue, &message);
        } else {
            tracing::warn!("rx_tlmc_configure_req: No valid addresses provided");
        }
    }

    fn rx_tlmc_prim(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        tracing::trace!("rx_tlmc_prim");
        match message.msg {
            SapMsgInner::TlmcConfigureReq(_) => {
                self.rx_tlmc_configure_req(queue, message);
            }
            _ => {
                panic!();
            }
        }
    }
}

impl TetraEntityTrait for UmacMs {
    fn entity(&self) -> TetraEntity {
        TetraEntity::Umac
    }

    fn rx_prim(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        tracing::debug!("rx_prim: {:?}", message);
        // tracing::debug!(ts=%message.dltime, "rx_prim: {:?}", message);

        match message.sap {
            Sap::TmvSap => {
                self.rx_tmv_prim(queue, message);
            }

            Sap::TmaSap => {
                self.rx_tma_prim(queue, message);
            }

            Sap::TlmbSap => {
                self.rx_tlmb_prim(queue, message);
            }

            Sap::TlmcSap => {
                self.rx_tlmc_prim(queue, message);
            }

            _ => {
                panic!()
            }
        }
    }
}
