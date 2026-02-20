pub const HALFSLOT_TYPE4_BITS: usize = 255; // TODO FIXME check if this is indeed type4
pub const TIMESLOT_TYPE4_BITS: usize = 255 * 2; // TODO FIXME check if this is indeed type4

pub const SEQ_SYNC_OFFSET: usize = 214;
pub const SEQ_NORM_DL_OFFSET: usize = 244;
pub const SEQ_NORM_UL_OFFSET: usize = 254;
pub const SEQ_EXT_OFFSET_SSB1: usize = 122;
pub const SEQ_EXT_OFFSET_SSB2: usize = 122 + HALFSLOT_TYPE4_BITS;

/* 9.4.4.3.2 Normal Training Sequence */
/// 22 n-bits
pub const SEQ_NORM1_AS_ARR: [u8; 22] = [1, 1, 0, 1, 0, 0, 0, 0, 1, 1, 1, 0, 1, 0, 0, 1, 1, 1, 0, 1, 0, 0];
/// 22 p-bits
pub const SEQ_NORM2_AS_ARR: [u8; 22] = [0, 1, 1, 1, 1, 0, 1, 0, 0, 1, 0, 0, 0, 0, 1, 1, 0, 1, 1, 1, 1, 0];
/// 22 q-bits
pub const SEQ_NORM3_AS_ARR: [u8; 22] = [1, 0, 1, 1, 0, 1, 1, 1, 0, 0, 0, 0, 0, 1, 1, 0, 1, 0, 1, 1, 0, 1];
/// 30 x-bits
pub const SEQ_EXT_AS_ARR: [u8; 30] = [
    1, 0, 0, 1, 1, 1, 0, 1, 0, 0, 0, 0, 1, 1, 1, 0, 1, 0, 0, 1, 1, 1, 0, 1, 0, 0, 0, 0, 1, 1,
];
/// 38 y-bits
pub const SEQ_SYNC_AS_ARR: [u8; 38] = [
    1, 1, 0, 0, 0, 0, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 1, 1, 1, 0, 1, 0, 0, 1, 1, 1, 0, 0, 0, 0, 0, 1, 1, 0, 0, 1, 1, 1,
];

pub const SEQ_NORM1: u64 = 0b1101000011101001110100;
pub const SEQ_NORM2: u64 = 0b0111101001000011011110;
pub const SEQ_NORM3: u64 = 0b1011011100000110101101;
pub const SEQ_NORM_LEN: usize = 22;

// /* 9.4.4.3.3 Extended training sequence */
pub const SEQ_EXT: u64 = 0b100111010000111010011101000011; // 30 bits
pub const SEQ_EXT_LEN: usize = 30;

// /* 9.4.4.3.4 Synchronization training sequence */
pub const SEQ_SYNC: u64 = 0b11000001100111001110100111000001100111; // 38 bits
pub const SEQ_SYNC_LEN: usize = 38;

/* 9.4.4.3.5 Tail bits */
pub const T_BITS: u64 = 0b1100;
