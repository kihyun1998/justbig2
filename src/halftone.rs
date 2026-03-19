//! Halftone Region decoder (ITU T.88 6.6, 6.7, 7.4.4, 7.4.5).
//!
//! Pattern dictionaries define tile patterns; halftone regions place them
//! on a grid using gray-scale index values decoded from bitplanes.

use crate::error::Result;
use crate::image::{ComposeOp, Jbig2Image};

/// A pattern dictionary — array of pattern tile images.
#[derive(Debug, Clone)]
pub struct PatternDict {
    pub patterns: Vec<Jbig2Image>,
    pub hpw: u32,
    pub hph: u32,
}

/// Pattern dictionary parameters.
#[derive(Debug, Clone)]
pub struct PatternDictParams {
    pub hdmmr: bool,
    pub hdtemplate: u8,
    pub hdpw: u32,
    pub hdph: u32,
    pub graymax: u32,
}

impl PatternDictParams {
    /// Parse from segment data (7 bytes after region info).
    pub fn parse(data: &[u8]) -> Option<(Self, usize)> {
        if data.len() < 7 {
            return None;
        }
        let flags = data[0];
        let hdmmr = flags & 1 != 0;
        let hdtemplate = (flags >> 1) & 3;
        let hdpw = data[1] as u32;
        let hdph = data[2] as u32;
        let graymax = u32::from_be_bytes([data[3], data[4], data[5], data[6]]);

        Some((
            PatternDictParams {
                hdmmr,
                hdtemplate,
                hdpw,
                hdph,
                graymax,
            },
            7,
        ))
    }
}

impl PatternDict {
    /// Extract individual patterns from a collective bitmap.
    /// The collective bitmap has all patterns laid out horizontally:
    /// pattern[i] occupies columns [i*hpw, (i+1)*hpw).
    pub fn from_collective(collective: &Jbig2Image, hpw: u32, hph: u32, n_patterns: u32) -> Self {
        let mut patterns = Vec::with_capacity(n_patterns as usize);
        for i in 0..n_patterns {
            let mut pat = Jbig2Image::new(hpw, hph);
            for y in 0..hph {
                for x in 0..hpw {
                    let px = collective.get_pixel(i * hpw + x, y);
                    pat.set_pixel(x, y, px);
                }
            }
            patterns.push(pat);
        }
        PatternDict {
            patterns,
            hpw,
            hph,
        }
    }
}

/// Halftone region parameters.
#[derive(Debug, Clone)]
pub struct HalftoneRegionParams {
    pub hmmr: bool,
    pub htemplate: u8,
    pub henableskip: bool,
    pub hcombop: ComposeOp,
    pub hdefpixel: bool,
    pub hgw: u32,
    pub hgh: u32,
    pub hgx: i32,
    pub hgy: i32,
    pub hrx: u16,
    pub hry: u16,
}

impl HalftoneRegionParams {
    /// Parse halftone region params from segment data (after region info, 21 bytes).
    pub fn parse(data: &[u8]) -> Option<(Self, usize)> {
        if data.len() < 21 {
            return None;
        }

        let flags = data[0];
        let hmmr = flags & 1 != 0;
        let htemplate = (flags >> 1) & 3;
        let henableskip = (flags >> 3) & 1 != 0;
        let hcombop = match (flags >> 4) & 7 {
            0 => ComposeOp::Or,
            1 => ComposeOp::And,
            2 => ComposeOp::Xor,
            3 => ComposeOp::Xnor,
            4 => ComposeOp::Replace,
            _ => ComposeOp::Or,
        };
        let hdefpixel = (flags >> 7) & 1 != 0;

        let hgw = u32::from_be_bytes([data[1], data[2], data[3], data[4]]);
        let hgh = u32::from_be_bytes([data[5], data[6], data[7], data[8]]);
        let hgx = i32::from_be_bytes([data[9], data[10], data[11], data[12]]);
        let hgy = i32::from_be_bytes([data[13], data[14], data[15], data[16]]);
        let hrx = u16::from_be_bytes([data[17], data[18]]);
        let hry = u16::from_be_bytes([data[19], data[20]]);

        Some((
            HalftoneRegionParams {
                hmmr,
                htemplate,
                henableskip,
                hcombop,
                hdefpixel,
                hgw,
                hgh,
                hgx,
                hgy,
                hrx,
                hry,
            },
            21,
        ))
    }
}

/// Decode a halftone region: place patterns on a grid using gray-scale indices.
pub fn decode_halftone_region(
    params: &HalftoneRegionParams,
    image: &mut Jbig2Image,
    pdict: &PatternDict,
    gray_vals: &[Vec<u32>],
) -> Result<()> {
    // Fill default pixel
    if params.hdefpixel {
        image.clear(1);
    }

    let n_patterns = pdict.patterns.len() as u32;

    for mg in 0..params.hgh {
        for ng in 0..params.hgw {
            // 8.8 fixed-point coordinate transform
            let x = ((params.hgx as i64) + (mg as i64) * (params.hry as i64) + (ng as i64) * (params.hrx as i64)) >> 8;
            let y = ((params.hgy as i64) + (mg as i64) * (params.hrx as i64) - (ng as i64) * (params.hry as i64)) >> 8;

            // Skip mask check
            if params.henableskip {
                let px = x;
                let py = y;
                if px + pdict.hpw as i64 <= 0 || px >= image.width as i64
                    || py + pdict.hph as i64 <= 0 || py >= image.height as i64
                {
                    continue;
                }
            }

            // Get gray value (pattern index)
            let mut gv = if (ng as usize) < gray_vals.len() && (mg as usize) < gray_vals[ng as usize].len() {
                gray_vals[ng as usize][mg as usize]
            } else {
                0
            };
            if gv >= n_patterns {
                gv = n_patterns.saturating_sub(1);
            }

            // Composite pattern onto output
            if let Some(pat) = pdict.patterns.get(gv as usize) {
                image.compose(pat, x as i32, y as i32, params.hcombop)?;
            }
        }
    }

    Ok(())
}
