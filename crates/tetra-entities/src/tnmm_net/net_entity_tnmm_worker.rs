use crossbeam_channel::{Receiver, Sender};

use crate::{
    network::{
        netentity::NetEntityWorker,
        transports::{NetworkMessage, NetworkTransport},
    },
    tnmm_net::{
        codec::TnmmTestCodec,
        test_pdu::{TestPdu, TestRequest, TestResponse, pack_test_pdu},
    },
};
use tetra_core::{TdmaTime, tetra_common::Sap, tetra_entities::TetraEntity};
use tetra_saps::{
    sapmsg::{SapMsg, SapMsgInner},
    tnmm::TnmmTestResponse,
};

/// Worker thread that handles all blocking network operations
///
/// Generic over transport type `T` - can work with TCP, QUIC, or any other
/// transport that implements `NetworkTransport`.
pub struct NetEntityTnmmWorker<T: NetworkTransport> {
    /// Debug label for this worker
    label: &'static str,
    /// TETRA entity type this network entity represents
    entity_self: TetraEntity,
    /// Destination entity, the other entity on our SAP
    entity_dest: TetraEntity,
    /// The SAP we're listening on
    sap: Sap,
    /// The transport (TCP, QUIC, etc.)
    transport: T,
    /// Codec for ser/des
    codec: TnmmTestCodec,
    /// Requests receiver from main thread
    e2w_receiver: Receiver<SapMsg>,
    /// Response sender back to main thread
    w2e_sender: Sender<SapMsg>,
}

impl<T: NetworkTransport + 'static> NetEntityWorker for NetEntityTnmmWorker<T> {
    type Transport = T;

    fn new(
        entity_self: TetraEntity,
        entity_dest: TetraEntity,
        sap: Sap,
        w2e_sender: Sender<SapMsg>,
        e2w_receiver: Receiver<SapMsg>,
        transport: Self::Transport,
    ) -> Self {
        Self {
            label: "NetEntityTnmmWorker",
            entity_self,
            entity_dest,
            sap,
            transport,
            codec: TnmmTestCodec::default(),
            w2e_sender,
            e2w_receiver,
        }
    }

    fn run(&mut self) {
        // Initial connect; okay if it fails, we'll retry on send
        tracing::info!("{} thread started", self.label);
        let _ = self.transport.connect();
        tracing::info!("{} thread started", self.label);

        while let Ok(msg) = self.e2w_receiver.recv() {
            let inner = &msg.msg;
            match inner {
                SapMsgInner::TnmmTestDemand(demand) => {
                    tracing::info!("{} received TnmmTestDemand: {:?}", self.label, demand);
                    self.handle_test_request(msg)
                }
                _ => panic!("{} Unhandled message: {:?}", self.label, inner),
            };

            // Wait for response with blocking read and timeout
            match self.transport.wait_for_response_reliable() {
                Ok(net_msg) => {
                    // tracing::debug!("{} received response: {:?}", self.label, net_msg);
                    self.handle_network_message(net_msg);
                }
                Err(e) => {
                    tracing::error!("{} failed to receive response: {:?}", self.label, e);
                }
            };
        }

        tracing::info!("{} thread stopped", self.label);
    }
}

impl<T: NetworkTransport> NetEntityTnmmWorker<T> {
    /// Handle TestRequest coming from the main thread
    fn handle_test_request(&mut self, message: SapMsg) {
        let SapMsgInner::TnmmTestDemand(prim) = message.msg else { panic!() };

        // Build a TestRequest PDU with current protocol version
        let pdu = pack_test_pdu(TestPdu::TestRequest(TestRequest { handle: 0, ssi: prim.issi }));

        let encoded = match self.codec.encode(&pdu) {
            Ok(data) => data,
            Err(e) => {
                tracing::error!("{} failed to encode TestRequest: {}", self.label, e);
                return;
            }
        };

        match self.transport.send_reliable(&encoded) {
            Ok(()) => {
                tracing::debug!("{} sent TestRequest ({} bytes)", self.label, encoded.len());
            }
            Err(e) => {
                // TODO send error to NetEntity
                tracing::error!("{} failed to send TestRequest: {}", self.label, e);
            }
        }
    }

    /// Handle TestResponse coming in from the network
    fn handle_test_response(&mut self, message: TestResponse) {
        let inner = TnmmTestResponse {
            issi: message.ssi,
            data: message.data,
        };
        let msg = SapMsg {
            sap: self.sap,
            src: self.entity_self,
            dest: self.entity_dest,
            dltime: TdmaTime::default(),
            msg: SapMsgInner::TnmmTestResponse(inner),
        };
        match self.w2e_sender.send(msg) {
            Ok(()) => {}
            Err(e) => tracing::error!("{} failed to send TestResponse to main thread: {:?}", self.label, e),
        };
    }

    /// Process incoming network messages (polling mode)
    fn process_incoming_messages(&mut self) {
        let msgs = self.transport.receive_reliable();
        for net_msg in msgs {
            self.handle_network_message(net_msg);
        }
    }

    /// Handle a single network message (used by both polling and blocking modes)
    fn handle_network_message(&mut self, net_msg: NetworkMessage) {
        tracing::debug!("{} incoming message: {} bytes", self.label, net_msg.payload.len());

        let pdu = match self.codec.decode(&net_msg.payload) {
            Ok(pdu) => pdu,
            Err(e) => {
                tracing::error!("{} failed to decode incoming TestPdu: {}", self.label, e);
                return;
            }
        };

        tracing::debug!("{} decoded: {:?}", self.label, pdu);
        match pdu.payload {
            Some(TestPdu::HeartbeatTock(message)) => {
                tracing::info!("{} received HeartbeatTock from {:?}: {:?}", self.label, net_msg.source, message);
            }
            Some(TestPdu::TestResponse(message)) => {
                tracing::info!("{} received TestResponse from {:?}: {:?}", self.label, net_msg.source, message);
                self.handle_test_response(message)
            }
            _ => {
                tracing::warn!("{} received unhandled TestPdu from {:?}: {:?}", self.label, net_msg.source, pdu);
            }
        }
    }
}
