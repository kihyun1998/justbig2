//! JBIG2 segment header parsing and dispatch (ITU T.88 7.2, 7.3).

use crate::error::{Jbig2Error, Result};

/// Segment type codes (lower 6 bits of flags).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SegmentType {
    SymbolDictionary = 0,
    IntermediateTextRegion = 4,
    ImmediateTextRegion = 6,
    ImmediateLosslessTextRegion = 7,
    PatternDictionary = 16,
    IntermediateHalftoneRegion = 20,
    ImmediateHalftoneRegion = 22,
    ImmediateLosslessHalftoneRegion = 23,
    IntermediateGenericRegion = 36,
    ImmediateGenericRegion = 38,
    ImmediateLosslessGenericRegion = 39,
    IntermediateGenericRefinementRegion = 40,
    ImmediateGenericRefinementRegion = 42,
    ImmediateLosslessGenericRefinementRegion = 43,
    PageInformation = 48,
    EndOfPage = 49,
    EndOfStripe = 50,
    EndOfFile = 51,
    Profile = 52,
    CodeTable = 53,
    ColorPalette = 54,
    Extension = 62,
}

impl SegmentType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::SymbolDictionary),
            4 => Some(Self::IntermediateTextRegion),
            6 => Some(Self::ImmediateTextRegion),
            7 => Some(Self::ImmediateLosslessTextRegion),
            16 => Some(Self::PatternDictionary),
            20 => Some(Self::IntermediateHalftoneRegion),
            22 => Some(Self::ImmediateHalftoneRegion),
            23 => Some(Self::ImmediateLosslessHalftoneRegion),
            36 => Some(Self::IntermediateGenericRegion),
            38 => Some(Self::ImmediateGenericRegion),
            39 => Some(Self::ImmediateLosslessGenericRegion),
            40 => Some(Self::IntermediateGenericRefinementRegion),
            42 => Some(Self::ImmediateGenericRefinementRegion),
            43 => Some(Self::ImmediateLosslessGenericRefinementRegion),
            48 => Some(Self::PageInformation),
            49 => Some(Self::EndOfPage),
            50 => Some(Self::EndOfStripe),
            51 => Some(Self::EndOfFile),
            52 => Some(Self::Profile),
            53 => Some(Self::CodeTable),
            54 => Some(Self::ColorPalette),
            62 => Some(Self::Extension),
            _ => None,
        }
    }
}

/// Parsed segment header.
#[derive(Debug, Clone)]
pub struct SegmentHeader {
    /// Segment number.
    pub number: u32,
    /// Raw flags byte.
    pub flags: u8,
    /// Segment type (lower 6 bits of flags).
    pub seg_type: Option<SegmentType>,
    /// Referred-to segment numbers.
    pub referred_to_segments: Vec<u32>,
    /// Page association.
    pub page_association: u32,
    /// Data length (may be 0xFFFFFFFF for unknown).
    pub data_length: u32,
}

impl SegmentHeader {
    /// Parse a segment header from data. Returns header and bytes consumed.
    /// Returns None if not enough data yet.
    pub fn parse(data: &[u8]) -> Result<Option<(Self, usize)>> {
        // Minimum header size: 4 (number) + 1 (flags) + 1 (rtscarf) + 1 (page) + 4 (length) = 11
        if data.len() < 11 {
            return Ok(None);
        }

        let number = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        if number == 0xFFFFFFFF {
            return Err(Jbig2Error::InvalidData("segment number too large".into()));
        }

        let flags = data[4];
        let seg_type = SegmentType::from_u8(flags & 63);

        // 7.2.4 referred-to segment count
        let rtscarf = data[5];
        let (referred_to_count, mut offset): (u32, usize) = if (rtscarf & 0xE0) == 0xE0 {
            // Long form
            if data.len() < 10 {
                return Ok(None);
            }
            let long_val = u32::from_be_bytes([data[5], data[6], data[7], data[8]]);
            let count = long_val & 0x1FFFFFFF;
            (count, 5 + 4 + ((count + 1) / 8) as usize)
        } else {
            let count = (rtscarf >> 5) as u32;
            (count, 5 + 1)
        };

        // 7.2.5 referred-to segment number size
        let ref_seg_size: usize = if number <= 256 {
            1
        } else if number <= 65536 {
            2
        } else {
            4
        };

        // 7.2.6 page association size
        let pa_size: usize = if flags & 0x40 != 0 { 4 } else { 1 };

        let total_needed = offset + (referred_to_count as usize) * ref_seg_size + pa_size + 4;
        if data.len() < total_needed {
            return Ok(None);
        }

        // Parse referred-to segments
        let mut referred_to_segments = Vec::with_capacity(referred_to_count as usize);
        for _ in 0..referred_to_count {
            let seg_num = match ref_seg_size {
                1 => data[offset] as u32,
                2 => u16::from_be_bytes([data[offset], data[offset + 1]]) as u32,
                4 => u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]),
                _ => unreachable!(),
            };
            referred_to_segments.push(seg_num);
            offset += ref_seg_size;
        }

        // 7.2.6 page association
        let page_association = if pa_size == 4 {
            let v = u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]);
            offset += 4;
            v
        } else {
            let v = data[offset] as u32;
            offset += 1;
            v
        };

        // 7.2.7 data length
        let data_length = u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]);
        offset += 4;

        Ok(Some((
            SegmentHeader {
                number,
                flags,
                seg_type,
                referred_to_segments,
                page_association,
                data_length,
            },
            offset,
        )))
    }
}

/// Region segment info header (7.4.1) — 17 bytes.
#[derive(Debug, Clone, Copy)]
pub struct RegionSegmentInfo {
    pub width: u32,
    pub height: u32,
    pub x: u32,
    pub y: u32,
    pub flags: u8,
    pub op: u8,
}

impl RegionSegmentInfo {
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 17 {
            return Err(Jbig2Error::InvalidData("region segment info too short".into()));
        }
        Ok(RegionSegmentInfo {
            width: u32::from_be_bytes([data[0], data[1], data[2], data[3]]),
            height: u32::from_be_bytes([data[4], data[5], data[6], data[7]]),
            x: u32::from_be_bytes([data[8], data[9], data[10], data[11]]),
            y: u32::from_be_bytes([data[12], data[13], data[14], data[15]]),
            flags: data[16],
            op: data[16] & 0x07,
        })
    }
}
