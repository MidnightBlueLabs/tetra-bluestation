#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalChannel {
    Tp,
    Cp,
    Unallocated,
}

/// The endpoint identifiers between the MLE and LLC, and between the LLC and MAC, refer to the MAC resource that is
/// currently used for that service. These identifiers may be local. There shall be a unique correspondence between the
/// endpoint identifier and the physical allocation (timeslot or timeslots) used in the MAC. (This correspondence is known
/// only within the MAC.) More than one advanced link may use one MAC resource.
/// In the current implementation, the endpoint_id is just the timeslot number used by the MAC.
pub type EndpointId = u32;

pub type LinkId = u32;

/// Handle assigned by MLE to primitives for MM/CMCE/SNDCP
pub type MleHandle = u32;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Layer2Service {
    /// TODO FIXME, remove this option once all Layer2Service uses have been checked to have the right type
    /// Behavior defaults to Acknowledged type
    Todo,
    /// Use acknowledged BL-DATA (or BL-ADATA) service
    Acknowledged,
    /// Use unacknowledged BL-UDATA service
    Unacknowledged,
}
