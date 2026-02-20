use crossbeam_channel::Sender;
use std::panic;

use tetra_config::SharedConfig;
use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{BitBuffer, BurstType, PhyBlockNum, PhyBlockType, Sap, TdmaTime, TrainingSequence};
use tetra_pdus::phy::traits::rxtx_dev::RxBurstBits;
use tetra_pdus::phy::traits::rxtx_dev::{RxTxDev, TxSlotBits};
use tetra_saps::tp::TpUnitdataInd;
use tetra_saps::{SapMsg, SapMsgInner};

use crate::phy::components::phy_io_file::{FileWriteMsg, PhyIoFileMode};
use crate::phy::components::{burst_consts::*, slotter, train_consts::*};
use crate::umac::subcomp::bs_sched::MACSCHED_TX_AHEAD;
use crate::{MessageQueue, TetraEntityTrait};

use super::components::phy_io_file::PhyIoFile;

pub struct PhyBs<D: RxTxDev> {
    config: SharedConfig,
    dltime: TdmaTime,

    /// Channel for asynchronous downlink TX data logging
    dl_tx_sender: Option<Sender<FileWriteMsg>>,
    /// Channel for asynchronous uplink RX data logging
    ul_rx_sender: Option<Sender<FileWriteMsg>>,

    /// Testing mode: Transmit input data from file instead of from stack
    dl_input_file: Option<PhyIoFile>,
    /// Testing mode: Parse input data from file instead of from SDR
    ul_input_file: Option<PhyIoFile>,

    /// RX/TX device
    rxtxdev: D,

    tick: u64,
}

impl<D: RxTxDev> PhyBs<D> {
    pub fn new(config: SharedConfig, rxtxdev: D) -> Self {
        let c = &config.config().phy_io;

        // Create async writers for file logging of generated DL and received UL signals
        let dl_tx_logger = c
            .dl_tx_file
            .as_ref()
            .and_then(|f| PhyIoFile::create_async_writer(f, "dl_tx_logger".to_string()).ok());
        let ul_rx_logger = c
            .ul_rx_file
            .as_ref()
            .and_then(|f| PhyIoFile::create_async_writer(f, "ul_rx_logger".to_string()).ok());

        // Open input files overriding either generated DL or received UL data
        let dl_input_file = if let Some(ref f) = c.dl_input_file {
            Some(PhyIoFile::new(f, PhyIoFileMode::ReadRepeat).expect("Failed to open dl_input_file"))
        } else {
            None
        };
        let ul_input_file = if let Some(ref f) = c.ul_input_file {
            Some(PhyIoFile::new(f, PhyIoFileMode::Read).expect("Failed to open ul_input_file"))
        } else {
            None
        };

        Self {
            config,
            dltime: TdmaTime::default(), // updated in tick_start
            dl_tx_sender: dl_tx_logger,
            ul_rx_sender: ul_rx_logger,
            dl_input_file,
            ul_input_file,
            rxtxdev,
            tick: 0,
        }
    }

    fn send_rxblock_to_lmac(
        queue: &mut MessageQueue,
        train_type: TrainingSequence,
        burst_type: BurstType,
        block_type: PhyBlockType,
        block_num: PhyBlockNum,
        bits: BitBuffer,
        dltime: TdmaTime,
    ) {
        // Uplink timeslot is two after downlink. Thus was transmitted at dltime - 2
        let msg_ts = dltime.add_timeslots(-2);
        let sapmsg = SapMsg {
            sap: Sap::TpSap,
            src: TetraEntity::Phy,
            dest: TetraEntity::Lmac,
            dltime: msg_ts,
            msg: SapMsgInner::TpUnitdataInd(TpUnitdataInd {
                train_type,
                burst_type,
                block_type,
                block_num,
                block: bits,
            }),
        };
        queue.push_back(sapmsg);
    }

