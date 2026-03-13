use crate::control::enums::sds_user_data::SdsUserData;

/// SDS data routing between CMCE SDS subentity and Brew entity
#[derive(Debug, Clone)]
pub struct CmceSdsData {
    /// Source ISSI (calling party)
    pub source_issi: u32,
    /// Destination ISSI (called party)
    pub dest_issi: u32,
    /// User-defined data (type1, type2, type3, or type4)
    pub user_defined_data: SdsUserData,
}
