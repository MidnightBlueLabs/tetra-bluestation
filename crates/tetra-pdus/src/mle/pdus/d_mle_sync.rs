use core::fmt;

use tetra_core::{BitBuffer, pdu_parse_error::PduParseErr};

/// Clause 18.4.2.1
#[derive(Debug, Clone)]
pub struct DMleSync {
    // 10 Country code
    pub mcc: u16,
    // 14
    pub mnc: u16,
    // 2
    pub neighbor_cell_broadcast: u8,
    // 2
    pub cell_load_ca: u8,
    // 1
    pub late_entry_supported: bool,
}

impl DMleSync {
    pub fn from_bitbuf(buf: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let mcc = buf.read_field(10, "mcc")? as u16;
        let mnc = buf.read_field(14, "mnc")? as u16;
        let neighbor_cell_broadcast = buf.read_field(2, "neighbor_cell_broadcast")? as u8;
        let cell_load_ca = buf.read_field(2, "cell_load_ca")? as u8;
        let late_entry_supported = buf.read_field(1, "late_entry_supported")? != 0;

        Ok(DMleSync {
            mcc,
            mnc,
            neighbor_cell_broadcast,
            cell_load_ca,
            late_entry_supported,
        })
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) {
        buf.write_bits(self.mcc as u64, 10);
        buf.write_bits(self.mnc as u64, 14);
        buf.write_bits(self.neighbor_cell_broadcast as u64, 2);
        buf.write_bits(self.cell_load_ca as u64, 2);
        buf.write_bits(self.late_entry_supported as u8 as u64, 1);
    }
}

impl fmt::Display for DMleSync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DMleSync {{ mcc: {} mnc: {} neighbor_cell_broadcast: {} cell_load_ca: {} late_entry_supported: {} }}",
            self.mcc, self.mnc, self.neighbor_cell_broadcast, self.cell_load_ca, self.late_entry_supported,
        )
    }
}