    fn split_rxslot_and_send_to_lmac(queue: &mut MessageQueue, burst: &RxBurstBits<'_>, dltime: TdmaTime) {
        let train_seq = burst.train_type;
        match train_seq {
            TrainingSequence::NormalTrainSeq1 => {
                assert!(burst.bits.len() == NUB_BITS);

                let mut blk = BitBuffer::new(NUB_BLK_BITS * 2);
                blk.copy_bits_from_bitarr(&burst.bits[NUB_BLK1_OFFSET..NUB_BLK1_OFFSET + NUB_BLK_BITS]);
                blk.copy_bits_from_bitarr(&burst.bits[NUB_BLK2_OFFSET..NUB_BLK2_OFFSET + NUB_BLK_BITS]);
                blk.seek(0);

                Self::send_rxblock_to_lmac(queue, train_seq, BurstType::NUB, PhyBlockType::NUB, PhyBlockNum::Both, blk, dltime);
            }

            TrainingSequence::NormalTrainSeq2 => {
                assert!(burst.bits.len() == NUB_BITS);

                let blk1 = BitBuffer::from_bitarr(&burst.bits[NUB_BLK1_OFFSET..NUB_BLK1_OFFSET + NUB_BLK_BITS]);
                let blk2 = BitBuffer::from_bitarr(&burst.bits[NUB_BLK2_OFFSET..NUB_BLK2_OFFSET + NUB_BLK_BITS]);

                Self::send_rxblock_to_lmac(
                    queue,
                    train_seq,
                    BurstType::NUB,
                    PhyBlockType::NUB,
                    PhyBlockNum::Block1,
                    blk1,
                    dltime,
                );
                Self::send_rxblock_to_lmac(
                    queue,
                    train_seq,
                    BurstType::NUB,
                    PhyBlockType::NUB,
                    PhyBlockNum::Block2,
                    blk2,
                    dltime,
                );
            }
            TrainingSequence::ExtendedTrainSeq => {
                assert!(burst.bits.len() == CUB_BITS);

                let mut blk = BitBuffer::new(CUB_BLK_BITS * 2);
                blk.copy_bits_from_bitarr(&burst.bits[CUB_BLK1_OFFSET..CUB_BLK1_OFFSET + CUB_BLK_BITS]);
                blk.copy_bits_from_bitarr(&burst.bits[CUB_BLK2_OFFSET..CUB_BLK2_OFFSET + CUB_BLK_BITS]);
                blk.seek(0);

                Self::send_rxblock_to_lmac(
                    queue,
                    train_seq,
                    BurstType::CUB,
                    PhyBlockType::SSN1,
                    PhyBlockNum::Block1,
                    blk,
                    dltime,
                );
            }

            _ => panic!(),
        }
    }

