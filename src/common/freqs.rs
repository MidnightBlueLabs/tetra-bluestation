// TODO FIXME actually match all this against the standard

use serde::Deserialize;

/// ETSI TS 100 392-15 V1.5.1 (2011-02), clause 6: Duplex spacing
const TETRA_DUPLEX_SPACING: [[Option<u32>; 16]; 8] = [
    [ None,    Some(1600), Some(10000), Some(10000), Some(10000), Some(10000), Some(10000), None,        None,        None,        None,    None,    None,    None,    None,    None ],
    [ None,    Some(4500), None,        Some(36000), Some(7600),  None,        None,        None,        Some(45000), Some(45000), None,    None,    None,    None,    None,    None ],
    [ Some(0), Some(0),    Some(0),     Some(0),     Some(0),     Some(0),     Some(0),     Some(0),     Some(0),     Some(0),     Some(0), Some(0), Some(0), Some(0), Some(0), Some(0)],
    [ None,    None,       None,        Some(8000),  Some(8000),  None,        None,        None,        Some(18000), Some(18000), None,    None,    None,    None,    None,    None ],
    [ None,    None,       None,        Some(18000), Some(5000),  None,        Some(30000), Some(30000), None,        Some(39000), None,    None,    None,    None,    None,    None ],
    [ None,    None,       None,        None,        Some(9500),  None,        None,        None,        None,        None,        None,    None,    None,    None,    None,    None ],
    [ None,    None,       None,        None,        None,        None,        None,        None,        None,        None,        None,    None,    None,    None,    None,    None ],
    [ None,    None,       None,        None,        None,        None,        None,        None,        None,        None,        None,    None,    None,    None,    None,    None ],
];


#[derive(Debug, Clone, Deserialize)]
pub struct FreqInfo {
    /// Frequency band in 100MHz increments
    pub band: u8,
    /// Carrier number, 0-4000
    pub carrier: u16,
    /// Frequency offset in Hz, can be negative.
    pub freq_offset: i32,
    /// Duplex spacing in Hz, aka, distance UL is below DL freq
    pub duplex_spacing: u32,
    /// Reverse operation flag, if true, UL is above DL frequency
    pub reverse_operation: bool,
}

impl FreqInfo {

    /// Construct FreqInfo based on DL frequency, duplex spacing and reverse operation flag.
    pub fn from_dlfreq(freq: u32, duplex_spacing: u32, reverse_operation: bool) -> Result<Self, String> {

        if freq % 6250 != 0 {
            return Err(format!("Invalid frequency {}", freq));
        }

        let band = freq / 100000000;
        let remainder = freq % 100000000;
        let mut carrier = remainder / 25000;
        let freq_offset = match remainder % 25000 {
            0 => 0,
            6250 => 6250,
            18750 => {
                carrier += 1; // Adjust carrier for negative offset
                -6250
            },
            12500 => 3,
            _ => return Err(format!("Invalid frequency offset in frequency {}", freq))
        };
        if !(1..=8).contains(&band) {
            return Err(format!("Invalid frequency band {}", band)); 
        };
        if carrier >= 4000 {
            return Err(format!("Invalid carrier number {}", carrier));
        };

        let ret = Self {
            band: band as u8,
            carrier: carrier as u16,
            freq_offset,
            duplex_spacing,
            reverse_operation,
        };

        Ok(ret)
    }

    /// Construct freqinfo based on DL and UL frequencies
    pub fn from_dlul_freqs(dlfreq: u32, ulfreq: u32) -> Result<Self, String> {
        let (reverse_operation, duplex_spacing) = if ulfreq < dlfreq {
            (false, dlfreq - ulfreq)
        } else {
            (true, ulfreq - dlfreq)
        };

        Self::from_dlfreq(dlfreq, duplex_spacing, reverse_operation)
    }

    /// Construct FreqInfo from band, carrier, frequency offset, duplex spacing and reverse operation flag.
    pub fn from_components(band: u8, carrier: u16, freq_offset: i32, duplex_spacing: u32, reverse_operation: bool) -> Result<Self, String> {
        assert!(band <= 8, "Invalid frequency band {}", band);
        assert!(carrier < 4000, "Invalid carrier number {}", carrier);
        assert!(freq_offset == 0 || freq_offset == 6250 || freq_offset == -6250 || freq_offset == 12500, "Invalid frequency offset {}", freq_offset);
        assert!(Self::get_duplex_setting(band, duplex_spacing).is_ok(), "Invalid duplex spacing for band {}, carrier {}, freq_offset {}", band, carrier, freq_offset);
        
        Ok(Self {
            band,
            carrier,
            freq_offset,
            duplex_spacing,
            reverse_operation,
        })
    }

