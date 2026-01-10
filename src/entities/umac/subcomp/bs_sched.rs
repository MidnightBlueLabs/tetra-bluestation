use crate::{
    common::{
        address::TetraAddress, bitbuffer::BitBuffer, tdma_time::TdmaTime, tetra_common::Todo,
    },
    entities::{
        lmac::components::scramble::SCRAMB_INIT,
        mle::pdus::{d_mle_sync::DMleSync, d_mle_sysinfo::DMleSysinfo},
        umac::{
            enums::{
                access_assign_dl_usage::AccessAssignDlUsage,
                access_assign_ul_usage::AccessAssignUlUsage,
                basic_slotgrant_cap_alloc::BasicSlotgrantCapAlloc,
                basic_slotgrant_granting_delay::BasicSlotgrantGrantingDelay,
                reservation_requirement::ReservationRequirement,
            },
            fields::basic_slotgrant::BasicSlotgrant,
            pdus::{
                access_assign::{AccessAssign, AccessField},
                access_assign_fr18::AccessAssignFr18,
                mac_resource::MacResource,
                mac_sync::MacSync,
                mac_sysinfo::MacSysinfo,
            },
            subcomp::fillbits::write_fill_bits,
        },
    },
    saps::tmv::{
        enums::logical_chans::LogicalChannel,
        {TmvUnitdataReq, TmvUnitdataReqSlot},
    },
    unimplemented_log,
};

/// We submit this many TX timeslots ahead of the current time
pub const MACSCHED_TX_AHEAD: usize = 1;

// We schedule up to this many frames ahead
pub const MACSCHED_NUM_FRAMES: usize = 18;

const NULL_PDU_LEN_BITS: usize = 16;
const SCH_HD_CAP: usize = 124;
const SCH_F_CAP: usize = 268;

#[derive(Debug)]
pub struct PrecomputedUmacPdus {
    pub mac_sysinfo1: MacSysinfo,
    pub mac_sysinfo2: MacSysinfo,
    pub mle_sysinfo: DMleSysinfo,
    pub mac_sync: MacSync,
    pub mle_sync: DMleSync,
}

#[derive(Debug)]
pub struct TimeslotSchedule {
    pub ul1: Option<u32>,
    pub ul2: Option<u32>,
    pub dl: Option<TmvUnitdataReq>,
}

#[derive(Debug)]
pub struct BsChannelScheduler {
    pub cur_ts: TdmaTime,
    scrambling_code: Option<u32>,
    precomps: Option<PrecomputedUmacPdus>,
    pub dltx_queues: [Vec<DlSchedElem>; 4],
    sched: [[TimeslotSchedule; MACSCHED_NUM_FRAMES]; 4],
}

#[derive(Debug)]
pub enum DlSchedElem {
    /// A SYSINFO or neighboring cells info block. The integer determines which of the precomputed blocks to use (SYSINFO1, SYSINFO2, NEIGHBORING_CELLS
    Broadcast(Todo),

    /// A received MAC-ACCESS PDU still has to be acknowledged
    RandomAccessAck(TetraAddress),

    /// A slotgrant response, which has to be transmitted with high priority or the delay numbers will be off
    /// ssi and BasicSlotgrant are provided.
    Grant(TetraAddress, BasicSlotgrant),

    /// A MAC-RESOURCE PDU. May be split into fragments upon processing, in which case a FragBuf will be inserted after processing the resource.
    Resource(MacResource, BitBuffer),

    /// A FragBuf containing remaining non-transmitted information after a MAC-RESOURCE start has been transmitted
    FragBuf(Todo),
}

const EMPTY_SCHED_ELEM: TimeslotSchedule = TimeslotSchedule {
    ul1: None,
    ul2: None,
    dl: None,
};
const EMPTY_SCHED_CHANNEL: [TimeslotSchedule; MACSCHED_NUM_FRAMES] =
    [EMPTY_SCHED_ELEM; MACSCHED_NUM_FRAMES];
const EMPTY_SCHED: [[TimeslotSchedule; MACSCHED_NUM_FRAMES]; 4] = [EMPTY_SCHED_CHANNEL; 4];

impl BsChannelScheduler {
    pub fn new() -> Self {
        BsChannelScheduler {
            cur_ts: TdmaTime {
                t: 0,
                f: 0,
                m: 0,
                h: 0,
            }, // Intentionally invalid, updated in tick function
            scrambling_code: None,
            precomps: None,
            dltx_queues: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            sched: EMPTY_SCHED,
        }
    }

    pub fn set_scrambling_code(&mut self, scrambling_code: u32) {
        self.scrambling_code = Some(scrambling_code);
    }

    pub fn set_precomputed_msgs(&mut self, precomps: PrecomputedUmacPdus) {
        self.precomps = Some(precomps);
    }

    /// Fully wipe the schedule
    pub fn purge_schedule(&mut self) {
        self.dltx_queues = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        self.sched = EMPTY_SCHED;
    }

    /// Sets the current downlink time to the given TdmaTime
    /// Wipes the schedule, as it can no longer be guaranteed to be valid
    pub fn set_dl_time(&mut self, new_ts: TdmaTime) {
        self.cur_ts = new_ts;
        self.purge_schedule();
    }

    pub fn ts_to_sched_index(&self, ts: &TdmaTime) -> usize {
        let to_index = (ts.f as usize - 1) + ((ts.m as usize - 1) * 18) + (ts.h as usize * 18 * 60);
        to_index % MACSCHED_NUM_FRAMES
    }

    ///////// UPLINK GRANT PROCESSING /////////

