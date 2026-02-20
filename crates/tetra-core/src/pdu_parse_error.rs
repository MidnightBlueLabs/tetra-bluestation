#[derive(Debug, PartialEq, Eq)]
pub enum PduParseErr {
    InvalidPduType { expected: u64, found: u64 },
    BufferEnded { field: Option<&'static str> },
    InvalidTrailingMbitValue,
    InvalidElemId { found: u64 },
    FieldNotPresent { field: Option<&'static str> },
    InvalidValue { field: &'static str, value: u64 },
    InconsistentLength { expected: usize, found: usize },
    Inconsistency { field: &'static str, reason: &'static str },
    NotImplemented { field: Option<&'static str> },
}

/// Checks whether a PDU type value matches the expected value. If not, returns PduParseErr::InvalidPduType
#[macro_export]
macro_rules! expect_pdu_type {
    ($value:expr, $expected:expr) => {{
        let raw_expected = $expected.into_raw();
        if $value == raw_expected {
            Ok(())
        } else {
            Err(PduParseErr::InvalidPduType {
                expected: raw_expected as u64,
                found: $value,
            })
        }
    }};
}

/// Checks whether a value matches an expected value. If not, returns PduParseErr::InvalidValue
#[macro_export]
macro_rules! expect_value {
    ($value:ident, $expected:expr) => {
        $crate::expect_value!(@inner $value, $expected, stringify!($value))
    };
    ($value:expr, $expected:expr, $field:expr) => {
        $crate::expect_value!(@inner $value, $expected, $field)
    };

    (@inner $value:expr, $expected:expr, $field:expr) => {{
        let val = $value;
        if val == $expected {
            Ok(())
        } else {
            Err(PduParseErr::InvalidValue {
                field: $field,
                value: val.into(),
            })
        }
    }};
}

/// Use when an assertion has already failed. Generates a PduParseErr::InvalidValue
#[macro_export]
macro_rules! expect_failed {
    ($value:ident) => {
        $crate::expect_failed!(@inner $value, stringify!($value))
    };
    ($value:expr, $field:expr) => {
        $crate::expect_failed!(@inner $value, $field)
    };

    (@inner $value:expr, $field:expr) => {{
        Err(PduParseErr::InvalidValue {
            field: $field,
            value: $value,
        })
    }};
}

#[macro_export]
macro_rules! let_field {
    ($buf:expr, $ident:ident, $bits:expr) => {
        let $ident = $buf.read_field($bits, stringify!($ident))?;
    };
}
