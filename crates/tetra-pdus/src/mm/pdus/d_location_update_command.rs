use core::fmt;

use tetra_core::expect_pdu_type;
use tetra_core::typed_pdu_fields::*;
use tetra_core::{BitBuffer, pdu_parse_error::PduParseErr};

use crate::mm::enums::mm_pdu_type_dl::MmPduTypeDl;

/// Representation of the D-LOCATION UPDATE COMMAND PDU (Clause 16.9.2.8).
/// The infrastructure sends this message to the MS to initiate a location update demand in the MS.
/// Response expected: U-LOCATION UPDATE DEMAND
/// Response to: -

// note 1: Ciphering parameters element is not present if Cipher control is set to ‘0’ and is present if set to ‘1’.
#[derive(Debug)]
pub struct DLocationUpdateCommand {
    /// Type1, 1 bits, Group identity report
    pub group_identity_report: bool,
    /// Type1, 1 bits, Cipher control
    pub cipher_control: bool,
    /// Conditional 10 bits, Conditional: present only if Cipher control = 1 (on); absent if Cipher control = 0 (off),
    pub ciphering_parameters: Option<u64>,
    /// Type2, 24 bits, MNI of the MS,
    pub address_extension: Option<u64>,
    /// Conditional 3 bits, Cell type control
    pub cell_type_control: Option<u64>,
    /// Conditional 3 bits, Proprietary
    pub proprietary: Option<u64>,
}

#[allow(unreachable_code)] // TODO FIXME review, finalize and remove this
#[allow(unused_variables)]
impl DLocationUpdateCommand {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(4, "pdu_type")?;
        expect_pdu_type!(pdu_type, MmPduTypeDl::DLocationUpdateCommand)?;

        // Type1
        let group_identity_report = buffer.read_field(1, "group_identity_report")? != 0;
        // Type1
        let cipher_control = buffer.read_field(1, "cipher_control")? != 0;
        // Conditional
        unimplemented!();
        let ciphering_parameters = if true { Some(0) } else { None };

        // obit designates presence of any further type2, type3 or type4 fields
        let mut obit = delimiters::read_obit(buffer)?;

        // Type2
        let address_extension = typed::parse_type2_generic(obit, buffer, 24, "address_extension")?;
        // Conditional
        unimplemented!();
        let cell_type_control = if obit { Some(0) } else { None };
        // Conditional
        unimplemented!();
        let proprietary = if obit { Some(0) } else { None };

        // Read trailing obit (if not previously encountered)
        obit = if obit { buffer.read_field(1, "trailing_obit")? == 1 } else { obit };
        if obit {
            return Err(PduParseErr::InvalidTrailingMbitValue);
        }

        Ok(DLocationUpdateCommand {
            group_identity_report,
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
        buffer.write_bits(MmPduTypeDl::DLocationUpdateCommand.into_raw(), 4);
        // Type1
        buffer.write_bits(self.group_identity_report as u64, 1);
        // Type1
        buffer.write_bits(self.cipher_control as u64, 1);
        // Conditional
        if let Some(ref value) = self.ciphering_parameters {
            buffer.write_bits(*value, 10);
        }

        // Check if any optional field present and place o-bit
        let obit = self.address_extension.is_some();
        delimiters::write_obit(buffer, obit as u8);
        if !obit {
            return Ok(());
        }

        // Type2
        typed::write_type2_generic(obit, buffer, self.address_extension, 24);

        // Conditional
        if let Some(ref value) = self.cell_type_control {
            buffer.write_bits(*value, 3);
        }
        // Conditional
        if let Some(ref value) = self.proprietary {
            buffer.write_bits(*value, 3);
        }
        // Write terminating m-bit
        delimiters::write_mbit(buffer, 0);
        Ok(())
    }
}

impl fmt::Display for DLocationUpdateCommand {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "DLocationUpdateCommand {{ group_identity_report: {:?} cipher_control: {:?} ciphering_parameters: {:?} address_extension: {:?} cell_type_control: {:?} proprietary: {:?} }}",
            self.group_identity_report,
            self.cipher_control,
            self.ciphering_parameters,
            self.address_extension,
            self.cell_type_control,
            self.proprietary,
        )
    }
}