    /// Finds a grant opportunity for uplink transmission
    /// If num_slots is 1, is_halfslot may specifiy whether only a half slot is needed
    /// Returns (opportunities_to_skip, Vec<timestamps_of_granted_slots>)
    /// Returns None if no suitable opportunity is found in the schedule
    pub fn ul_find_grant_opportunity(
        &self,
        ts: u8,
        num_slots: usize,
        is_halfslot: bool,
    ) -> Option<(usize, Vec<TdmaTime>)> {
        let mut grant_timeslots = Vec::with_capacity(num_slots);
        let mut opportunities_skipped = 0;

        assert!(
            !is_halfslot || num_slots == 1,
            "is_halfslot set for num_slots > 1"
        );

        for dist in 1..MACSCHED_NUM_FRAMES {
            // let candidate_t = self.cur_ts.add_timeslots(dist as i32 * 4);
            // Base off of internal perception of time, convert to UL time
            // Below may crash someday, but I'd want to investigate that situation
            let candidate_t = self.cur_ts.add_timeslots(dist as i32 * 4 - 2);
            assert!(
                candidate_t.t == ts,
                "ul_find_grant_opportunity: candidate_t.ts {} does not match requested ts {}. Please report this to developer. ",
                candidate_t.t,
                ts
            );

            tracing::debug!(
                "ul_find_grant_opportunity: considering candidate ul_ts {}, have {:?}",
                candidate_t,
                grant_timeslots
            );

            if self.cur_ts.is_mandatory_clch() {
                // Not an opportunity; skip
                continue;
            }

            let index = self.ts_to_sched_index(&candidate_t);
            let elem = &self.sched[ts as usize - 1][index];
            // tracing::debug!("ul_find_grant_opportunity: sched[{}] ts {}: {:?}", index, candidate_t, elem);
            if (elem.ul1.is_none() && elem.ul2.is_none())
                || (is_halfslot && (elem.ul1.is_none() || elem.ul2.is_none()))
            {
                // Free UL slot, add this timeslot to result vec
                grant_timeslots.push(candidate_t);
                // continue;
            } else {
                // Something is here, clear our grant timeslots
                opportunities_skipped += grant_timeslots.len() + 1;
                grant_timeslots.clear();
            }

            // Check if done
            if grant_timeslots.len() == num_slots {
                return Some((opportunities_skipped, grant_timeslots));
            }
        }

        // If we get here, we did not find a suitable grant opportunity
        None
    }

    /// Reserves all slots designated in a grant option
    /// If only one halfslot is needed, returns 1 or 2 designating which slot was reserved
    pub fn ul_reserve_grant(
        &mut self,
        ssi: u32,
        grant_timestamps: Vec<TdmaTime>,
        is_halfslot: bool,
    ) -> u8 {
        assert!(!grant_timestamps.is_empty());
        assert!(!is_halfslot || grant_timestamps.len() == 1);
        // let ts = grant_timestamps[0].t as usize;
        for ts in grant_timestamps {
            let index = self.ts_to_sched_index(&ts);

            let elem: &mut TimeslotSchedule = &mut self.sched[ts.t as usize - 1][index];
            if is_halfslot {
                if elem.ul1.is_none() {
                    elem.ul1 = Some(ssi);
                    return 1;
                } else {
                    assert!(
                        elem.ul2.is_none(),
                        "ul_reserve_grant: ul2 already set for ts {:?}, ssi {}",
                        ts,
                        ssi
                    );
                    elem.ul2 = Some(ssi);
                    return 2;
                }
            } else {
                assert!(
                    elem.ul1.is_none(),
                    "ul_reserve_grant: ul1 already set for ts {:?}, ssi {}",
                    ts,
                    ssi
                );
                assert!(
                    elem.ul2.is_none(),
                    "ul_reserve_grant: ul2 already set for ts {:?}, ssi {}",
                    ts,
                    ssi
                );
                elem.ul1 = Some(ssi);
                elem.ul2 = Some(ssi);
            }
        }

        // Full slots reserved
        0
    }

    /// Tries to find a way to satisfy a granting request, and reserves the slots in the schedule.
    /// If successful, returns a BasicSlotgrant with the granting delay and capacity allocation.
    pub fn ul_process_cap_req(
        &mut self,
        timeslot: u8,
        addr: TetraAddress,
        res_req: &ReservationRequirement,
    ) -> Option<BasicSlotgrant> {
        let is_halfslot = res_req == &ReservationRequirement::Req1Subslot;
        let requested_cap = if is_halfslot {
            1
        } else {
            res_req.to_req_slotcount()
        };

        // Find a suitable grant opportunity
        let grant_op = self.ul_find_grant_opportunity(timeslot, requested_cap, is_halfslot);

        tracing::debug!(
            "ul_process_cap_req: addr {}, res_req {:?}, requested_cap {}, is_halfslot {}, grant_op: {:?}",
            addr,
            res_req,
            requested_cap,
            is_halfslot,
            grant_op
        );

        // If found, reserve the slots and return a BasicSlotgrant
        if let Some((skips, grant_timestamps)) = grant_op {
            // Reserve the target granting opportunity. Get subslot (only relevant for halfslot reservation)
            let subslot = self.ul_reserve_grant(addr.ssi, grant_timestamps, is_halfslot);

            // Build BasicSlotgrant response element
            let cap_alloc = if res_req == &ReservationRequirement::Req1Subslot {
                match subslot {
                    1 => BasicSlotgrantCapAlloc::FirstSubslotGranted,
                    2 => BasicSlotgrantCapAlloc::SecondSubslotGranted,
                    _ => unreachable!(
                        "ul_process_cap_req: subslot must be 1 or 2, got {}",
                        subslot
                    ),
                }
            } else {
                BasicSlotgrantCapAlloc::from_req_slotcount(requested_cap)
            };
            let grant_delay = if skips == 0 {
                BasicSlotgrantGrantingDelay::CapAllocAtNextOpportunity
            } else {
                BasicSlotgrantGrantingDelay::DelayNOpportunities(skips as u8)
            };
            Some(BasicSlotgrant {
                capacity_allocation: cap_alloc,
                granting_delay: grant_delay,
            })
        } else {
            tracing::warn!(
                "ul_process_cap_req: no suitable grant opportunity found for addr {}, res_req {:?}",
                addr,
                res_req
            );
            None
        }
    }

