#[derive(Debug, PartialEq, Eq)]
pub struct Type4FieldGeneric {
    pub field_id: u64,
    pub len: usize,
    pub elems: usize,
    /// Up to 64 bits of data (later bits are discarded)
    pub data: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Type3FieldGeneric {
    pub field_id: u64,
    pub len: usize,
    /// Up to 64 bits of data (later bits are discarded)
    pub data: u64,
}

/// Helper functions for dealing with type2, type3 and type4 fields for MLE, CMCE, MM and SNDCP PDUs.
pub mod delimiters {
    use crate::{bitbuffer::BitBuffer, pdu_parse_error::PduParseErr};

    /// Read the o-bit between type1 and type2/type3 elements
    pub fn read_obit(buffer: &mut BitBuffer) -> Result<bool, PduParseErr> {
        Ok(buffer.read_field(1, "obit")? == 1)
    }

    /// Write the o-bit between type1 and type2/type3 elements
    pub fn write_obit(buffer: &mut BitBuffer, val: u8) {
        buffer.write_bit(val);
    }

    /// Read a p-bit preceding a type2 element
    pub fn read_pbit(buffer: &mut BitBuffer) -> Result<bool, PduParseErr> {
        Ok(buffer.read_field(1, "pbit")? == 1)
    }

    /// Write the p-bit preceding a type2 element
    pub fn write_pbit(buffer: &mut BitBuffer, val: u8) {
        buffer.write_bit(val);
    }

    /// Read an m-bit found before a type3 or type4 element, and trailing the message
    pub fn read_mbit(buffer: &mut BitBuffer) -> Result<bool, PduParseErr> {
        Ok(buffer.read_field(1, "mbit")? == 1)
    }

    /// Write the m-bit before a type3 or type4 element, and trailing the message
    pub fn write_mbit(buffer: &mut BitBuffer, val: u8) {
        buffer.write_bit(val);
    }
}

pub mod typed {
    use crate::{
        bitbuffer::BitBuffer,
        pdu_parse_error::PduParseErr,
        typed_pdu_fields::{Type3FieldGeneric, Type4FieldGeneric, delimiters},
    };

