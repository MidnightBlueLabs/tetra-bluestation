// Numbers as defined in Annex A.2 LLC constants

///  This is the maximum length of one TL-SDU if the optional Frame Check Sequence (FCS) is used.
///  Default value = 2 595 bits (i.e. approximately 324 octets).
///  The FCS is optional. If the FCS is not used, the TL-SDU part may be larger by four octets.
pub const N251_BL_MAX_TLSDU_LEN_BITS: u32 = 2595;

/// MS designer choice from range 1 to 5 if the stealing repeats flag is not set.
pub const N252_BL_MAX_TLSDU_RETRANSMITS_ACKED: u32 = 3;

/// MS designer choice from range 3 to 5 if the stealing repeats flag is set.
pub const N252_BL_MAX_TLSDU_RETRANSMITS_ACKED_STEALING_REPEATS: u32 = 3;

/// MS designer choice from range 0 to 5.
/// NOTE 1: The service user may indicate the required number of TL-SDU repetitions for a particular TL-SDU in the
/// unacknowledged basic link service. The value of N.253 chosen by the MS designer applies when the
/// service user does not indicate the required number of repetitions.
pub const N253_BL_MAX_TLSDU_REPETITIONS_UNACKED: u32 = 3;

/// MS designer choice from range 1 to 5.
pub const N262_AL_MAX_CONNECTION_SETUP_RETRIES: u32 = 3;

/// MS designer choice from range 3 to 5.
pub const N263_AL_MAX_DISCONNECTION_RETRIES: u32 = 3;

/// This value may be defined during the set-up of the advanced link (see AL-SETUP definition). Range: 1 to 4.
pub const N264_AL_NUM_DQPSK_TIMESLOTS: u32 = 4;

/// MS designer choice from range 0 to 5.
pub const N265_AL_MAX_RECONNECTION_RETRIES: u32 = 3;

/// This is the maximum length of one TL-SDU including the FCS, it is defined during the set-up of the advanced
/// link (see AL-SETUP PDU definition), Range: (32, 4 096) octets.
pub const N271_AL_MAX_TLSDU_LEN: u32 = 4096;

/// This value is defined during the set-up of the advanced link, (see AL-SETUP definition).
///  Range: (1;3) for the original advanced link.
///  Range: (1;15) for an extended advanced link.
pub const N272_AL_WINDOW_SIZE_TLSDU_ACKED: u32 = 3;

/// This value is defined during the set-up of the advanced link (see AL-SETUP definition). Range: (0;7).
pub const N273_AL_MAX_TLSDU_RETRANSMISSIONS: u32 = 3;

/// This value is defined during the set-up of the advanced link, (see AL-SETUP definition). Range: (0;15).
pub const N274_AL_MAX_SEGMENT_RETRANSMISSIONS: u32 = 3;

/// This value is defined during the set-up of the advanced link (see AL-SETUP definition).
/// Range: (1;3) for the original advanced link.
/// Range: (1;15) for an extended advanced link.
pub const N281_AL_WINDOW_SIZE_TLSDU_UNACKED: u32 = 3;

/// This value is defined during the set-up of the advanced link (see AL-SETUP definition). Range: (0;7).
pub const N282_AL_NUM_REPETITIONS_UNACKED: u32 = 3;

/// MS designer choice from range 0 to 5.
/// NOTE 2: The MAC may indicate the required number of repetitions of a particular layer 2 signalling PDU. The
/// value of N.293 chosen by the MS designer applies when the MAC does not indicate the required number
/// of repetitions.
/// NOTE 3: It is recommended that N.293 is set to 0 in most cases.
pub const N293_AL_NUM_REPETITIONS_LAYER2_SIGNALLING_PDU: u32 = 3;