    fn ul_get_usage(&self, ts: TdmaTime) -> AccessAssignUlUsage {
        let ul_sched = &self.sched[ts.t as usize - 1][self.ts_to_sched_index(&ts)];
        assert!(ul_sched.ul1.is_some() || ul_sched.ul2.is_none());

        if ul_sched.ul1.is_some() && ul_sched.ul2.is_some() {
            AccessAssignUlUsage::AssignedOnly
        } else if ul_sched.ul1.is_some() {
            AccessAssignUlUsage::CommonAndAssigned
        } else {
            AccessAssignUlUsage::CommonOnly
        }
    }

    ////////// DOWNLINK SCHEDULING /////////

    /// Registers that we should transmit a MAC-RESOURCE or similar with a grant, somewhere this tick
    pub fn dl_enqueue_grant(&mut self, timeslot: u8, addr: TetraAddress, grant: BasicSlotgrant) {
        tracing::debug!(
            "dl_enqueue_grant: ts {} enqueueing PDU {:?} for addr {}",
            timeslot,
            grant,
            addr
        );
        let elem = DlSchedElem::Grant(addr, grant);
        self.dltx_queues[timeslot as usize - 1].push(elem);
    }

    pub fn dl_enqueue_random_access_ack(&mut self, timeslot: u8, addr: TetraAddress) {
        tracing::debug!(
            "dl_enqueue_random_access_ack: ts {} enqueueing random access acknowledgementfor addr {}",
            timeslot,
            addr
        );
        let elem = DlSchedElem::RandomAccessAck(addr);
        self.dltx_queues[timeslot as usize - 1].push(elem);
    }

    pub fn dl_enqueue_tma(&mut self, timeslot: u8, pdu: MacResource, sdu: BitBuffer) {
        tracing::debug!(
            "dl_enqueue_tma: ts {} enqueueing PDU {:?} SDU {}",
            timeslot,
            pdu,
            sdu.dump_bin()
        );
        let elem = DlSchedElem::Resource(pdu, sdu);
        self.dltx_queues[timeslot as usize - 1].push(elem);
    }

    pub fn dl_schedule_tmb(&mut self, _traffic: BitBuffer, _ts: &TdmaTime) {
        unimplemented!("Broadcast scheduling not implemented yet");
    }

    pub fn dl_schedule_tmd(&mut self, _traffic: BitBuffer, _ts: &TdmaTime) {
        unimplemented!("Traffic scheduling not implemented yet");
    }

    /// Takes a block or None value.
    /// If block is present and some signalling channel, and space is available,
    /// adds a trailing Null PDU.
    /// If blk is None, returns None.
    /// Otherwise, returns blk unchanged (eg. for SYNC, broadcast, etc).
    pub fn try_add_null_pdus(&mut self, blk: Option<TmvUnitdataReq>) -> Option<TmvUnitdataReq> {
        // A null pdu in a slot:
        // 0000000000010000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000
        // Oddly, the fill_bits ind is set to 0, while a fill bit is indeed present to fill the slot.
        // We replicate that behavior here.
        if let Some(mut b) = blk {
            if b.logical_channel == LogicalChannel::Stch
                || b.logical_channel == LogicalChannel::SchHd
                || b.logical_channel == LogicalChannel::SchF
            {
                if b.mac_block.get_len_remaining() >= NULL_PDU_LEN_BITS {
                    tracing::trace!("try_add_null_pdus: closing blk with Null PDU");

                    // We have room for a Null PDU
                    let mut pdu = MacResource {
                        fill_bits: false,
                        length_ind: 2, // Null PDU is 16 bits
                        addr: None,
                        ..Default::default()
                    };
                    let _ = pdu.update_len_and_fill_ind(0);
                    pdu.to_bitbuf(&mut b.mac_block);

                    // TODO FIXME: it's possibly the best idea to still add fill bits trailing this null pdu.
                    // Check real-world captures.
                } else {
                    unimplemented!(
                        "try_add_null_pdus: Not enough space for Null PDU in block, got {} bits remaining",
                        b.mac_block.get_len_remaining()
                    );
                }
            }

            Some(b)
        } else {
            None
        }
    }

    /// Returns a mutable reference to the first scheduled resource for the given timeslot and address
    pub fn dl_get_scheduled_resource_for_ssi(
        &mut self,
        ts: TdmaTime,
        addr: &TetraAddress,
    ) -> Option<&mut DlSchedElem> {
        let queue = &mut self.dltx_queues[ts.t as usize - 1];

        for index in 0..queue.len() {
            let elem = &mut queue[index];
            if let DlSchedElem::Resource(pdu, _sdu) = elem {
                if let Some(pdu_ssi) = pdu.addr {
                    if pdu_ssi.ssi == addr.ssi {
                        // Found a resource for this address
                        return queue.get_mut(index);
                    }
                }
            }
        }
        // No resource for this address was found
        None
    }

