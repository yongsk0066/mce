// Adapted from corevoikko (voikko-fst/format.rs)

use crate::VfstError;

const COOKIE1: u32 = 0x0001_3A6E;
const COOKIE2: u32 = 0x0003_51FA;

/// Size of the VFST binary header in bytes.
pub const HEADER_SIZE: usize = 16;

/// Parsed VFST file header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VfstHeader {
    pub weighted: bool,
}

/// Parse and validate the 16-byte VFST binary header.
pub fn parse_header(data: &[u8]) -> Result<VfstHeader, VfstError> {
    if data.len() < HEADER_SIZE {
        return Err(VfstError::TooShort {
            expected: HEADER_SIZE,
            actual: data.len(),
        });
    }

    let cookie1 = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let cookie2 = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

    if cookie1 != COOKIE1 || cookie2 != COOKIE2 {
        return Err(VfstError::InvalidMagic);
    }

    Ok(VfstHeader {
        weighted: data[8] == 0x01,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_header(weighted: bool) -> Vec<u8> {
        let mut buf = vec![0u8; HEADER_SIZE];
        buf[..4].copy_from_slice(&COOKIE1.to_le_bytes());
        buf[4..8].copy_from_slice(&COOKIE2.to_le_bytes());
        buf[8] = if weighted { 0x01 } else { 0x00 };
        buf
    }

    #[test]
    fn parse_unweighted() {
        let h = parse_header(&make_header(false)).unwrap();
        assert!(!h.weighted);
    }

    #[test]
    fn parse_weighted() {
        let h = parse_header(&make_header(true)).unwrap();
        assert!(h.weighted);
    }

    #[test]
    fn reject_too_short() {
        assert!(matches!(
            parse_header(&[0u8; 8]).unwrap_err(),
            VfstError::TooShort {
                expected: 16,
                actual: 8
            }
        ));
    }

    #[test]
    fn reject_invalid_magic() {
        let mut data = make_header(false);
        data[0] = 0xFF;
        assert!(matches!(
            parse_header(&data).unwrap_err(),
            VfstError::InvalidMagic
        ));
    }

    #[test]
    fn header_with_trailing_data() {
        let mut data = make_header(false);
        data.extend_from_slice(&[0u8; 100]);
        assert!(!parse_header(&data).unwrap().weighted);
    }

    #[test]
    fn empty_input_returns_too_short() {
        let result = parse_header(&[]);
        assert!(matches!(
            result.unwrap_err(),
            VfstError::TooShort {
                expected: 16,
                actual: 0
            }
        ));
    }

    #[test]
    fn single_byte_returns_too_short() {
        let result = parse_header(&[0x42]);
        assert!(matches!(
            result.unwrap_err(),
            VfstError::TooShort {
                expected: 16,
                actual: 1
            }
        ));
    }

    #[test]
    fn fifteen_bytes_returns_too_short() {
        let result = parse_header(&[0u8; 15]);
        assert!(matches!(
            result.unwrap_err(),
            VfstError::TooShort {
                expected: 16,
                actual: 15
            }
        ));
    }

    #[test]
    fn exactly_sixteen_bytes_valid() {
        let data = make_header(false);
        assert_eq!(data.len(), 16);
        assert!(!parse_header(&data).unwrap().weighted);
    }

    #[test]
    fn corrupt_cookie2_rejected() {
        let mut data = make_header(false);
        data[4] = 0xFF;
        assert!(matches!(
            parse_header(&data).unwrap_err(),
            VfstError::InvalidMagic
        ));
    }

    #[test]
    fn both_cookies_corrupt_rejected() {
        let data = vec![0xFFu8; HEADER_SIZE];
        assert!(matches!(
            parse_header(&data).unwrap_err(),
            VfstError::InvalidMagic
        ));
    }

    #[test]
    fn all_zeros_rejected_as_invalid_magic() {
        let data = vec![0u8; HEADER_SIZE];
        assert!(matches!(
            parse_header(&data).unwrap_err(),
            VfstError::InvalidMagic
        ));
    }

    #[test]
    fn non_boolean_weighted_byte_treated_as_unweighted() {
        let mut data = make_header(false);
        data[8] = 0x02;
        assert!(!parse_header(&data).unwrap().weighted);
    }

    #[test]
    fn reserved_header_bytes_ignored() {
        let mut data = make_header(true);
        for i in 9..16 {
            data[i] = 0xFF;
        }
        assert!(parse_header(&data).unwrap().weighted);
    }
}
