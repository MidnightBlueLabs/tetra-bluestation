use core::fmt;

use tetra_core::expect_pdu_type;
use tetra_core::typed_pdu_fields::*;
use tetra_core::{BitBuffer, pdu_parse_error::PduParseErr};

use crate::mm::enums::mm_pdu_type_dl::MmPduTypeDl;
use crate::mm::enums::type34_elem_id_dl::MmType34ElemIdDl;

/// Representation of the D-LOCATION UPDATE REJECT PDU (Clause 16.9.2.9).
/// The infrastructure sends this message to the MS to indicate that updating in the network is not accepted.
/// Response expected: -
/// Response to: U-LOCATION UPDATE DEMAND

// note 1: Information element "Ciphering parameters" is not present if "Cipher control" is set to "0", "ciphering off".
// note 2: Information element "Ciphering parameters" is present if "Cipher control" is set to "1", "ciphering on".
#[derive(Debug)]
pub struct DLocationUpdateReject {
    /// Type1, 3 bits, Location update type
    pub location_update_type: u8,
    /// Type1, 5 bits, Reject cause
    pub reject_cause: u8,
    /// Type1, 1 bits, Cipher control
    pub cipher_control: bool,
    /// Conditional 10 bits, See note,
    pub ciphering_parameters: Option<u64>,
    /// Type2, 24 bits, MNI of the MS,
    pub address_extension: Option<u64>,
    /// Type3, Cell type control
    pub cell_type_control: Option<Type3FieldGeneric>,
    /// Type3, Proprietary
    pub proprietary: Option<Type3FieldGeneric>,
}

#[allow(unreachable_code)] // TODO FIXME review, finalize and remove this
#[allow(unused_variables)]
impl DLocationUpdateReject {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(4, "pdu_type")?;
        expect_pdu_type!(pdu_type, MmPduTypeDl::DLocationUpdateReject)?;

        // Type1
        let location_update_type = buffer.read_field(3, "location_update_type")? as u8;
        // Type1
        let reject_cause = buffer.read_field(5, "reject_cause")? as u8;
        // Type1
        let cipher_control = buffer.read_field(1, "cipher_control")? != 0;
        // Conditional
        unimplemented!();
        let ciphering_parameters = if true { Some(0) } else { None };

        // obit designates presence of any further type2, type3 or type4 fields
        let mut obit = delimiters::read_obit(buffer)?;

        // Type2
        let address_extension = typed::parse_type2_generic(obit, buffer, 24, "address_extension")?;

        // Type3
        let cell_type_control = typed::parse_type3_generic(obit, buffer, MmType34ElemIdDl::CellTypeControl)?;

        // Type3
        let proprietary = typed::parse_type3_generic(obit, buffer, MmType34ElemIdDl::Proprietary)?;
        // Read trailing mbit (if not previously encountered)
        obit = if obit { buffer.read_field(1, "trailing_obit")? == 1 } else { obit };
        if obit {
            return Err(PduParseErr::InvalidTrailingMbitValue);
        }

        Ok(DLocationUpdateReject {
            location_update_type,
            reject_cause,
            cipher_control,
            ciphering_parameters,
            address_extension,
            cell_type_control,
            proprietary,
        })
    }

    /// Serialize this PDU into the given BitBuffer.
    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        // PDU Type
        buffer.write_bits(MmPduTypeDl::DLocationUpdateReject.into_raw(), 4);
        // Type1
        buffer.write_bits(self.location_update_type as u64, 3);
        // Type1
        buffer.write_bits(self.reject_cause as u64, 5);
        // Type1
        buffer.write_bits(self.cipher_control as u64, 1);
        // Conditional
        if let Some(ref value) = self.ciphering_parameters {
            buffer.write_bits(*value, 10);
        }

        // Check if any optional field present and place o-bit
        let obit = self.address_extension.is_some() || self.cell_type_control.is_some() || self.proprietary.is_some();
        delimiters::write_obit(buffer, obit as u8);
        if !obit {
            return Ok(());
        }

        // Type2
        typed::write_type2_generic(obit, buffer, self.address_extension, 24);

        // Type3
        typed::write_type3_generic(obit, buffer, &self.cell_type_control, MmType34ElemIdDl::CellTypeControl)?;

        // Type3
        typed::write_type3_generic(obit, buffer, &self.proprietary, MmType34ElemIdDl::Proprietary)?;

        // Write terminating m-bit
        delimiters::write_mbit(buffer, 0);
        Ok(())
    }
}

impl fmt::Display for DLocationUpdateReject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "DLocationUpdateReject {{ location_update_type: {:?} reject_cause: {:?} cipher_control: {:?} ciphering_parameters: {:?} address_extension: {:?} cell_type_control: {:?} proprietary: {:?} }}",
            self.location_update_type,
            self.reject_cause,
            self.cipher_control,
            self.ciphering_parameters,
            self.address_extension,
            self.cell_type_control,
            self.proprietary,
        )
    }
}
