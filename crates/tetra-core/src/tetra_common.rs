// Placeholder type
pub type Todo = i32;

// SAPs as defined in the standard
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Sap {
    TpSap,  // Phy/LMAC
    TpcSap, // Phy/LMAC mgmt

    /// LMAC/UMAC
    TmvSap,

    /// UMAC/LLC
    TmaSap,
    // TmbSap, // UMAC/ TLB-SAP, broadcast, merged with TLB-SAP in TLMB-SAP
    // TmcSap, // UMAC/ TLC-SAP, mgmt, merged with TLC-SAP in TLMC-SAP
    TmdSap, // Uplane

    /// LLC/MLE
    TlaSap,
    /// LLC/MLE broadcast, merged TMB-SAP and TLB-SAP
    TlmbSap,
    /// LLC/MLE mgmt, merged TMC-SAP and TLC-SAP
    TlmcSap,

    /// MLE/MM
    LmmSap,
    /// MLE/CMCE
    LcmcSap,

    /// MS CMCE -> User
    TnccSap,
    /// MS CMCE -> User
    TnssSap,
    /// MS CMCE -> User
    TnsdsSap,

    /// MLE/SNDCP
    TlpdSap,

    /// MM -> User
    TnmmSap,

    /// Custom SAP for inter-entity control messages
    Control,
}
