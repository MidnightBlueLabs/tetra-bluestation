use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

/// The three states a transmit receipt can be in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TxState {
    /// Message is queued but not yet sent over the air.
    Pending = 0,
    /// MAC layer had to discard this message
    Discarded = 1,
    /// MAC layer has sent the PDU over the air.
    Transmitted = 2,
    /// Message was transmitted but acknowledgement never came
    Lost = 3,
    /// The remote side has acknowledged receipt.
    Acknowledged = 4,
}

impl TxState {
    fn from_raw(v: u8) -> Self {
        match v {
            0 => Self::Pending,
            1 => Self::Discarded,
            2 => Self::Transmitted,
            3 => Self::Lost,
            _ => Self::Acknowledged,
        }
    }
}

/// A transmit receipt kept by the originator (e.g. CMCE) to query whether the
/// message was sent and/or acknowledged.
///
/// State machine (transitions driven by the paired [`TxSignal`]):
///
/// ```text
/// Pending -> Transmitted | Discarded
///   Transmitted: MAC has sent the PDU over the air.
///   Discarded:   MAC was too busy. Final state.
///
/// expects_ack == true:
///   Transmitted -> Acknowledged | Lost
///     Acknowledged: LLC received ACK from remote. Final state.
///     Lost:         LLC timed out waiting for ACK. Final state.
///
/// expects_ack == false:
///   Transmitted is the final state.
/// ```
#[derive(Clone, Debug)]
pub struct TxReceipt {
    expects_ack: bool,
    state: Arc<AtomicU8>,
}

impl TxReceipt {
    /// Creates a linked `(TxReceipt, TxReporter)` pair.
    /// The receipt stays with the originator; the signal travels down the stack.
    pub fn new(expects_ack: bool) -> (Self, TxReporter) {
        let state = Arc::new(AtomicU8::new(TxState::Pending as u8));
        (
            Self {
                expects_ack,
                state: state.clone(),
            },
            TxReporter { expects_ack, state },
        )
    }

    /// Returns the current state of the receipt.
    pub fn get_state(&self) -> TxState {
        TxState::from_raw(self.state.load(Ordering::Relaxed))
    }

    /// True once the PDU has been sent over the air (or further).
    pub fn is_transmitted(&self) -> bool {
        self.state.load(Ordering::Relaxed) >= TxState::Transmitted as u8
    }

    /// True once the remote side has acknowledged receipt.
    pub fn is_acknowledged(&self) -> bool {
        self.state.load(Ordering::Relaxed) >= TxState::Acknowledged as u8
    }

    /// Returns true if this is the final state for this message.
    pub fn is_in_final_state(&self) -> bool {
        match self.get_state() {
            TxState::Pending => false,
            TxState::Discarded => true,
            TxState::Transmitted => !self.expects_ack,
            TxState::Lost => true,
            TxState::Acknowledged => true,
        }
    }
}

/// The reporting half of a transmit receipt, carried alongside the PDU down
/// through MAC and LLC. These layers call the `mark_*` methods to drive state
/// transitions that the paired [`TxReceipt`] can observe.
#[derive(Clone, Debug)]
pub struct TxReporter {
    expects_ack: bool,
    state: Arc<AtomicU8>,
}

impl TxReporter {
    /// Returns the current state.
    pub fn get_state(&self) -> TxState {
        TxState::from_raw(self.state.load(Ordering::Relaxed))
    }

    fn mark(&self, curr_state: TxState, new_state: TxState) {
        match self
            .state
            .compare_exchange(curr_state as u8, new_state as u8, Ordering::Relaxed, Ordering::Relaxed)
        {
            Ok(_) => {}
            Err(_) => {
                panic!(
                    "TxReporter: invalid transition {:?} -> {:?} (actual state: {:?})",
                    curr_state,
                    new_state,
                    self.get_state()
                );
            }
        }
    }

    /// Pending → Transmitted: MAC layer has sent the PDU over the air.
    pub fn mark_transmitted(&self) {
        tracing::info!("TxReporter: marking as transmitted");
        self.mark(TxState::Pending, TxState::Transmitted);
    }

    /// Pending → Discarded: MAC layer was too busy to transmit.
    pub fn mark_discarded(&self) {
        self.mark(TxState::Pending, TxState::Discarded);
    }

    /// Transmitted → Acknowledged: LLC received an ACK from the remote side.
    pub fn mark_acknowledged(&self) {
        assert!(
            self.expects_ack,
            "TxReporter: cannot mark as acknowledged a message that does not expect an ACK"
        );
        self.mark(TxState::Transmitted, TxState::Acknowledged);
    }

    /// Transmitted → Lost: LLC did not receive an ACK within the expected time window.
    pub fn mark_lost(&self) {
        assert!(
            self.expects_ack,
            "TxReporter: cannot mark as lost a message that does not expect an ACK"
        );
        self.mark(TxState::Transmitted, TxState::Lost);
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receipt_observes_signal_transitions() {
        let (receipt, reporter) = TxReceipt::new(true);
        assert_eq!(reporter.get_state(), TxState::Pending);
        reporter.mark_transmitted();
        assert_eq!(receipt.get_state(), TxState::Transmitted);
        reporter.mark_acknowledged();
        assert_eq!(receipt.get_state(), TxState::Acknowledged);
    }

    #[test]
    fn cloned_signal_shares_state() {
        let (receipt, reporter) = TxReceipt::new(false);
        let signal2 = reporter.clone();
        signal2.mark_transmitted();
        assert_eq!(receipt.get_state(), TxState::Transmitted);
        assert_eq!(reporter.get_state(), TxState::Transmitted);
    }

    #[test]
    #[should_panic(expected = "invalid transition")]
    fn double_mark_transmitted_panics() {
        let (_receipt, reporter) = TxReceipt::new(false);
        reporter.mark_transmitted();
        reporter.mark_transmitted();
    }

    #[test]
    #[should_panic(expected = "invalid transition")]
    fn mark_acknowledged_from_pending_panics() {
        let (_receipt, reporter) = TxReceipt::new(true);
        reporter.mark_acknowledged(); // must be Transmitted first
    }
}