    pub fn parse_type2_generic(
        obit: bool,
        buffer: &mut BitBuffer,
        num_bits: usize,
        field_name: &'static str,
    ) -> Result<Option<u64>, PduParseErr> {
        if !obit {
            return Ok(None);
        }
        match delimiters::read_pbit(buffer) {
            Ok(true) => {
                // Field present
                tracing::trace!("parse_type2_generic field_present {:20}: {}", field_name, buffer.dump_bin());
                match buffer.read_field(num_bits, field_name) {
                    Ok(v) => Ok(Some(v)),
                    Err(e) => Err(e),
                }
            }
            Ok(false) => {
                // Field not present
                tracing::trace!("parse_type2_generic no_field      {:20}: {}", field_name, buffer.dump_bin());
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    /// Parse a Type-2 element into a struct that implements `from_bitbuf`.
    pub fn parse_type2_struct<T, F>(obit: bool, buffer: &mut BitBuffer, parser: F) -> Result<Option<T>, PduParseErr>
    where
        F: FnOnce(&mut BitBuffer) -> Result<T, PduParseErr>,
    {
        if !obit {
            return Ok(None);
        }

        match delimiters::read_pbit(buffer) {
            Ok(true) => {
                // Field present
                tracing::trace!("parse_type2_struct field_present: {}", buffer.dump_bin());
                let value = parser(buffer)?;
                Ok(Some(value))
            }
            Ok(false) => {
                // Field not present
                tracing::trace!("parse_type2_struct no_field      : {}", buffer.dump_bin());
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    /// Write one Type-2 element.
    /// If `value` is `Some(v)`, writes P-bit=1 then `len` bits of `v`. If `None`, writes P-bit=0.
    pub fn write_type2_generic(obit: bool, buffer: &mut BitBuffer, value: Option<u64>, len: usize) {
        // No optional elements
        if !obit {
            assert!(value.is_none(), "Type2 element cannot be present when obit is false");
            return;
        }

        match value {
            Some(v) => {
                tracing::trace!("write_type2_generic field_present {}", buffer.dump_bin());
                delimiters::write_pbit(buffer, 1);
                buffer.write_bits(v, len);
            }
            None => {
                tracing::trace!("write_type2_generic no_field {}", buffer.dump_bin());
                delimiters::write_pbit(buffer, 0);
            }
        }
    }

    /// Write a Type-2 element from a struct that implements `to_bitbuf`.
    pub fn write_type2_struct<T, F>(obit: bool, buffer: &mut BitBuffer, value: &Option<T>, writer: F) -> Result<(), PduParseErr>
    where
        F: Fn(&T, &mut BitBuffer) -> Result<(), PduParseErr>,
    {
        // No optional elements
        if !obit {
            assert!(value.is_none(), "Type2 element cannot be present when obit is false");
            return Ok(());
        }
        match value {
            Some(v) => {
                tracing::trace!("write_type2_struct field_present {}", buffer.dump_bin());
                delimiters::write_pbit(buffer, 1);
                writer(v, buffer)?;
                Ok(())
            }
            None => {
                tracing::trace!("write_type2_struct no_field {}", buffer.dump_bin());
                delimiters::write_pbit(buffer, 0);
                Ok(())
            }
        }
    }

    /// Read the m-bit for a type3 or type4 element without advancing the buffer pos
    /// If set, reads the type3/4 field identifier and compares to expected id.
    /// Return true if present, false if not present, or PduParseErr on error
    fn peek_type34_mbit_and_id(buffer: &BitBuffer, expected_id: u64) -> Result<bool, PduParseErr> {
        let mbit = buffer.peek_bits(1);
        match mbit {
            Some(0) => {
                // Field not present
                Ok(false)
            }
            Some(1) => {
                // Some field is present, read and compare id
                let id_bits = buffer.peek_bits_posoffset(1, 4);
                match id_bits {
                    Some(id) if id == expected_id => {
                        // The expected is here; the field exists
                        Ok(true)
                    }
                    Some(_) => {
                        // Some different field is here
                        Ok(false)
                    }
                    None => {
                        // Read failed
                        Err(PduParseErr::BufferEnded {
                            field: Some("peek_type34_mbit_and_id id_bits"),
                        })
                    }
                }
            }
            None => Err(PduParseErr::BufferEnded {
                field: Some("peek_type34_mbit_and_id mbit"),
            }),
            _ => panic!(), // Never happens
        }
    }

    /// Parse type3 field into a placeholder struct, pending implementation.
    /// Checks whether a given type3 field identifier is present. If not, returns None without advancing
    /// the bitbuffer position. If present, reads the element and returns it as a u64, advancing the buffer position.
    /// to the end of the element.
    pub fn parse_type3_generic<E>(obit: bool, buffer: &mut BitBuffer, expected_id: E) -> Result<Option<Type3FieldGeneric>, PduParseErr>
    where
        E: Into<u64>,
    {
        // If the obit is set to false, the element cannot be present
        if !obit {
            return Ok(None);
        }

        // Obit is present, check if mbit present, and check if the elementid is the expected one
        let id = expected_id.into();
        let field_present = peek_type34_mbit_and_id(buffer, id)?;
        if !field_present {
            return Ok(None);
        }

        // Target field is present. Advance buffer position and read field contents
        buffer.seek_rel(5);
        let len_bits = match buffer.read_bits(11) {
            Some(x) => x as usize,
            None => {
                return Err(PduParseErr::BufferEnded {
                    field: Some("parse_type3_generic len_bits"),
                });
            }
        };
        let read_bits = if len_bits > 64 { 64 } else { len_bits };
        let data = match buffer.read_bits(read_bits) {
            Some(x) => x,
            None => {
                return Err(PduParseErr::BufferEnded {
                    field: Some("parse_type3_generic data"),
                });
            }
        };

        // Seek forward to end of element, if larger than 64 bits
        if len_bits > 64 {
            tracing::warn!("Type3 element {} length {} exceeds 64 bits, data truncated", id, len_bits);
            buffer.seek_rel(len_bits as isize - 64);
        }

        Ok(Some(Type3FieldGeneric {
            field_id: id,
            len: len_bits,
            data,
        }))
    }

    /// Parse a Type-3 element into a struct that implements `from_bitbuf`.
    /// Validates the m-bit and element ID, then calls the parser function directly on the buffer if present.
    pub fn parse_type3_struct<E, T, F>(obit: bool, buffer: &mut BitBuffer, expected_id: E, parser: F) -> Result<Option<T>, PduParseErr>
    where
        E: Into<u64>,
        F: FnOnce(&mut BitBuffer) -> Result<T, PduParseErr>,
    {
        // If the obit is set to false, the element cannot be present
        if !obit {
            return Ok(None);
        }

        // Obit is present, peek if mbit present, and peek if the elementid is the expected one
        let id = expected_id.into();
        let field_present = peek_type34_mbit_and_id(buffer, id)?;
        if !field_present {
            tracing::trace!("parse_type3_struct no_field {}: {}", id, buffer.dump_bin());
            return Ok(None);
        }
        // Target field is present. Advance buffer past m-bit (1) + id (4) + length (11)
        buffer.seek_rel(5); // m-bit + id

        tracing::trace!("parse_type3_struct got header for {:2}: {}", id, buffer.dump_bin());

        let len_bits = match buffer.read_bits(11) {
            Some(x) => x as usize,
            None => {
                return Err(PduParseErr::BufferEnded {
                    field: Some("parse_type3_struct len_bits"),
                });
            }
        };

        tracing::trace!("parse_type3_struct got len {:4}:      {}", len_bits, buffer.dump_bin());

        // Store current position to check parsed length for discrepancies. Then, read length
        let start_pos = buffer.get_pos();

        // Now buffer is positioned at the data. Parse the struct directly. The parser is responsible for reading exactly len_bits
        let result = parser(buffer)?;

        tracing::trace!("parse_type3_struct done parsing:      {}", buffer.dump_bin());

        // If read out length does not match expectation, something went very wrong
        if start_pos + len_bits != buffer.get_pos() {
            tracing::warn!(
                "Type3 element {} parsed length mismatch: expected {}, parsed {}",
                id,
                len_bits,
                buffer.get_pos() - start_pos
            );
            return Err(PduParseErr::InconsistentLength {
                expected: len_bits,
                found: (buffer.get_pos() - start_pos) as usize,
            });
        };

        // Parsed and expected length matches, return result
        Ok(Some(result))
    }

    /// Write the type4 header start (1-bit mbit + 4-bit field type)
    pub fn write_type34_header_generic(buffer: &mut BitBuffer, field_type: u64) {
        delimiters::write_mbit(buffer, 1);
        buffer.write_bits(field_type, 4);
    }

    /// Write an optional Type-3 element using a `to_bitbuf` function.
    pub fn write_type3_struct<E, T, F>(
        obit: bool,
        buffer: &mut BitBuffer,
        value: &Option<T>,
        field_id: E,
        writer: F,
    ) -> Result<(), PduParseErr>
    where
        E: Into<u64>,
        F: Fn(&T, &mut BitBuffer) -> Result<(), PduParseErr>,
    {
        // Sanity check
        let id = field_id.into();
        if !obit && value.is_some() {
            return Err(PduParseErr::InvalidValue {
                field: "write_type3_struct",
                value: id,
            });
        }

        if let Some(elem) = value {
            tracing::trace!("write_type3_struct writing field {:2} {}", id, buffer.dump_bin());

            // Write mbit and 4-bit field ID, then length field, then write the element itself
            write_type34_header_generic(buffer, id);
            let pos_len_field = buffer.get_raw_pos();
            buffer.write_bits(0, 11); // Write instead of seek to autoexpand

            tracing::trace!("write_type3_struct header           {}", buffer.dump_bin());

            writer(elem, buffer)?;

            tracing::trace!("write_type3_struct payload          {}", buffer.dump_bin());

            // Calculate actual length and backfill
            let pos_end = buffer.get_raw_pos();
            let len_bits = (pos_end - pos_len_field - 11) as u64;
            buffer.set_raw_pos(pos_len_field);
            buffer.write_bits(len_bits, 11);

            tracing::trace!("write_type3_struct len {:2}:          {}", len_bits, buffer.dump_bin());
            buffer.set_raw_pos(pos_end);
        } else {
            // Don't write anything (no mbit)
            tracing::trace!("write_type3_struct no_field          {}", buffer.dump_bin());
        }
        Ok(())
    }

    /// Write an optional Type-3 element using a `to_bitbuf` function.
    pub fn write_type3_generic<E>(
        obit: bool,
        buffer: &mut BitBuffer,
        value: &Option<Type3FieldGeneric>,
        field_id: E,
    ) -> Result<(), PduParseErr>
    where
        E: Into<u64>,
    {
        // Sanity check
        let id = field_id.into();
        if !obit && value.is_some() {
            return Err(PduParseErr::InvalidValue {
                field: "write_type3_generic",
                value: id,
            });
        }

        if let Some(elem) = value {
            tracing::trace!("write_type3_generic field_present {}", buffer.dump_bin());
            // Write mbit and 4-bit field ID, then write length, then the element itself
            write_type34_header_generic(buffer, id);
            buffer.write_bits(elem.len as u64, 11);
            buffer.write_bits(elem.data, elem.len);
        } else {
            // Don't write anything (no mbit)
            tracing::trace!("write_type3_generic no_field {}", buffer.dump_bin());
        }
        Ok(())
    }

    fn parse_type4_header(buffer: &mut BitBuffer, expected_id: u64) -> Result<Option<(usize, usize)>, PduParseErr> {
        // Check whether the element is present
        let id = expected_id.into();
        let field_present = peek_type34_mbit_and_id(buffer, id)?;
        if !field_present {
            return Ok(None);
        }

        // Target field is present. Advance buffer position and read field contents
        buffer.seek_rel(5);
        let len_bits = match buffer.read_bits(11) {
            Some(x) => x as usize,
            None => {
                return Err(PduParseErr::BufferEnded {
                    field: Some("parse_type4_header len_bits"),
                });
            }
        };
        // tracing::debug!("MmType4FieldUl: len_bits: {}", len_bits);
        let num_elems = match buffer.read_bits(6) {
            Some(x) => x as usize,
            None => {
                return Err(PduParseErr::BufferEnded {
                    field: Some("parse_type4_header num_elems"),
                });
            }
        };

        tracing::trace!(
            "parse_type4_header got header for {:2}, len {}, count {}: {}",
            id,
            len_bits,
            num_elems,
            buffer.dump_bin()
        );

        Ok(Some((num_elems, len_bits - 6)))
    }

    /// Parse a Type-4 element into a Vec of structs that implement `from_bitbuf`.
    pub fn parse_type4_struct<E, T, F>(obit: bool, buffer: &mut BitBuffer, expected_id: E, parser: F) -> Result<Option<Vec<T>>, PduParseErr>
    where
        E: Into<u64>,
        F: Fn(&mut BitBuffer) -> Result<T, PduParseErr>,
    {
        // If the obit is set to false, the element cannot be present
        if !obit {
            return Ok(None);
        }

        // Obit is present, check if mbit present, and check if the elementid is the expected one
        let id = expected_id.into();
        match parse_type4_header(buffer, id)? {
            None => {
                // Field not present
                Ok(None)
            }
            Some((num_elems, len_bits)) => {
                // Field is present, and we've gout our total lenght and number of elements
                let mut elems = Vec::with_capacity(num_elems);
                let start_pos = buffer.get_pos();

                // Parse all elements into array structs
                for _ in 0..num_elems {
                    let elem = parser(buffer)?;
                    elems.push(elem);
                }

                // If read out length does not match expectation, something went very wrong
                if start_pos + len_bits != buffer.get_pos() {
                    tracing::warn!(
                        "Type4 element {} parsed length mismatch: expected {}, parsed {}",
                        id,
                        len_bits,
                        buffer.get_pos() - start_pos
                    );
                    return Err(PduParseErr::InconsistentLength {
                        expected: len_bits,
                        found: (buffer.get_pos() - start_pos) as usize,
                    });
                };

                // Parsed and expected length matches, return result
                Ok(Some(elems))
            }
        }
    }

    /// Parse a Type-4 element into a placeholder struct type, pending proper implementation.
    /// Imperfect as we cannot know individual element sizes, besides issues with overflowing the 64-bit read
    pub fn parse_type4_generic<E>(obit: bool, buffer: &mut BitBuffer, expected_id: E) -> Result<Option<Type4FieldGeneric>, PduParseErr>
    where
        E: Into<u64>,
    {
        // If the obit is set to false, the element cannot be present
        if !obit {
            return Ok(None);
        }

        // Obit is present, check if mbit present, and check if the elementid is the expected one
        let id = expected_id.into();
        match parse_type4_header(buffer, id)? {
            None => {
                // Field not present
                Ok(None)
            }
            Some((num_elems, len_bits)) => {
                // Field is present, and we've got our total lenght and number of elements
                let read_bits = if len_bits > 64 { 64 } else { len_bits };
                let val = buffer.read_field(read_bits, "parse_type4_header")?;

                // Build placeholder return struct
                let ret = Type4FieldGeneric {
                    field_id: id,
                    len: len_bits,
                    elems: num_elems,
                    data: val,
                };

                // Seek forward to end of element, if larger than 64 bits
                if len_bits > 64 {
                    tracing::warn!("Type4 element {} length {} exceeds 64 bits, data truncated", id, len_bits);
                    buffer.seek_rel(len_bits as isize - 64);
                }

                // Parsed and expected length matches, return result
                Ok(Some(ret))
            }
        }
    }

    /// Write a Type-4 element from a Vec of structs using a `to_bitbuf` function.
    pub fn write_type4_struct<E, T, F>(
        obit: bool,
        buffer: &mut BitBuffer,
        value: &Option<Vec<T>>,
        field_id: E,
        writer: F,
    ) -> Result<(), PduParseErr>
    where
        E: Into<u64>,
        F: Fn(&T, &mut BitBuffer) -> Result<(), PduParseErr>,
    {
        // Sanity check
        let id = field_id.into();
        if !obit && value.is_some() {
            return Err(PduParseErr::InvalidValue {
                field: "write_type4_struct",
                value: id,
            });
        }

        if let Some(elems) = value {
            if elems.is_empty() {
                // todo fixme we need to check the standards docs for knowing what to do here
                tracing::warn!("write_type4_struct called with empty elems vec. Check standard to see what is proper behavior");
            }

            // Write m-bit and field ID
            write_type34_header_generic(buffer, id);

            // Reserve space for length (11 bits) + num_elems (6 bits)
            let pos_len_field = buffer.get_raw_pos();
            buffer.write_bits(0, 11 + 6); // Write instead of space to autoexpand

            // Write all elements
            for elem in elems {
                writer(elem, buffer)?;
            }

            // Calculate actual length and backfill
            let pos_end = buffer.get_raw_pos();
            let len_bits = (pos_end - pos_len_field - 11) as u64;
            let num_elems = elems.len() as u64;

            // tracing::debug!("Wrote {} elements for Type4 field {}, total len {}, buf now: {}", elems.len(), id, len_bits, buffer.dump_bin());

            buffer.set_raw_pos(pos_len_field);
            buffer.write_bits(len_bits, 11);
            buffer.write_bits(num_elems, 6);
            buffer.set_raw_pos(pos_end);
        }
        // If None, don't write anything (no m-bit)
        Ok(())
    }

    /// Write a Type-4 element from a Vec of structs using a `to_bitbuf` function.
    pub fn write_type4_todo<E>(
        obit: bool,
        _buffer: &mut BitBuffer,
        value: &Option<Type4FieldGeneric>,
        field_id: E,
    ) -> Result<(), PduParseErr>
    where
        E: Into<u64>,
    {
        // Sanity check
        let id = field_id.into();
        if !obit && value.is_some() {
            return Err(PduParseErr::InvalidValue {
                field: "write_type4_todo",
                value: id,
            });
        }

        if let Some(_elem) = value {
            unimplemented!("can't generically write a type4 field");
        }

        Ok(())
    }
}
