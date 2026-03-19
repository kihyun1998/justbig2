//! JBIG2 file header parsing (ITU T.88 D.4).

use crate::error::{Jbig2Error, Result};

/// JBIG2 8-byte identification string.
pub const MAGIC: [u8; 8] = [0x97, 0x4A, 0x42, 0x32, 0x0D, 0x0A, 0x1A, 0x0A];

/// File organization mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Organization {
    Sequential,
    RandomAccess,
}

/// Parsed JBIG2 file header.
#[derive(Debug, Clone)]
pub struct FileHeader {
    pub organization: Organization,
    /// Number of pages, or None if unknown.
    pub n_pages: Option<u32>,
}

impl FileHeader {
    /// Parse a JBIG2 file header from data.
    /// Returns `Ok(None)` if not enough data yet, `Ok(Some(...))` on success.
    pub fn parse(data: &[u8]) -> Result<Option<(Self, usize)>> {
        if data.len() < 9 {
            return Ok(None);
        }

        if data[..8] != MAGIC {
            return Err(Jbig2Error::InvalidData("invalid JBIG2 magic number".into()));
        }

        let flags = data[8];

        // Check for unsupported amendments
        if flags & 0x04 != 0 {
            return Err(Jbig2Error::UnsupportedFeature(
                "12 adaptive template pixels (T.88 amendment 2)".into(),
            ));
        }
        if flags & 0x08 != 0 {
            return Err(Jbig2Error::UnsupportedFeature(
                "colored region segments (T.88 amendment 3)".into(),
            ));
        }

        let organization = if flags & 1 != 0 {
            Organization::Sequential
        } else {
            Organization::RandomAccess
        };

        let pages_known = flags & 2 == 0;
        if pages_known {
            if data.len() < 13 {
                return Ok(None);
            }
            let n_pages = u32::from_be_bytes([data[9], data[10], data[11], data[12]]);
            Ok(Some((FileHeader { organization, n_pages: Some(n_pages) }, 13)))
        } else {
            Ok(Some((FileHeader { organization, n_pages: None }, 9)))
        }
    }
}
