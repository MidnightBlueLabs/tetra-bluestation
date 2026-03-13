#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SdsUserData {
    /// Type field 0, 16 bits, short_data_type_identifier == 0
    Type1(u16),
    /// Type field 1, 32 bits, short_data_type_identifier == 1
    Type2(u32),
    /// Type field 2, 64 bits, short_data_type_identifier == 2
    Type3(u64),
    /// Type field 3, variable length, short_data_type_identifier == 3
    Type4(u16, Vec<u8>),
}

impl SdsUserData {
    pub fn type_identifier(&self) -> u8 {
        match self {
            SdsUserData::Type1(_) => 0,
            SdsUserData::Type2(_) => 1,
            SdsUserData::Type3(_) => 2,
            SdsUserData::Type4(_, _) => 3,
        }
    }

    pub fn length_bits(&self) -> u16 {
        match self {
            SdsUserData::Type1(_) => 16,
            SdsUserData::Type2(_) => 32,
            SdsUserData::Type3(_) => 64,
            SdsUserData::Type4(len_bits, _) => *len_bits,
        }
    }

    pub fn to_arr(&self) -> Vec<u8> {
        match self {
            SdsUserData::Type1(value) => value.to_be_bytes().to_vec(),
            SdsUserData::Type2(value) => value.to_be_bytes().to_vec(),
            SdsUserData::Type3(value) => value.to_be_bytes().to_vec(),
            SdsUserData::Type4(_, data) => data.clone(),
        }
    }
}