    pub fn from_sysinfo_settings(band: u8, carrier: u16, freq_offset_setting: u8, duplex_setting: u8, reverse_operation: bool) -> Result<Self, String> {
        assert!(band <= 8, "Invalid frequency band {}", band);
        assert!(carrier < 4000, "Invalid carrier number {}", carrier);
        assert!(duplex_setting < 8, "Invalid duplex setting {}", duplex_setting);

        let duplex_spacing = Self::get_duplex_spacing(band, duplex_setting).expect("Invalid duplex spacing for band and duplex setting");

        let freq_offset = match freq_offset_setting {
            0 => 0,
            1 => 6250,
            2 => -6250,
            3 => 12500,
            _ => panic!("Invalid frequency offset setting {}", freq_offset_setting),
        };

        Self::from_components(band, carrier, freq_offset, duplex_spacing, reverse_operation)
    }

    /// Get the frequency offset setting as used in Sysinfo message
    pub fn get_freq_offset_setting(&self) -> u8 {
        match self.freq_offset {
            0 => 0,
            6250 => 1,
            -6250 => 2,
            12500 => 3,
            _ => panic!("Invalid frequency offset {}", self.freq_offset),
        }
    }

    /// Get the duplex spacing in hz for the current frequency band and a given duplex setting, as given in the Sysinfo message
    pub fn get_duplex_spacing(band: u8, duplex_setting: u8) -> Option<u32> {
        
        assert!(duplex_setting < 8, "Invalid duplex setting {}", duplex_setting);
        
        let duplex_spacing = TETRA_DUPLEX_SPACING[duplex_setting as usize][band as usize];
        duplex_spacing.map(|v| v * 1000)
    }

    /// Get the duplex setting as used in Sysinfo for a given frequency band and duplex spacing.
    pub fn get_duplex_setting(band: u8, duplex_spacing: u32) -> Result<u8, String> {
        assert!(band < 16, "Invalid frequency band {}", band);
        assert!(duplex_spacing % 1000 == 0, "Invalid duplex spacing {}", duplex_spacing);
        for (duplex_setting, spacing_vals) in TETRA_DUPLEX_SPACING.iter().enumerate() {
            let spacing = spacing_vals[band as usize];
            if let Some(s) = spacing {
                if s * 1000 == duplex_spacing {
                    return Ok(duplex_setting as u8);
                }
            }
        }
                
        Err(format!("Invalid duplex spacing {} for band {}", duplex_spacing, band))
    }

    /// Compute the DlFreq for this frequency info instance
    pub fn get_dl_freq(&self) -> u32 {
        let mut freq = 100000000 * self.band as i32;
        freq += self.carrier as i32 * 25000;
        freq += self.freq_offset;
        freq as u32
    }

    pub fn get_ul_freq(&self) -> u32 {
        let dl_freq = self.get_dl_freq();
        if !self.reverse_operation {
            dl_freq - self.duplex_spacing
        } else {
            dl_freq + self.duplex_spacing
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_freqinfo_from_dlfreq_and_ul_freq() {
        let freq = 400_000_000 + 1001 * 25_000;
        let duplex_spacing = 10000000;
        let reverse_operation = false;
        let fi = FreqInfo::from_dlfreq(freq, duplex_spacing, reverse_operation).unwrap(); // testing

        assert_eq!(fi.band, 4);
        assert_eq!(fi.carrier, 1001);
        assert_eq!(fi.freq_offset, 0);
        assert_eq!(fi.duplex_spacing, 10_000_000);
        assert!(!fi.reverse_operation);

        println!("DL freq: {}", fi.get_dl_freq());
        println!("UL freq: {}", fi.get_ul_freq());

        let fi2 = FreqInfo::from_components(fi.band, fi.carrier, fi.freq_offset, duplex_spacing, reverse_operation).unwrap(); // testing
        assert_eq!(fi.get_dl_freq(), freq);

        // UL freq should be DL freq + duplex_spacing
        assert_eq!(fi.get_ul_freq(), freq - duplex_spacing);

        // Test duplex spacing / setting
        assert_eq!(FreqInfo::get_duplex_setting(fi.band, 0).unwrap(), 2);
        assert_eq!(FreqInfo::get_duplex_spacing(1, 0).unwrap(), 1600000);

        assert_eq!(fi.band, fi2.band);
        assert_eq!(fi.carrier, fi2.carrier);
        assert_eq!(fi.freq_offset, fi2.freq_offset);
        assert_eq!(fi.duplex_spacing, fi2.duplex_spacing);
        assert_eq!(fi.reverse_operation, fi2.reverse_operation);

        // Test impossible duplex spacing
        assert!(FreqInfo::get_duplex_spacing(1, 7).is_none());
    }
}