    /// Make a minimal resource to contain a grant or a random access acknowledgement
    pub fn dl_make_minimal_resource(
        addr: &TetraAddress,
        grant: Option<BasicSlotgrant>,
        random_access_ack: bool,
    ) -> MacResource {
        let mut pdu = MacResource {
            fill_bits: false, // updated later
            pos_of_grant: 0,
            encryption_mode: 0,
            random_access_flag: random_access_ack,
            length_ind: 0, // updated later
            addr: Some(*addr),
            event_label: None,
            usage_marker: None,
            power_control_element: None,
            slot_granting_element: grant,
            chan_alloc_element: None,
        };
        pdu.update_len_and_fill_ind(0);
        pdu
    }

    pub fn dl_take_all_grants_and_acks(&mut self, timeslot: u8) -> Vec<DlSchedElem> {
        let queue = &mut self.dltx_queues[timeslot as usize - 1];
        let mut taken = Vec::new();

        let mut i = 0;
        while i < queue.len() {
            if matches!(
                queue[i],
                DlSchedElem::Grant(_, _) | DlSchedElem::RandomAccessAck(_)
            ) {
                let elem = queue.remove(i);
                taken.push(elem);
            } else {
                i += 1;
            }
        }
        taken
    }

    pub fn dl_integrate_sched_elems_for_timeslot(&mut self, ts: TdmaTime) {
        // Remove all grants and acks from queue and collect them into a vec
        let grants_and_acks = self.dl_take_all_grants_and_acks(ts.t);

        // Process grants and acks
        for elem in grants_and_acks {
            // Try to find existing resource for this address
            let addr = match &elem {
                DlSchedElem::Grant(addr, _) => addr,
                DlSchedElem::RandomAccessAck(addr) => addr,
                _ => panic!(),
            };
            let mac_resource = self.dl_get_scheduled_resource_for_ssi(ts, addr);

            match mac_resource {
                Some(DlSchedElem::Resource(pdu, _sdu)) => {
                    // Integrate grant into the resource
                    match &elem {
                        DlSchedElem::Grant(_, grant) => {
                            tracing::debug!(
                                "dl_integrate_sched_elems_for_timeslot: Integrating grant {:?} into resource for addr {}",
                                grant,
                                addr
                            );
                            pdu.slot_granting_element = Some(grant.clone());
                        }
                        DlSchedElem::RandomAccessAck(_) => {
                            tracing::debug!(
                                "dl_integrate_sched_elems_for_timeslot: Integrating ack into resource for addr {}",
                                addr
                            );
                            pdu.random_access_flag = true;
                        }
                        _ => panic!(),
                    }
                }
                None => {
                    // No resource for this address was found, create a new one

                    let pdu = match &elem {
                        DlSchedElem::Grant(_, grant) => {
                            tracing::debug!(
                                "dl_integrate_sched_elems_for_timeslot: Creating new resource for addr {} with grant {:?}",
                                addr,
                                grant
                            );
                            Self::dl_make_minimal_resource(addr, Some(grant.clone()), false)
                        }
                        DlSchedElem::RandomAccessAck(_) => {
                            tracing::debug!(
                                "dl_integrate_sched_elems_for_timeslot: Creating new resource for addr {} with ack",
                                addr
                            );
                            Self::dl_make_minimal_resource(addr, None, true)
                        }
                        _ => panic!(),
                    };

                    // Push new resource into the queue
                    let dlsched_res = DlSchedElem::Resource(pdu, BitBuffer::new(0));
                    self.dltx_queues[ts.t as usize - 1].push(dlsched_res);
                }
                _ => panic!(),
            }
        }
    }

    /// Return first queued grant.
    /// If none; return first in-progress fragmented message.
    /// If none; return first to-be-transmitted resource.
    /// If none, return None.
    pub fn dl_take_prioritized_sched_item(&mut self, ts: u8) -> Option<DlSchedElem> {
        // Map 1-based ts to 0-based index, bail on 0 or out of range.
        let slot = ts as usize - 1;
        let q = self.dltx_queues.get_mut(slot).unwrap();

        // Return grants first
        if let Some(i) = q.iter().position(|e| matches!(e, DlSchedElem::Grant(_, _))) {
            return Some(q.remove(i));
        }

        // Return FragBufs next
        if let Some(i) = q.iter().position(|e| matches!(e, DlSchedElem::FragBuf(_))) {
            return Some(q.remove(i));
        }

        // Return Resources last
        if let Some(i) = q
            .iter()
            .position(|e| matches!(e, DlSchedElem::Resource(_, _)))
        {
            return Some(q.remove(i));
        }

        None
    }

    pub fn tick_start(&mut self, ts: TdmaTime) {
        // Increment current time
        self.cur_ts = self.cur_ts.add_timeslots(1);
        assert!(
            ts == self.cur_ts,
            "BsChannelScheduler tick_start: ts mismatch, expected {}, got {}",
            self.cur_ts,
            ts
        );
    }

