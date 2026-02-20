pub const DQPSK4_BITS_PER_SYM: usize = 2;

pub const SB_BITS: usize = (6 + 1 + 40 + 60 + 19 + 15 + 108 + 1 + 5) * DQPSK4_BITS_PER_SYM;
pub const SB_BLK1_OFFSET: usize = (6 + 1 + 40) * DQPSK4_BITS_PER_SYM;
pub const SB_BBK_OFFSET: usize = (6 + 1 + 40 + 60 + 19) * DQPSK4_BITS_PER_SYM;
pub const SB_BLK2_OFFSET: usize = (6 + 1 + 40 + 60 + 19 + 15) * DQPSK4_BITS_PER_SYM;

pub const SB_BLK1_BITS: usize = 60 * DQPSK4_BITS_PER_SYM;
pub const SB_BBK_BITS: usize = 15 * DQPSK4_BITS_PER_SYM;
pub const SB_BLK2_BITS: usize = 108 * DQPSK4_BITS_PER_SYM;

pub const NDB_BITS: usize = (5 + 1 + 1 + 108 + 7 + 11 + 8 + 108 + 1 + 5) * DQPSK4_BITS_PER_SYM;
pub const NDB_BLK1_OFFSET: usize = (5 + 1 + 1) * DQPSK4_BITS_PER_SYM;
pub const NDB_BBK1_OFFSET: usize = (5 + 1 + 1 + 108) * DQPSK4_BITS_PER_SYM;
pub const NDB_BBK2_OFFSET: usize = (5 + 1 + 1 + 108 + 7 + 11) * DQPSK4_BITS_PER_SYM;
pub const NDB_BLK2_OFFSET: usize = (5 + 1 + 1 + 108 + 7 + 11 + 8) * DQPSK4_BITS_PER_SYM;

pub const NDB_BBK1_BITS: usize = 7 * DQPSK4_BITS_PER_SYM;
pub const NDB_BBK2_BITS: usize = 8 * DQPSK4_BITS_PER_SYM;
pub const NDB_BLK_BITS: usize = 108 * DQPSK4_BITS_PER_SYM;
pub const NDB_BBK_BITS: usize = SB_BBK_BITS;

pub const CUB_BITS: usize = 4 + 84 + 30 + 84 + 4;
pub const CUB_BLK_BITS: usize = 84;
pub const CUB_HEADBITS_OFFSET: usize = 34;
pub const CUB_BLK1_OFFSET: usize = 4;
pub const CUB_TRAINING_OFFSET: usize = 4 + 84;
pub const CUB_BLK2_OFFSET: usize = 4 + 84 + 30;
pub const CUB_TAILBITS_OFFSET: usize = 4 + 84 + 30 + 84;
pub const CUB_BURST_BITS: usize = CUB_TAILBITS_OFFSET + 4;

pub const NUB_BITS: usize = 4 + 216 + 22 + 216 + 4;
pub const NUB_BLK_BITS: usize = 216;
pub const NUB_HEADBITS_OFFSET: usize = 34;
pub const NUB_BLK1_OFFSET: usize = 4;
pub const NUB_TRAINING_OFFSET: usize = 4 + 216;
pub const NUB_BLK2_OFFSET: usize = 4 + 216 + 22;
pub const NUB_TAILBITS_OFFSET: usize = 4 + 216 + 22 + 216;
pub const NUB_BURST_BITS: usize = NUB_TAILBITS_OFFSET + 4;
