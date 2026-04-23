use core::fmt;

use tetra_core::expect_pdu_type;
use tetra_core::{BitBuffer, pdu_parse_error::PduParseErr};

use crate::mm::enums::mm_pdu_type_dl::MmPduTypeDl;
use crate::mm::enums::status_downlink::StatusDownlink;
use crate::mm::fields::energy_saving_information::EnergySavingInformation;

/// Representation of the D-MM STATUS PDU (Clause 16.9.2.5.1).
/// The infrastructure sends this message to the MS to request or indicate/reject a change of an operation mode.
/// Response expected: -/U-MM STATUS
/// Response to: -/U-MM STATUS

// note 1: This information element shall indicate the requested service or a response to a request and the sub-type of the D-MM STATUS PDU.
// note 2: This information element or set of information elements shall be as defined by the status downlink information element, refer to clauses 16.9.2.5.1 to 16.9.2.5.7.
// note 3: This Status downlink element indicates which sub-PDU this D-MM STATUS PDU contains. If the receiving party does not support the indicated function but recognizes the PDU structure, it should set the value to Not-supported sub-PDU type element.
#[derive(Debug)]
pub struct DMmStatus {
    /// Type1, 6 bits, See notes 1 and 3,
    pub status_downlink: StatusDownlink,
    /// Energy saving information, present for ChangeOfEnergySavingModeRequest/Response
    pub energy_saving_information: Option<EnergySavingInformation>,
}

impl DMmStatus {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(4, "pdu_type")?;
        expect_pdu_type!(pdu_type, MmPduTypeDl::DMmStatus)?;

        // Type1
        let val = buffer.read_field(6, "status_downlink")?;
        let status_downlink = StatusDownlink::try_from(val).map_err(|_| PduParseErr::InvalidValue {
            field: "status_downlink",
            value: val,
        })?;

        let energy_saving_information = match status_downlink {
            StatusDownlink::ChangeOfEnergySavingModeRequest | StatusDownlink::ChangeOfEnergySavingModeResponse => {
                Some(EnergySavingInformation::from_bitbuf(buffer)?)
            }
            _ => {
                unimplemented!("D-MM-STATUS sub-PDU parsing for {:?}", status_downlink);
            }
        };

        Ok(DMmStatus {
            status_downlink,
            energy_saving_information,
        })
    }

    /// Serialize this PDU into the given BitBuffer.
    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        // PDU Type
        buffer.write_bits(MmPduTypeDl::DMmStatus.into_raw(), 4);
        // Type1
        buffer.write_bits(self.status_downlink.into_raw(), 6);

        match self.status_downlink {
            StatusDownlink::ChangeOfEnergySavingModeRequest | StatusDownlink::ChangeOfEnergySavingModeResponse => {
                if let Some(ref esi) = self.energy_saving_information {
                    esi.to_bitbuf(buffer)?;
                } else {
                    return Err(PduParseErr::FieldNotPresent {
                        field: Some("energy_saving_information"),
                    });
                }
            }
            _ => {
                unimplemented!("D-MM-STATUS sub-PDU serialization for {:?}", self.status_downlink);
            }
        }

        Ok(())
    }
}

impl fmt::Display for DMmStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "DMmStatus {{ status_downlink: {} energy_saving_information: {:?} }}",
            self.status_downlink, self.energy_saving_information,
        )
    }
}