    /// Prepares a scheduled FUTURE timeslot for transfer to lmac and transmission
    /// Generates BBK block
    /// If the timeslot is not full, generates SYNC SB1/SB2 blocks.
    /// Increments cur_ts by one timeslot.
    /// Caller should check timestamp of returned DlTxElem to prevent desync
    pub fn finalize_ts_for_tick(&mut self) -> TmvUnitdataReqSlot {
        // We finalize a FUTURE slot: cur_ts plus some number of timeslots
        let ts = self.cur_ts.add_timeslots(MACSCHED_TX_AHEAD as i32);

        // TODO FIXME allocate only if we have something to put in it
        let mut buf = BitBuffer::new(SCH_F_CAP);

        // Integrate all grants and random access acks into resources (either existing or new)
        self.dl_integrate_sched_elems_for_timeslot(ts);

        while !self.dltx_queues[ts.t as usize - 1].is_empty() {
            let opt = self.dl_take_prioritized_sched_item(ts.t);

            if !(opt.is_none() || ts.t == 1) {
                tracing::error!("got violating element {:?}", opt);
                panic!();
            }

            match opt {
                Some(sched_elem) => match sched_elem {
                    DlSchedElem::Broadcast(_) => {
                        unimplemented_log!(
                            "finalize_ts_for_tick: Broadcast scheduling not implemented"
                        );
                    }

                    DlSchedElem::Resource(mut pdu, mut sdu) => {
                        let sdu_bits = sdu.get_len();
                        let num_fill_bits = pdu.update_len_and_fill_ind(sdu_bits);

                        pdu.to_bitbuf(&mut buf);
                        buf.copy_bits(&mut sdu, sdu_bits);
                        write_fill_bits(&mut buf, Some(num_fill_bits));
                        tracing::debug!("<- finalized {:?} sdu {}", pdu, sdu.dump_bin());
                    }

                    DlSchedElem::FragBuf(_) => {
                        unimplemented_log!(
                            "finalize_ts_for_tick: FragBuf scheduling not implemented"
                        );
                    }

                    _ => panic!(
                        "finalize_ts_for_tick: Unexpected DlSchedElem type: {:?}",
                        sched_elem
                    ),
                },
                None => {
                    // No more items to process, we can finalize this timeslot
                    break;
                }
            }
        }

        // Check if any signalling message was put
        let mut elem = if buf.get_pos() == 0 {
            // Put default SYNC/SYSINFO frame
            TmvUnitdataReqSlot {
                ts,
                blk1: None,
                blk2: None, // MAY be populated later
                bbk: None,  // WILL be populated later
            }
        } else {
            TmvUnitdataReqSlot {
                ts,
                blk1: Some(TmvUnitdataReq {
                    logical_channel: LogicalChannel::SchF,
                    mac_block: buf,
                    scrambling_code: self.scrambling_code.unwrap(),
                }),
                blk2: None, // MAY be populated later
                bbk: None,  // WILL be populated later
            }
        };

        // FIXME implement that sched can contain more items, buf for now, fail if that's the case
        // assert!(self.dl_take_schedule_item(&ts).is_none(), "finalize_ts_for_tick: dl_take_schedule_item should return None, but got {:?}", elem);

        // By default, UL is CommonOnly.
        // If we encounter a subslot grant, we set this to CommonAndAssigned
        // If the full slot is granted (or two subslot grants are encountered), we set this to AssignedOnly
        // let ul_usage = self.ul_get_usage(ts);

        if elem.bbk.is_none() {
            // let index = self.ts_to_sched_index(&ts);
            // let sched = &self.sched[ts.t as usize - 1][index];

            // Generate BBK block
            let mut aach_bb = BitBuffer::new(14);
            if ts.f != 18 {
                let mut aach = AccessAssign::default();

                if elem.blk1.as_ref().is_some_and(|b| {
                    b.logical_channel == LogicalChannel::Stch || b.logical_channel.is_traffic()
                }) {
                    // aach.dl_usage = Some(AccessAssignDlUsage::Traffic());
                    unimplemented!();
                }

                match ts.t {
                    1 => {
                        // STRATEGY 1, always send UL CommonAndAssigned
                        // This seems to cause problems with Motorola, which may refuse random access on anything but CommonOnly
                        // Yields something like: 01000010000100
                        // aach.dl_usage = AccessAssignDlUsage::CommonControl;
                        // aach.ul_usage = AccessAssignUlUsage::CommonAndAssigned;
                        // // f1 gets populated with DL Unallocated marker
                        // aach.f2_af = Some(AccessField{
                        //     access_code: 0,
                        //     base_frame_len: 4,
                        // });
                        // END LEGACY STRATEGY 1

                        // STRATEGY 2, always send UL CommonOnly | AssignedOnly
                        // This is harder since we need to check whether we currently have a grant on the uplink
                        // We try to imitate MBTS which outputs 00 001010 001010
                        aach.dl_usage = AccessAssignDlUsage::CommonControl;
                        aach.ul_usage = self.ul_get_usage(ts);
                        // Set access fields based on usage
                        match aach.ul_usage {
                            AccessAssignUlUsage::CommonOnly => {
                                aach.f1_af1 = Some(AccessField {
                                    access_code: 0,
                                    base_frame_len: 4,
                                });
                                aach.f2_af2 = Some(AccessField {
                                    access_code: 0,
                                    base_frame_len: 4,
                                });
                            }
                            AccessAssignUlUsage::CommonAndAssigned
                            | AccessAssignUlUsage::AssignedOnly => {
                                aach.f2_af = Some(AccessField {
                                    access_code: 0,
                                    base_frame_len: 4,
                                });
                            }
                            _ => {
                                // Traffic or unallocated; no AccessFields
                            }
                        }
                    }
                    2..=4 => {
                        // Additional channels, unallocated except we sent a chanalloc
                        // Those are currently unimplemented, so, unallocated it is
                        aach.dl_usage = AccessAssignDlUsage::Unallocated;
                        aach.ul_usage = AccessAssignUlUsage::Unallocated;
                    }
                    _ => panic!("finalize_ts_for_tick: invalid timeslot {}", ts.t),
                }

                // if ts.t < 4 {
                //     // ts 1,2,3
                //     aach.dl_usage = AccessAssignDlUsage::CommonControl;
                //     aach.ul_usage = AccessAssignUlUsage::CommonAndAssigned;
                //     // f1 gets populated with DL Unallocated marker
                //     aach.f2_af = Some(AccessField{
                //         access_code: 0,
                //         base_frame_len: 4,
                //     });
                // } else {
                //     // ts 4
                //     aach.dl_usage = AccessAssignDlUsage::Unallocated;
                //     aach.ul_usage = AccessAssignUlUsage::Unallocated;
                //     // f1 and f2 are populated with Unallocated markers
                // }

                // TODO FIXME: Access field defaults are possibly not great
                // TODO FIXME: support assigned control
                aach.to_bitbuf(&mut aach_bb);
            } else {
                // Fr18
                let aach = AccessAssignFr18 {
                    ul_usage: AccessAssignUlUsage::CommonOnly,
                    f1_af1: Some(AccessField {
                        access_code: 0,
                        base_frame_len: 1,
                    }),
                    f2_af2: Some(AccessField {
                        access_code: 0,
                        base_frame_len: 0,
                    }),
                    ..Default::default()
                };
                // TODO FIXME: Access field defaults are possibly not great
                aach.to_bitbuf(&mut aach_bb);
            }

            let bbk = TmvUnitdataReq {
                logical_channel: LogicalChannel::Aach,
                mac_block: aach_bb,
                scrambling_code: self.scrambling_code.unwrap(),
            };

            elem.bbk = Some(bbk);
        } else {
            panic!();
        }

        // tracing::trace!("finalize_ts_for_tick: have {}{}{}",
        //     if elem.bbk.is_some() { "bbk " } else { "" },
        //     if elem.blk1.is_some() { "blk1 " } else { "" },
        //     if elem.blk2.is_some() { "blk2 " } else { "" });

        // Populate blk1 if empty: BSCH on frame 18, SCH/HD on other frames
        if elem.blk1.is_none() {
            if ts.f == 18 {
                // Frame 18: BSCH (SDB burst) with SYNC
                let mut buf = BitBuffer::new(60);
                if let Some(ref mut precomps) = self.precomps {
                    precomps.mac_sync.time = ts;
                    precomps.mac_sync.to_bitbuf(&mut buf);
                    precomps.mle_sync.to_bitbuf(&mut buf);
                } else {
                    panic!("precomps not available");
                };

                elem.blk1 = Some(TmvUnitdataReq {
                    logical_channel: LogicalChannel::Bsch,
                    mac_block: buf,
                    scrambling_code: SCRAMB_INIT,
                });
            } else {
                // Frames 1-17: SCH/HD (NDB burst) with NULL PDU
                let mut buf = BitBuffer::new(124);
                let mut pdu = MacResource {
                    fill_bits: false,
                    length_ind: 2,
                    addr: None,
                    ..Default::default()
                };
                let _ = pdu.update_len_and_fill_ind(0);
                pdu.to_bitbuf(&mut buf);

                elem.blk1 = Some(TmvUnitdataReq {
                    logical_channel: LogicalChannel::SchHd,
                    mac_block: buf,
                    scrambling_code: self.scrambling_code.unwrap(),
                });
            }
        }

        // Check if second block may still be populated (blk1 is half-slot and blk2 is None)
        let blk1_lchan = elem.blk1.as_ref().unwrap().logical_channel;
        assert!(blk1_lchan != LogicalChannel::Stch, "unimplemented");

        // Populate blk2 with SYSINFO if blk1 is half-slot
        if elem.blk2.is_none()
            && (blk1_lchan == LogicalChannel::Bsch
                || blk1_lchan == LogicalChannel::SchHd
                || blk1_lchan == LogicalChannel::Stch)
        {
            assert!(elem.blk1.as_ref().unwrap().mac_block.get_len() <= 124);

            let mut buf = BitBuffer::new(124);

            // Update hyperframe and write MAC-SYSINFO (alternating sysinfo1/sysinfo2)
            let precomps = self.precomps.as_mut().unwrap();
            if ts.t % 2 == 1 {
                precomps.mac_sysinfo1.hyperframe_number = Some(ts.h);
                precomps.mac_sysinfo1.to_bitbuf(&mut buf);
            } else {
                precomps.mac_sysinfo2.hyperframe_number = Some(ts.h);
                precomps.mac_sysinfo2.to_bitbuf(&mut buf);
            }
            self.precomps
                .as_ref()
                .unwrap()
                .mle_sysinfo
                .to_bitbuf(&mut buf);

            elem.blk2 = Some(TmvUnitdataReq {
                logical_channel: LogicalChannel::Bnch,
                mac_block: buf,
                scrambling_code: self.scrambling_code.unwrap(),
            })

            // TESTING CODE: DL/UL SYNC CHECKING
            // let mut buf = BitBuffer::new(124); // Exactly 124
            // buf.write_bits(0xFFFF, 16);
            // buf.write_bits(ts.t as u64, 8);
            // buf.write_bits(ts.f as u64, 8);
            // buf.write_bits(ts.m as u64, 8);
            // buf.write_bits(ts.h as u64, 16);
            // buf.write_bits(0xFFFF, 16);
            // buf.seek(0);
            // elem.blk2 = Some(TmvUnitdataReq {
            //     logical_channel: LogicalChannel::BNCH,
            //     mac_block: buf,
            //     scrambling_code: self.scrambling_code.unwrap(),
            // });
        } else {
            // We're done, no blk2 needed. Just a quick check blk1 is indeed long
            assert!(
                elem.blk1.as_ref().unwrap().mac_block.get_len() == 268,
                "blk1 is long, but blk2 is set!"
            );
        }

        assert!(
            elem.bbk.is_some(),
            "finalize_ts_for_tick: BBK block is not set, this should not happen"
        );
        assert!(
            elem.blk1.is_some(),
            "finalize_ts_for_tick: blk1 block is not set, this should not happen"
        );

        // If signalling channels are here, and there is spare room, we need to close them with a Null pdu
        elem.blk1 = self.try_add_null_pdus(elem.blk1);
        elem.blk2 = self.try_add_null_pdus(elem.blk2);

        // Move all BitBuffer positions to the start of the window
        elem.bbk.as_mut().unwrap().mac_block.seek(0);
        elem.blk1.as_mut().unwrap().mac_block.seek(0);
        if let Some(blk2) = elem.blk2.as_mut() {
            blk2.mac_block.seek(0);
        }

        // Clear UL schedule for this timeslot
        let index = self.ts_to_sched_index(&ts);
        self.sched[ts.t as usize - 1][index].ul1 = None;
        self.sched[ts.t as usize - 1][index].ul2 = None;

        // We now have our bbk, blk1 and (optional) blk2
        elem
    }

