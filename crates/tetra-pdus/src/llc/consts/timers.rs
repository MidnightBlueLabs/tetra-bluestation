use tetra_core::frames;

// Timers as defined in Annex A.1 LLC timers
pub const T251_SENDER_RETRY_TIMER: u32 = frames!(4); // 4 signalling frames
pub const T252_ACK_WAITING_TIMER: u32 = frames!(9);
pub const T261_SETUP_WAITING_TIMER: u32 = frames!(4);
pub const T263_DISCONNECT_WAITING_TIMER: u32 = frames!(4);
pub const T265_RECONNECT_WAITING_TIMER: u32 = frames!(4);
pub const T271_RECEIVER_NOT_READY_FOR_TX_TIMER: u32 = frames!(36);
pub const T272_RECEIVER_NOT_READY_FOR_RX_TIMER: u32 = frames!(18);
