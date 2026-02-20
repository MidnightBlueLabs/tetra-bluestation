use std::collections::VecDeque;

use tetra_core::unimplemented_log;

/// States for a downlink call. Can originate from:
/// - A local MS that opens a call using U-SETUP
/// - A remote, networked call incoming from the call router
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DlCallState {
    /// Call is created but setup messages have not yet been sent
    Init,

    /// Call is active.
    /// Traffic is sent either downlink, uplink, or both
    CallActive,

    /// Call still exists but transmission has ceased
    TxCeased,

    /// Call has been disconnected or released
    Disconnected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DlCallPdu {
    ///          ULSTREAM
    /// Response to:        U-SETUP
    /// Response expected:  -
    ///   Inform originating MS that call is proceeding and that connecting party has been alerted
    DAlert,

    ///          ULSTREAM
    /// Response to:        U-SETUP
    /// Response expected:  -
    ///   This PDU shall be the acknowledgement from the infrastructure to call set-up request
    ///   indicating that the call is proceeding.
    DCallProceeding,

    ///          ULSTREAM
    /// Response to:        U-CALL RESTORE
    /// Response expected:  -
    ///   This PDU shall indicate to the MS that a call has been restored after a temporary break
    ///   of the call. Used when transferring a call between cells.
    DCallRestore,

    ///          ULSTREAM
    // Response to:        U-SETUP
    // Response expected:  -
    //   This PDU shall be the order to the calling MS to through-connect.
    DConnect, // not used in downlink call

    /// DLSTREAM
    /// Response to:        U-CONNECT
    /// Response expected:  -
    ///   This PDU shall be the order to the called MS to through-connect.
    DConnectAck,

    /// TODO UNCLEAR CALLER / CALLEE
    /// Response to:        -
    /// Response expected:  U-RELEASE
    ///   This PDU shall be the disconnect request message sent from the infrastructure to
    ///   the MS.
    DDisconnect,

    /// TODO UNCLEAR CALLER / CALLEE
    /// Response to:        -
    /// Response expected:  -
    ///   This PDU shall be the general information message to the MS.
    DInfo,

    /// TODO UNCLEAR CALLER / CALLEE
    /// Response to:        - / U-DISCONNECT
    /// Response expected:  -
    ///   This PDU shall be a message from the infrastructure to the MS to inform that
    ///   the connection has been released.
    DRelease,

    /// DLSTREAM
    /// Response to:        -
    /// Response expected:  U-ALERT / U-CONNECT / -
    ///   This PDU shall be the call set-up message sent to the called MS.
    DSetup,

    /// DLSTREAM ULSTREAM? TODO CHECK
    /// Response to:        U-TX CEASED
    /// Response expected:  -
    ///   This PDU shall be the PDU from the SwMI to all MS within a call that a transmission
    ///   has ceased.
    DTxCeased,

    /// DLSTREAM ULSTREAM? TODO CHECK
    /// Response to:        -
    /// Response expected:  -
    ///   This PDU shall be the information from the SwMI to the MS that the interruption
    ///   of the call has ceased.
    DTxContinue,

    /// DLSTREAM ULSTREAM? TODO CHECK
    /// Response to:        U-TX DEMAND
    /// Response expected:  -
    ///   This PDU shall inform the MS concerned with a call that permission to transmit has
    ///   been granted by the SwMI to a MS, and to inform that MS that it has been granted
    ///   permission to transmit. This PDU shall also inform a MS that its request to transmit has
    ///   been rejected or queued.
    DTxGranted,

    /// DLSTREAM ULSTREAM? TODO CHECK
    /// Response to:        -
    /// Response expected:  -
    ///   This PDU shall be a message from the SwMI indicating that a permission to transmit
    //    has been withdrawn.
    DTxInterrupt,

    /// DLSTREAM ULSTREAM? TODO CHECK
    /// Response to:        U-TX DEMAND
    /// Response expected:  -
    ///   This PDU shall be a message from the SwMI that the call is being interrupted.
    DTxWait,
}

/// These UL PDUs can be expected in response to a downlink call
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UlCallPdu {
    /// DLSTREAM
    /// Response to:        D-SETUP
    /// Response expected:  -
    ///   This PDU shall be a acknowledgement from the called MS that the called user
    ///   has been alerted.
    UAlert,

    ///          ULSTREAM
    /// Response to:        -
    /// Response expected:  D-CALL RESTORE
    ///   This PDU shall be the order from the MS for restoration of a specific call after
    ///   a temporary break of the call. Used when transferring a call between cells.
    UCallRestore,

    /// DLSTREAM
    /// Response to:        D-SETUP
    /// Response expected:  D-CONNECT ACKNOWLEDGE
    ///   This PDU shall be the acknowledgement to the SwMI that the called MS is ready
    ///   for through-connection.
    UConnect,

    /// DLSTREAM ULSTREAM? TODO CHECK
    /// Response to:        -
    /// Response expected:  D-DISCONNECT/D-RELEASE
    ///   This PDU shall be the MS request to the SwMI to disconnect a call.
    UDisconnect,

    /// DLSTREAM ULSTREAM? TODO CHECK
    /// Response to:        -
    /// Response expected:  -
    ///   This PDU shall be the general information message from the MS.
    UInfo,

    /// DLSTREAM ULSTREAM? TODO CHECK
    /// Response to:        D-DISCONNECT
    /// Response expected:  -
    ///   This PDU shall be the acknowledgement to a disconnection.
    URelease,

    ///          ULSTREAM
    /// Response to:        -
    /// Response expected:  D-CALL PROCEEDING / D-ALERT / D-CONNECT
    ///
    USetup,

    ///          ULSTREAM
    /// Response to:        -
    /// Response expected:  D-TX CEASED / D-TX GRANTED / D-TX WAIT
    ///   This PDU shall be the message to the SwMI that a transmission has ceased.
    UTxCeased,

    ///          ULSTREAM
    /// Response to:        -
    /// Response expected:  D-TX GRANTED
    ///   This PDU shall be the message to the SwMI that a transmission is requested.
    UTxDemand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteEvent {
    NewCall,
    CallActive,
    TxCeased,
}

#[derive(Debug)]
struct Call {
    state: DlCallState,
    is_group: bool,
    is_acked: bool,
    current_caller_issi: u32,
    current_caller_is_local: bool,
    callee_ssi: u32,
    callee_is_local: bool,
    req_to_transmit_queue: VecDeque<u32>,
}

impl Call {
    pub fn new(
        current_caller_issi: u32,
        current_caller_is_local: bool,
        callee_ssi: u32,
        callee_is_local: bool,
        is_group: bool,
        is_acked: bool,
    ) -> Self {
        Self {
            state: DlCallState::Init,
            current_caller_issi,
            current_caller_is_local,
            callee_ssi,
            callee_is_local,
            is_group,
            is_acked,
            req_to_transmit_queue: VecDeque::new(),
        }
    }

    fn discard(&self, pdu: UlCallPdu) {
        tracing::warn!(
            "State: {:?} local calller {} callee {}, group: {}, acked: {}, discarding pdu: {:?}",
            self.state,
            self.current_caller_is_local,
            self.callee_is_local,
            self.is_group,
            self.is_acked,
            pdu
        );
    }

    fn handle_pdu(&mut self, pdu: UlCallPdu) {
        match self.state {
            DlCallState::Init => {
                // The call is not communicated to the participants, and we accept no incoming message
                match pdu {
                    UlCallPdu::UAlert
                    | UlCallPdu::UCallRestore
                    | UlCallPdu::UConnect
                    | UlCallPdu::UDisconnect
                    | UlCallPdu::UInfo
                    | UlCallPdu::UTxCeased
                    | UlCallPdu::UTxDemand
                    | UlCallPdu::URelease => self.discard(pdu),
                    UlCallPdu::USetup => {
                        if self.handle_u_setup(pdu) {
                            self.state = DlCallState::CallActive;
                        }
                    }
                }
            }
            DlCallState::CallActive => {
                // Call is ongoing. Tx has been granted
                match (
                    pdu,
                    self.current_caller_is_local,
                    self.callee_is_local,
                    self.is_group,
                    self.is_acked
                ) {
                    (UlCallPdu::USetup, _, _, _, _) |           // Invalid after setup
                    (UlCallPdu::URelease, _, _, _, _) |         // Only after D-DISCONNECT
                    (UlCallPdu::UInfo, _, _, false, _) |        // Invalid for direct call
                    (UlCallPdu::UInfo, _, _, _, false) |        // Invalid for unacked call
                    (UlCallPdu::UTxCeased, false, _, _, _) |    // Invalid for remote caller
                    (UlCallPdu::UAlert, _, _, true, _) |        // Invalid for group call
                    (UlCallPdu::UAlert, _, _, _, false) |       // Invalid for unacked call
                    (UlCallPdu::UConnect, _, _, true, _) => {   // Invalid for group call
                        self.discard(pdu);
                    }

                    (UlCallPdu::UTxCeased, true, _, _, _) => {
                        unimplemented_log!("{:?}", pdu);
                        // self.handle_u_tx_ceased();
                    },
                    (UlCallPdu::UTxDemand, _, _, _, _) => {
                        unimplemented_log!("{:?}", pdu);
                        // self.handle_u_tx_demand();
                    },
                    (UlCallPdu::UConnect, _, _, false, _) => {
                        unimplemented_log!("{:?}", pdu);
                        // self.handle_u_connect();
                    },
                    (UlCallPdu::UDisconnect, _, _, _, _) => {
                        unimplemented_log!("{:?}", pdu);
                        // self.handle_u_disconnect();
                    }

                    (UlCallPdu::UInfo, _, _, true, true) => {
                        unimplemented_log!("{:?}", pdu);
                    }
                    (UlCallPdu::UAlert, _, _, false, true) => {
                        unimplemented_log!("{:?}", pdu);
                    }
                    (UlCallPdu::UCallRestore, _, _, _, _) => {
                        unimplemented_log!("{:?}", pdu);
                    }
                }
            }
            DlCallState::TxCeased => {
                // Call is ongoing. Depending on local/remote state of caller/callee, we handle certain messages
                match (
                    pdu,
                    self.current_caller_is_local,
                    self.callee_is_local,
                    self.is_group,
                    self.is_acked
                ) {
                    (UlCallPdu::USetup, _, _, _, _) |           // Invalid after setup
                    (UlCallPdu::URelease, _, _, _, _) |         // Only after D-DISCONNECT
                    (UlCallPdu::UTxCeased, _, _, _, _) |        // Invalid when already in TxCeased
                    (UlCallPdu::UConnect, _, _, _, _) |         // Only during call setup
                    (UlCallPdu::UInfo, _, _, _, _) |            // Only during call setup
                    (UlCallPdu::UAlert, _, _, _, _) => {        // Only during call setup
                        tracing::warn!("State: {:?} local calller {} callee {}, group: {}, acked: {}, discarding pdu: {:?}",
                            self.state,
                            self.current_caller_is_local,
                            self.callee_is_local,
                            self.is_group,
                            self.is_acked,
                            pdu
                        );
                    }

                    (UlCallPdu::UTxDemand, _, _, _, _) => {
                        unimplemented_log!("{:?}", pdu);
                        // self.handle_u_tx_demand();
                    },

                    (UlCallPdu::UDisconnect, _, _, _, _) => {
                        unimplemented_log!("{:?}", pdu);
                        // self.handle_u_disconnect();
                    }

                    (UlCallPdu::UCallRestore, _, _, _, _) => {
                        unimplemented_log!("{:?}", pdu);
                        // self.handle_u_call_restore();
                    }

                }
            }
            DlCallState::Disconnected => {
                self.discard(pdu);
            }
        }
    }

    fn handle_remote_event(&mut self, event: RemoteEvent) {
        match (self.state, event) {
            (DlCallState::Init, RemoteEvent::NewCall) => {
                assert!(self.callee_is_local);
                assert!(self.current_caller_is_local == false);
                self.send_d_setup(self.callee_ssi);
                self.signal_umac_dl_circuit();
                self.state = DlCallState::CallActive;
            }

            (DlCallState::CallActive, RemoteEvent::CallActive) => {
                self.send_d_call_proceeding(self.current_caller_issi);
            }

            (DlCallState::CallActive, RemoteEvent::TxCeased) => {
                self.send_d_tx_ceased();
                self.state = DlCallState::TxCeased;
            }

            (DlCallState::TxCeased, RemoteEvent::CallActive) => {
                self.send_d_tx_continue();
                self.state = DlCallState::CallActive;
            }

            (_, _) => {
                unimplemented_log!("Unhandled remote event {:?} in state {:?}", event, self.state);
            }
        }
    }

    fn handle_u_setup(&mut self, _pdu: UlCallPdu) -> bool {
        tracing::info!("Handling U-SETUP in state {:?}", self.state);
        false
    }

    fn send_d_setup(&mut self, ssi: u32) {
        tracing::info!("Sending D-SETUP to SSI {}", ssi);
    }

    fn send_d_call_proceeding(&mut self, issi: u32) {
        tracing::info!("Sending D-CALL PROCEEDING to ISSI {}", issi);
    }

    fn send_d_connect(&mut self, issi: u32) {
        tracing::info!("Sending D-CONNECT to ISSI {}", issi);
    }

    fn send_d_tx_ceased(&mut self) {
        tracing::info!("Sending D-TX CEASED");
    }

    fn send_d_tx_continue(&mut self) {
        tracing::info!("Sending D-TX CONTINUE");
    }

    fn signal_umac_dl_circuit(&mut self) {
        tracing::info!("Signaling UMAC to allocate DL circuit");
    }
}

#[cfg(test)]
mod test {

    use super::*;

    use tetra_core::debug;

    #[test]
    fn test_dl_call_fsm() {
        debug::setup_logging_verbose();
        let mut call = Call::new(1001, false, 2001, true, true, false);
        call.handle_pdu(super::UlCallPdu::UAlert);
    }
}

// // ========= FSM-facing types =========

// #[derive(Debug, Clone, PartialEq, Eq)]
// pub enum Event {
//     /// On reception of U-SETUP
//     USetup,

//     /// Expected state: CallActive
//     UTxCeased,

//     /// Expected state: TxCeased
//     UTxDemandByOwner,

//     UTxDemandPrio,

//     UDisconnect,

//     /// Timer expired (identify which in real code)
//     TimerExpired(TimerId),
// }

// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// pub enum State {
//     New,
//     CallActive,
//     TxCeased,
//     Error,
// }

// #[derive(Debug, Clone, PartialEq, Eq)]
// pub enum Effect {
//     SendToOwner(Pdu),
//     SendToRecipients(Pdu),
//     StartTimer(TimerId, Duration),
//     CancelTimer(TimerId),

//     /// Internal scheduling (lets you chain internal events cleanly)
//     Raise(Event),

//     Warn(&'static str),
// }

// #[derive(Debug, Clone, PartialEq, Eq)]
// pub struct Transition {
//     pub next: State,
//     pub effects: Vec<Effect>,
// }

// #[derive(Debug, Clone, PartialEq, Eq)]
// pub struct Unexpected {
//     pub state: State,
//     pub event: Event,
// }

// // ========= Call context (your mutable in-memory model) =========

// #[derive(Debug, Clone)]
// pub struct Ctx {
//     pub call_id: CallId,
//     pub recipients: Vec<u32>, // placeholder
// }

// impl Ctx {
//     pub fn new(call_id: CallId) -> Self {
//         Self {
//             call_id,
//             recipients: Vec::new(),
//         }
//     }

//     pub fn init_from_u_setup(&mut self) {
//         // Populate call participants, bearer choices, etc.
//         // Keep this purely in-memory.
//         if self.recipients.is_empty() {
//             self.recipients.extend([1001, 1002, 1003]);
//         }
//     }
// }

// // ========= IO boundary =========

// pub trait Services {
//     fn send_to_owner(&mut self, pdu: Pdu);
//     fn send_to_recipients(&mut self, pdu: Pdu);
//     fn start_timer(&mut self, id: TimerId, dur: Duration);
//     fn cancel_timer(&mut self, id: TimerId);
// }

// // ========= Machine =========

// pub struct Machine {
//     pub state: State,
//     pub ctx: Ctx,
// }

// impl Machine {
//     pub fn new(ctx: Ctx) -> Self {
//         Self {
//             state: State::New,
//             ctx,
//         }
//     }

//     /// Pure state step: returns Effects; does not perform IO itself.
//     pub fn step(&mut self, event: Event) -> Result<Vec<Effect>, Unexpected> {
//         // Global transitions (ANY STATE rules) first.
//         if let Some(t) = self.global(event.clone()) {
//             self.state = t.next;
//             return Ok(t.effects);
//         }

//         let t = match (self.state, event) {
//             (State::New, Event::USetup) => handlers::new_state::new_u_setup(&mut self.ctx),

//             (State::CallActive, Event::UTxCeased) => {
//                 handlers::call_active::u_tx_ceased(&mut self.ctx)
//             }

//             (State::TxCeased, Event::UTxDemandByOwner) => {
//                 handlers::tx_ceased::u_tx_demand_by_owner(&mut self.ctx)
//             }

//             (s, e) => return Err(Unexpected { state: s, event: e }),
//         };

//         self.state = t.next;
//         Ok(t.effects)
//     }

//     fn global(&mut self, event: Event) -> Option<Transition> {
//         match event {
//             Event::UDisconnect => Some(handlers::any::u_disconnect(&mut self.ctx)),
//             Event::TimerExpired(id) => Some(handlers::any::timer_expired(&mut self.ctx, id)),
//             _ => None,
//         }
//     }

//     /// Full dispatch: runs the FSM and executes Effects via Services.
//     /// Also supports Effect::Raise for internal chaining.
//     pub fn dispatch(&mut self, event: Event, svc: &mut impl Services) -> Result<(), Unexpected> {
//         let mut q = VecDeque::new();
//         q.push_back(event);

//         while let Some(ev) = q.pop_front() {
//             let effects = self.step(ev)?;
//             for eff in effects {
//                 match eff {
//                     Effect::SendToOwner(pdu) => svc.send_to_owner(pdu),
//                     Effect::SendToRecipients(pdu) => svc.send_to_recipients(pdu),
//                     Effect::StartTimer(id, dur) => svc.start_timer(id, dur),
//                     Effect::CancelTimer(id) => svc.cancel_timer(id),
//                     Effect::Raise(e) => q.push_back(e),
//                     Effect::Warn(msg) => eprintln!("{msg}"),
//                 }
//             }
//         }
//         Ok(())
//     }
// }

// // ========= Transition handlers (keep the match-table clean) =========

// mod handlers {
//     use super::*;

//     pub mod new_state {
//         use super::*;

//         pub fn new_u_setup(ctx: &mut Ctx) -> Transition {
//             ctx.init_from_u_setup();

//             let call_id = ctx.call_id;

//             let mut effects = Vec::new();
//             // Spec-derived sends (examples per your comments)
//             effects.push(Effect::SendToOwner(Pdu::DCallProceeding { call_id }));
//             effects.push(Effect::SendToOwner(Pdu::DConnect { call_id }));
//             effects.push(Effect::SendToRecipients(Pdu::DSetup { call_id }));

//             // Example: guard timer for setup completion (optional)
//             effects.push(Effect::StartTimer(TimerId::SetupGuard, Duration::from_secs(10)));

//             // If "Setup" is purely transient, do not model it as a State.
//             Transition {
//                 next: State::CallActive,
//                 effects,
//             }
//         }
//     }

//     pub mod call_active {
//         use super::*;

//         pub fn u_tx_ceased(ctx: &mut Ctx) -> Transition {
//             let call_id = ctx.call_id;

//             let effects = vec![
//                 Effect::SendToRecipients(Pdu::DTxCeased { call_id }),
//                 Effect::SendToOwner(Pdu::DTxCeased { call_id }),
//             ];

//             Transition {
//                 next: State::TxCeased,
//                 effects,
//             }
//         }
//     }

//     pub mod tx_ceased {
//         use super::*;

//         pub fn u_tx_demand_by_owner(ctx: &mut Ctx) -> Transition {
//             let call_id = ctx.call_id;

//             let effects = vec![
//                 Effect::SendToRecipients(Pdu::DTxGranted { call_id }),
//                 Effect::SendToOwner(Pdu::DTxGranted { call_id }),
//             ];

//             Transition {
//                 next: State::CallActive,
//                 effects,
//             }
//         }
//     }

//     pub mod any {
//         use super::*;

//         pub fn u_disconnect(ctx: &mut Ctx) -> Transition {
//             let call_id = ctx.call_id;

//             let effects = vec![
//                 Effect::CancelTimer(TimerId::SetupGuard),
//                 Effect::SendToRecipients(Pdu::DDisconnect { call_id }),
//                 Effect::SendToOwner(Pdu::DDisconnect { call_id }),
//             ];

//             // In a full CC implementation, this would likely transition into a
//             // CALL DISCONNECT superstate/substate machine, not directly to New.
//             Transition {
//                 next: State::New,
//                 effects,
//             }
//         }

//         pub fn timer_expired(_ctx: &mut Ctx, id: TimerId) -> Transition {
//             match id {
//                 TimerId::SetupGuard => Transition {
//                     next: State::Error,
//                     effects: vec![Effect::Warn("Setup guard timer expired")],
//                 },
//             }
//         }
//     }
// }

// // ========= Example Services implementation (recording mock) =========

// #[derive(Default)]
// pub struct RecordingServices {
//     pub sent_to_owner: Vec<Pdu>,
//     pub sent_to_recipients: Vec<Pdu>,
//     pub timers_started: Vec<(TimerId, Duration)>,
//     pub timers_cancelled: Vec<TimerId>,
// }

// impl Services for RecordingServices {
//     fn send_to_owner(&mut self, pdu: Pdu) {
//         self.sent_to_owner.push(pdu);
//     }
//     fn send_to_recipients(&mut self, pdu: Pdu) {
//         self.sent_to_recipients.push(pdu);
//     }
//     fn start_timer(&mut self, id: TimerId, dur: Duration) {
//         self.timers_started.push((id, dur));
//     }
//     fn cancel_timer(&mut self, id: TimerId) {
//         self.timers_cancelled.push(id);
//     }
// }

// // ========= Demo main =========

// fn main() {
//     let ctx = Ctx::new(CallId(42));
//     let mut m = Machine::new(ctx);
//     let mut svc = RecordingServices::default();

//     m.dispatch(Event::USetup, &mut svc).unwrap();
//     assert_eq!(m.state, State::CallActive);

//     m.dispatch(Event::UTxCeased, &mut svc).unwrap();
//     assert_eq!(m.state, State::TxCeased);

//     m.dispatch(Event::UTxDemandByOwner, &mut svc).unwrap();
//     assert_eq!(m.state, State::CallActive);

//     m.dispatch(Event::UDisconnect, &mut svc).unwrap();
//     assert_eq!(m.state, State::New);
// }

// // ========= Tests =========

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn setup_emits_expected_effects_and_enters_call_active() {
//         let ctx = Ctx::new(CallId(1));
//         let mut m = Machine::new(ctx);
//         let mut svc = RecordingServices::default();

//         m.dispatch(Event::USetup, &mut svc).unwrap();

//         assert_eq!(m.state, State::CallActive);

//         assert_eq!(
//             svc.sent_to_owner,
//             vec![
//                 Pdu::DCallProceeding { call_id: CallId(1) },
//                 Pdu::DConnect { call_id: CallId(1) },
//             ]
//         );
//         assert_eq!(
//             svc.sent_to_recipients,
//             vec![Pdu::DSetup { call_id: CallId(1) }]
//         );
//         assert_eq!(
//             svc.timers_started,
//             vec![(TimerId::SetupGuard, Duration::from_secs(10))]
//         );
//     }

//     #[test]
//     fn unexpected_event_returns_error_does_not_mutate_state() {
//         let ctx = Ctx::new(CallId(2));
//         let mut m = Machine::new(ctx);

//         let err = m.step(Event::UTxCeased).unwrap_err();
//         assert_eq!(err.state, State::New);
//         assert_eq!(err.event, Event::UTxCeased);
//         assert_eq!(m.state, State::New);
//     }

//     #[test]
//     fn disconnect_is_global_any_state() {
//         let ctx = Ctx::new(CallId(3));
//         let mut m = Machine::new(ctx);
//         let mut svc = RecordingServices::default();

//         m.dispatch(Event::USetup, &mut svc).unwrap();
//         assert_eq!(m.state, State::CallActive);

//         m.dispatch(Event::UDisconnect, &mut svc).unwrap();
//         assert_eq!(m.state, State::New);

//         assert_eq!(
//             svc.sent_to_owner.last().cloned(),
//             Some(Pdu::DDisconnect { call_id: CallId(3) })
//         );
//     }
// }