    pub fn dump_ul_schedule(&self, skip_empty: bool) {
        tracing::trace!("Dumping uplink schedule:");
        for dist in 0..MACSCHED_NUM_FRAMES {
            let ts = self.cur_ts.add_timeslots(dist as i32 * 4);
            let index = self.ts_to_sched_index(&ts);
            let elem = &self.sched[ts.t as usize - 1][index];
            if skip_empty && elem.ul1.is_none() && elem.ul2.is_none() && elem.dl.is_none() {
                continue;
            }
            tracing::trace!("  Schedule {}: {:?}", ts, elem);
        }
    }

    pub fn dump_dl_queue(&self) {
        tracing::trace!("Dumping downlink queue:");
        for (index, elem) in self.dltx_queues.iter().enumerate() {
            for e in elem {
                tracing::trace!("  ts[{}] {:?}", index, e);
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::{
        common::{
            address::{SsiType, TetraAddress},
            debug::setup_logging_default,
        },
        entities::{
            mle::fields::bs_service_details::BsServiceDetails,
            umac::{
                enums::sysinfo_opt_field_flag::SysinfoOptFieldFlag,
                fields::{
                    sysinfo_default_def_for_access_code_a::SysinfoDefaultDefForAccessCodeA,
                    sysinfo_ext_services::SysinfoExtendedServices,
                },
            },
        },
    };

    use super::*;

    pub fn get_testing_slotter() -> BsChannelScheduler {
        let _ = setup_logging_default(None);

        // TODO FIXME make all parameters configurable
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
            main_carrier: 1001,
            freq_band: 4,
            freq_offset_index: 0,
            duplex_spacing: 0,
            reverse_operation: false,
            num_of_csch: 0,
            ms_txpwr_max_cell: 5,
            rxlev_access_min: 3,
            access_parameter: 7,
            radio_dl_timeout: 15,
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
            cck_id: sysinfo1.cck_id, // TODO FIXME change to Some() when we enable encryption
            hyperframe_number: sysinfo1.hyperframe_number,
            option_field: SysinfoOptFieldFlag::ExtServicesBroadcast,
            ts_common_frames: None,
            default_access_code: None,
            ext_services: Some(ext_services),
        };

        let mle_sysinfo_pdu = DMleSysinfo {
            location_area: 2,
            subscriber_class: 65535, // All subscriber classes allowed
            bs_service_details: BsServiceDetails {
                registration: true,
                deregistration: true,
                priority_cell: false,
                no_minimum_mode: true,
                migration: false,
                system_wide_services: false,
                voice_service: true,
                circuit_mode_data_service: false,
                sndcp_service: false,
                aie_service: false,
                advanced_link: false,
            },
        };

        let mac_sync_pdu = MacSync {
            system_code: 1,
            colour_code: 1,
            time: TdmaTime::default(),
            sharing_mode: 0, // Continuous transmission
            ts_reserved_frames: 0,
            u_plane_dtx: false,
            frame_18_ext: false,
        };

        let mle_sync_pdu = DMleSync {
            mcc: 204,
            mnc: 1337,
            neighbor_cell_broadcast: 2,
            cell_load_ca: 0,
            late_entry_supported: true,
        };

        let precomps = PrecomputedUmacPdus {
            mac_sysinfo1: sysinfo1,
            mac_sysinfo2: sysinfo2,
            mle_sysinfo: mle_sysinfo_pdu,
            mac_sync: mac_sync_pdu,
            mle_sync: mle_sync_pdu,
        };

        let mut sched = BsChannelScheduler::new();
        sched.set_scrambling_code(1);
        sched.set_precomputed_msgs(precomps);
        sched.set_dl_time(TdmaTime::default());
        sched
    }

    #[test]
    fn test_halfslot_grants() {
        let mut sched = get_testing_slotter();
        let resreq = ReservationRequirement::Req1Subslot;
        let addr = TetraAddress {
            encrypted: false,
            ssi_type: SsiType::Issi,
            ssi: 1234,
        };
        let grant1 = sched.ul_process_cap_req(1, addr, &resreq);
        tracing::info!("grant1: {:?}", grant1);
        assert!(
            grant1.is_some(),
            "ul_process_cap_req should return Some, but got None"
        );

        let u1 = sched.ul_get_usage(TdmaTime {
            t: 1,
            f: 1,
            m: 1,
            h: 0,
        });
        let u2 = sched.ul_get_usage(TdmaTime {
            t: 1,
            f: 2,
            m: 1,
            h: 0,
        });
        let u3 = sched.ul_get_usage(TdmaTime {
            t: 1,
            f: 3,
            m: 1,
            h: 0,
        });
        tracing::info!("usage ts 1/2/3: {:?}/{:?}/{:?}", u1, u2, u3);

        let cap_alloc1 = grant1.unwrap().capacity_allocation;
        assert_eq!(
            cap_alloc1,
            BasicSlotgrantCapAlloc::FirstSubslotGranted,
            "ul_process_cap_req should return FirstSubslotGranted, but got {:?}",
            cap_alloc1
        );
        let grant2 = sched.ul_process_cap_req(1, addr, &resreq);
        tracing::info!("grant2: {:?}", grant2);
        assert!(
            grant2.is_some(),
            "ul_process_cap_req should return Some, but got None"
        );
        let cap_alloc2 = grant2.unwrap().capacity_allocation;
        assert_eq!(
            cap_alloc2,
            BasicSlotgrantCapAlloc::SecondSubslotGranted,
            "ul_process_cap_req should return SecondSubslotGranted, but got {:?}",
            cap_alloc2
        );

        let u1 = sched.ul_get_usage(TdmaTime {
            t: 1,
            f: 1,
            m: 1,
            h: 0,
        });
        let u2 = sched.ul_get_usage(TdmaTime {
            t: 1,
            f: 2,
            m: 1,
            h: 0,
        });
        let u3 = sched.ul_get_usage(TdmaTime {
            t: 1,
            f: 3,
            m: 1,
            h: 0,
        });
        tracing::info!("usage ts 1/2/3: {:?}/{:?}/{:?}", u1, u2, u3);
    }

    #[test]
    fn test_halfslot_and_fullslot_grant() {
        let mut sched = get_testing_slotter();
        let resreq1 = ReservationRequirement::Req1Subslot;
        let addr = TetraAddress {
            encrypted: false,
            ssi_type: SsiType::Issi,
            ssi: 1234,
        };

        sched.dump_ul_schedule(true);
        let grant1 = sched.ul_process_cap_req(1, addr, &resreq1);
        tracing::info!("grant1: {:?}", grant1);

        let u1 = sched.ul_get_usage(TdmaTime {
            t: 1,
            f: 1,
            m: 1,
            h: 0,
        });
        let u2 = sched.ul_get_usage(TdmaTime {
            t: 1,
            f: 2,
            m: 1,
            h: 0,
        });
        let u3 = sched.ul_get_usage(TdmaTime {
            t: 1,
            f: 3,
            m: 1,
            h: 0,
        });
        tracing::info!("usage ts 1/2/3: {:?}/{:?}/{:?}", u1, u2, u3);

        assert!(grant1.is_some());
        let cap_alloc1 = grant1.unwrap().capacity_allocation;
        assert_eq!(cap_alloc1, BasicSlotgrantCapAlloc::FirstSubslotGranted);

        sched.dump_ul_schedule(true);
        let resreq2 = ReservationRequirement::Req3Slots;
        let Some(grant2) = sched.ul_process_cap_req(1, addr, &resreq2) else {
            panic!()
        };
        tracing::info!("grant2: {:?}", grant2);
        sched.dump_ul_schedule(true);

        let u1 = sched.ul_get_usage(TdmaTime {
            t: 1,
            f: 1,
            m: 1,
            h: 0,
        });
        let u2 = sched.ul_get_usage(TdmaTime {
            t: 1,
            f: 2,
            m: 1,
            h: 0,
        });
        let u3 = sched.ul_get_usage(TdmaTime {
            t: 1,
            f: 3,
            m: 1,
            h: 0,
        });
        tracing::info!("usage ts 1/2/3: {:?}/{:?}/{:?}", u1, u2, u3);

        assert_eq!(
            grant2.capacity_allocation,
            BasicSlotgrantCapAlloc::Grant3Slots
        );
        assert_eq!(
            grant2.granting_delay,
            BasicSlotgrantGrantingDelay::DelayNOpportunities(1)
        );
    }

    #[test]
    fn test_dl_grant_and_ack_integration() {
        let mut sched = get_testing_slotter();
        let ts = TdmaTime::default();
        let addr = TetraAddress {
            encrypted: false,
            ssi_type: SsiType::Issi,
            ssi: 1234,
        };
        let pdu = BsChannelScheduler::dl_make_minimal_resource(&addr, None, false);
        let sdu = BitBuffer::new(0);
        sched.dl_enqueue_tma(ts.t, pdu, sdu);

        let grant = BasicSlotgrant {
            capacity_allocation: BasicSlotgrantCapAlloc::FirstSubslotGranted,
            granting_delay: BasicSlotgrantGrantingDelay::CapAllocAtNextOpportunity,
        };

        sched.dl_enqueue_grant(ts.t, addr, grant);
        sched.dl_enqueue_random_access_ack(ts.t, addr);

        sched.dump_ul_schedule(true);
        sched.dump_dl_queue();

        assert!(sched.dltx_queues[ts.t as usize - 1].len() == 3);

        tracing::info!("Integrating queue");
        sched.dl_integrate_sched_elems_for_timeslot(ts);

        sched.dump_ul_schedule(true);
        sched.dump_dl_queue();

        assert!(sched.dltx_queues[ts.t as usize - 1].len() == 1);
    }
}
