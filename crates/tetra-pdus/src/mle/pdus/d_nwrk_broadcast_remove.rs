use core::fmt;

use tetra_core::typed_pdu_fields::*;
use tetra_core::{BitBuffer, expect_pdu_type, pdu_parse_error::PduParseErr};

use crate::mle::enums::mle_pdu_type_dl::MlePduTypeDl;

/// Representation of the D-NWRK-BROADCAST REMOVE PDU (Clause 18.4.1.4.1c).
/// Upon receipt from the SwMI, the message shall inform the MS-MLE about broadcast neighbour cell and channel information received on the present cell that is to be removed from the MS's memory.
/// Response expected: -
/// Response to: -

// note 1: If present, the element shall indicate how many "removal data for CA cell" elements follow. If not present, no "removal data for CA cell" elements shall follow.
// note 2: The element definition is contained in clause 18.5 which gives the type and length for each sub-element which is included in this element. The element shall be present as many times as indicated by the "Number of CA cells for removal" element. There shall be no P-bit preceding each "removal data for CA cell" element which is carried by this PDU.
// note 3: If present, the element shall indicate how many "removal data for DA cell" elements follow. If not present, no "removal data for DA cell" elements shall follow.
// note 4: The element definition is contained in clause 18.5 which gives the type and length for each sub-element which is included in this element. The element shall be present as many times as indicated by the "Number of DA cells for removal" element. There shall be no P-bit preceding each "removal data for DA cell" element which is carried by this PDU.
// note 5: This element shall not be included unless its value is appropriate to all cells using the channel on which this PDU is sent.
// note 6: Shall not be used in the present document.
#[derive(Debug)]
pub struct DNwrkBroadcastRemove {
    /// Type1, 4 bits, D-NWRK-BROADCAST REMOVE,
    pub pdu_type_extension: u8,
    /// Type2, 5 bits, See note 1,
    pub number_of_ca_cells_for_removal: Option<u64>,
    /// Conditional See note 2,
    pub removal_data_for_ca_cell: Option<u64>,
    /// Type2, 8 bits, See note 3,
    pub number_of_da_cells_for_removal: Option<u64>,
    /// Conditional See note 4,
    pub removal_data_for_da_cell: Option<u64>,
    /// Conditional See note 5,
    pub removal_data_for_serving_cell: Option<u64>,
    /// Type2, 8 bits, See note 6,
    pub reserved1: Option<u64>,
    /// Type2, 8 bits, See note 6,
    pub reserved2: Option<u64>,
    /// Type2, 16 bits, See note 6,
    pub reserved3: Option<u64>,
    /// Type2, 32 bits, See note 6,
    pub reserved4: Option<u64>,
}

#[allow(unreachable_code)] // TODO FIXME review, finalize and remove this
#[allow(unused_variables)]
impl DNwrkBroadcastRemove {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(3, "pdu_type")?;
        expect_pdu_type!(pdu_type, MlePduTypeDl::ExtPdu)?;

        // Type1
        let pdu_type_extension = buffer.read_field(4, "pdu_type_extension")? as u8;

        // obit designates presence of any further type2, type3 or type4 fields
        let obit = delimiters::read_obit(buffer)?;

        // Type2
        let number_of_ca_cells_for_removal = typed::parse_type2_generic(obit, buffer, 5, "number_of_ca_cells_for_removal")?;
        // Conditional
        unimplemented!();
        let removal_data_for_ca_cell = if obit { Some(0) } else { None };
        // Type2
        let number_of_da_cells_for_removal = typed::parse_type2_generic(obit, buffer, 8, "number_of_da_cells_for_removal")?;
        // Conditional
        unimplemented!();
        let removal_data_for_da_cell = if obit { Some(0) } else { None };
        // Conditional
        unimplemented!();
        let removal_data_for_serving_cell = if obit { Some(0) } else { None };
        // Type2
        let reserved1 = typed::parse_type2_generic(obit, buffer, 8, "reserved1")?;
        // Type2
        let reserved2 = typed::parse_type2_generic(obit, buffer, 8, "reserved2")?;
        // Type2
        let reserved3 = typed::parse_type2_generic(obit, buffer, 16, "reserved3")?;
        // Type2
        let reserved4 = typed::parse_type2_generic(obit, buffer, 32, "reserved4")?;

        // Read trailing obit (if not previously encountered)
        obit = if obit { buffer.read_field(1, "trailing_obit")? == 1 } else { obit };
        if obit {
            return Err(PduParseErr::InvalidTrailingMbitValue);
        }

        Ok(DNwrkBroadcastRemove {
            pdu_type_extension,
            number_of_ca_cells_for_removal,
            removal_data_for_ca_cell,
            number_of_da_cells_for_removal,
            removal_data_for_da_cell,
            removal_data_for_serving_cell,
            reserved1,
            reserved2,
            reserved3,
            reserved4,
        })
    }

    /// Serialize this PDU into the given BitBuffer.
    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        // PDU Type
        buffer.write_bits(MlePduTypeDl::ExtPdu.into_raw(), 3);
        // Type1
        buffer.write_bits(self.pdu_type_extension as u64, 4);

        // Check if any optional field present and place o-bit
        let obit = self.number_of_ca_cells_for_removal.is_some()
            || self.number_of_da_cells_for_removal.is_some()
            || self.reserved1.is_some()
            || self.reserved2.is_some()
            || self.reserved3.is_some()
            || self.reserved4.is_some();
        delimiters::write_obit(buffer, obit as u8);
        if !obit {
            return Ok(());
        }

        // Type2
        typed::write_type2_generic(obit, buffer, self.number_of_ca_cells_for_removal, 5);

        // Conditional
        if let Some(ref _value) = self.removal_data_for_ca_cell {
            unimplemented!();
            buffer.write_bits(*_value, 999);
        }
        // Type2
        typed::write_type2_generic(obit, buffer, self.number_of_da_cells_for_removal, 8);

        // Conditional
        if let Some(ref _value) = self.removal_data_for_da_cell {
            unimplemented!();
            buffer.write_bits(*_value, 999);
        }
        // Conditional
        if let Some(ref _value) = self.removal_data_for_serving_cell {
            unimplemented!();
            buffer.write_bits(*_value, 999);
        }
        // Type2
        typed::write_type2_generic(obit, buffer, self.reserved1, 8);

        // Type2
        typed::write_type2_generic(obit, buffer, self.reserved2, 8);

        // Type2
        typed::write_type2_generic(obit, buffer, self.reserved3, 16);

        // Type2
        typed::write_type2_generic(obit, buffer, self.reserved4, 32);

        // Write terminating m-bit
        delimiters::write_mbit(buffer, 0);
        Ok(())
    }
}

impl fmt::Display for DNwrkBroadcastRemove {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DNwrkBroadcastRemove {{ pdu_type_extension: {:?} number_of_ca_cells_for_removal: {:?} removal_data_for_ca_cell: {:?} number_of_da_cells_for_removal: {:?} removal_data_for_da_cell: {:?} removal_data_for_serving_cell: {:?} reserved1: {:?} reserved2: {:?} reserved3: {:?} reserved4: {:?} }}",
            self.pdu_type_extension,
            self.number_of_ca_cells_for_removal,
            self.removal_data_for_ca_cell,
            self.number_of_da_cells_for_removal,
            self.removal_data_for_da_cell,
            self.removal_data_for_serving_cell,
            self.reserved1,
            self.reserved2,
            self.reserved3,
            self.reserved4,
        )
    }
}