    fn rx_tpsap_prim(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        // Handle TpUnitdataReq with a TX slot
        // Prepare TxSlotBits for transmission
        // TODO FIXME: optimize

        self.tick += 1;

        let SapMsgInner::TpUnitdataReq(prim) = message.msg else { panic!() };

        // Generate block (from file or from LMAC data)
        let mut dl_burst = [0u8; TIMESLOT_TYPE4_BITS];
        if let Some(dl_input_file) = &mut self.dl_input_file {
            // Code for testing mode, when replaying from DL input file
            dl_input_file.read_block(&mut dl_burst).expect("Failed to read dl_input_file data");
        } else {
            // We received data from LMAC, convert BBK block to bitarr
            assert!(prim.bbk.is_some());
            let mut bbk = [0u8; 30];
            prim.bbk.unwrap().to_bitarr(&mut bbk);

            // Build NDB or SDB burst
            dl_burst = match prim.burst_type {
                BurstType::SDB => {
                    // SDB burst
                    assert!(prim.train_type == TrainingSequence::SyncTrainSeq);
                    assert!(prim.blk1.is_some() && prim.blk2.is_some());

                    let mut blk1 = [0u8; 120];
                    let mut blk2 = [0u8; 216];
                    prim.blk1.unwrap().to_bitarr(&mut blk1); // Guaranteed for SDB
                    prim.blk2.unwrap().to_bitarr(&mut blk2); // Guaranteed for SDB

                    slotter::build_sdb(&blk1, &bbk, &blk2)
                }
                BurstType::NDB => {
                    let mut blk1 = [0u8; 216];
                    let mut blk2 = [0u8; 216];

                    match prim.train_type {
                        TrainingSequence::NormalTrainSeq1 => {
                            // Single large block
                            assert!(prim.blk1.is_some() && prim.blk2.is_none());
                            let mut blk1_src = prim.blk1.unwrap(); // Guaranteed for NDB
                            blk1_src.to_bitarr(&mut blk1);
                            blk1_src.to_bitarr(&mut blk2);
                        }
                        TrainingSequence::NormalTrainSeq2 => {
                            // Two half slots
                            assert!(prim.blk1.is_some() && prim.blk2.is_some());
                            prim.blk1.unwrap().to_bitarr(&mut blk1); // Guaranteed for NDB
                            prim.blk2.unwrap().to_bitarr(&mut blk2); // Guaranteed for NDB trainseq 2
                        }
                        _ => panic!("Unsupported training sequence for NDB burst"),
                    }

                    slotter::build_ndb(prim.train_type, &blk1, &bbk, &blk2)
                }
                _ => panic!(),
            };
        }

        // Prepare the TX slot for the tx device
        let tx_slot: [TxSlotBits; 1] = [TxSlotBits {
            time: message.dltime.add_timeslots(MACSCHED_TX_AHEAD as i32),
            slot: Some(&dl_burst),
            ..Default::default()
        }];

        // Code for testing mode, when capturing all DL output to file
        if let Some(dl_tx_sender) = &self.dl_tx_sender {
            let _ = dl_tx_sender.try_send(FileWriteMsg::WriteBlock(dl_burst.to_vec()));
        }

        // Transmit slot and receive rx data (if any trainseq was found)
        // This function is blocking and the source of timing sync in the whole stack
        // let tick_done = std::time::Instant::now();
        let rx = self.rxtxdev.rxtx_timeslot(&tx_slot).expect("Got error from rxtx_timeslot");
        // let new_tick_start = std::time::Instant::now();
        // let elapsed = new_tick_start.duration_since(tick_done);
        // tracing::debug!("rxtx_timeslot: tick_done {:?}, new_tick_start {:?}, elapsed {:?}", tick_done, new_tick_start, elapsed);

        // Process received slot (either full, subslot1 or subslot2)
        // In exceptional cases, we might receive multiple slots (multiple possible detected bursts in one timeslot)
        // This may be due to two subslots, or due to false psoitives in training seq detection
        // The Lmac error correction will eliminate the false positives
        for rx_slot in rx {
            if let Some(rx_slot) = rx_slot {
                let mut slot_sent = false;
                if rx_slot.slot.train_type != TrainingSequence::NotFound {
                    tracing::info!(ts=%self.dltime, "rx_tpsap_prim got {:?} in fullslot", rx_slot.slot.train_type);

                    if let Some(ul_rx_sender) = &self.ul_rx_sender {
                        // Log received data to file (non-blocking)
                        let _ = ul_rx_sender.try_send(FileWriteMsg::WriteHeaderAndBlock(3, self.tick, rx_slot.slot.bits.to_vec()));
                    }

                    Self::split_rxslot_and_send_to_lmac(queue, &rx_slot.slot, self.dltime);
                    slot_sent = true;
                }
                if rx_slot.subslot1.train_type != TrainingSequence::NotFound {
                    tracing::info!(ts=%self.dltime, "rx_tpsap_prim got {:?} in subslot1", rx_slot.subslot1.train_type);
                    if slot_sent {
                        tracing::warn!("Sending same burst twice to LMAC");
                    }
                    if let Some(ul_rx_sender) = &self.ul_rx_sender {
                        // Log received data to file (non-blocking)
                        let _ = ul_rx_sender.try_send(FileWriteMsg::WriteHeaderAndBlock(1, self.tick, rx_slot.subslot1.bits.to_vec()));
                    }

                    Self::split_rxslot_and_send_to_lmac(queue, &rx_slot.subslot1, self.dltime);
                    slot_sent = true;
                }
                if rx_slot.subslot2.train_type != TrainingSequence::NotFound {
                    tracing::info!(ts=%self.dltime, "rx_tpsap_prim got {:?} in subslot2", rx_slot.subslot2.train_type);
                    if slot_sent {
                        tracing::warn!("Sending same burst twice to LMAC");
                    }
                    if let Some(ul_rx_sender) = &self.ul_rx_sender {
                        // Log received data to file (non-blocking)
                        let _ = ul_rx_sender.try_send(FileWriteMsg::WriteHeaderAndBlock(2, self.tick, rx_slot.subslot2.bits.to_vec()));
                    }

                    Self::split_rxslot_and_send_to_lmac(queue, &rx_slot.subslot2, self.dltime);
                }
            }
        }
    }

    fn rx_tpc_prim(&mut self, _queue: &mut MessageQueue, _message: SapMsg) {
        unimplemented!();
    }
}

impl<D: RxTxDev + Send + 'static> TetraEntityTrait for PhyBs<D> {
    fn entity(&self) -> TetraEntity {
        TetraEntity::Phy
    }

    fn rx_prim(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        tracing::debug!("rx_prim: {:?}", message);
        // tracing::debug!(ts=%message.dltime, "rx_prim: {:?}", message);

        match message.sap {
            Sap::TpSap => {
                self.rx_tpsap_prim(queue, message);
            }
            Sap::TpcSap => {
                self.rx_tpc_prim(queue, message);
            }
            _ => {
                panic!();
            }
        }
    }

    fn tick_start(&mut self, _queue: &mut MessageQueue, ts: TdmaTime) {
        self.dltime = ts;
    }
}
