//! Text Region decoder (ITU T.88 6.4, 7.4.3).
//!
//! Places glyph instances from symbol dictionaries onto an output image.

use crate::arith::ArithState;
use crate::arith_iaid::ArithIaidCtx;
use crate::arith_int::ArithIntCtx;
use crate::error::{Jbig2Error, Result};
use crate::image::{ComposeOp, Jbig2Image};
use crate::symbol_dict::SymbolDict;

/// Reference corner for glyph placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RefCorner {
    BottomLeft = 0,
    TopLeft = 1,
    BottomRight = 2,
    TopRight = 3,
}

impl RefCorner {
    pub fn from_u8(v: u8) -> Self {
        match v & 3 {
            0 => Self::BottomLeft,
            1 => Self::TopLeft,
            2 => Self::BottomRight,
            3 => Self::TopRight,
            _ => unreachable!(),
        }
    }
}

/// Text region parameters (Table 9).
#[derive(Debug, Clone)]
pub struct TextRegionParams {
    pub sbhuff: bool,
    pub sbrefine: bool,
    pub sbdefpixel: bool,
    pub sbcombop: ComposeOp,
    pub transposed: bool,
    pub refcorner: RefCorner,
    pub sbdsoffset: i32,
    pub sbnuminstances: u32,
    pub logsbstrips: u8,
    pub sbstrips: u32,
    pub sbrtemplate: u8,
    pub sbrat: [i8; 4],
}

impl TextRegionParams {
    /// Parse text region flags from segment data at the given offset (after region info).
    /// Returns (params, bytes consumed).
    pub fn parse(data: &[u8]) -> Option<(Self, usize)> {
        if data.len() < 2 {
            return None;
        }

        let flags = u16::from_be_bytes([data[0], data[1]]);
        let sbhuff = flags & 0x0001 != 0;
        let sbrefine = (flags & 0x0002) != 0;
        let logsbstrips = ((flags & 0x000C) >> 2) as u8;
        let sbstrips = 1u32 << logsbstrips;
        let refcorner = RefCorner::from_u8(((flags >> 4) & 3) as u8);
        let transposed = (flags & 0x0040) != 0;
        let sbcombop = match (flags >> 7) & 3 {
            0 => ComposeOp::Or,
            1 => ComposeOp::And,
            2 => ComposeOp::Xor,
            3 => ComposeOp::Xnor,
            _ => ComposeOp::Or,
        };
        let sbdefpixel = (flags & 0x0200) != 0;
        // SBDSOFFSET is a signed 5-bit value
        let raw_offset = ((flags >> 10) & 0x1F) as i32;
        let sbdsoffset = if raw_offset > 15 { raw_offset - 32 } else { raw_offset };
        let sbrtemplate = ((flags >> 15) & 1) as u8;

        let mut offset = 2;

        // If huffman, skip huffman flags (2 bytes)
        if sbhuff {
            if data.len() < offset + 2 {
                return None;
            }
            // TODO: parse huffman table selections
            offset += 2;
        }

        // If arithmetic + refinement + template 0, read SBRAT (4 bytes)
        let mut sbrat = [0i8; 4];
        if !sbhuff && sbrefine && sbrtemplate == 0 {
            if data.len() < offset + 4 {
                return None;
            }
            for i in 0..4 {
                sbrat[i] = data[offset + i] as i8;
            }
            offset += 4;
        }

        // SBNUMINSTANCES (4 bytes)
        if data.len() < offset + 4 {
            return None;
        }
        let sbnuminstances = u32::from_be_bytes([
            data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
        ]);
        offset += 4;

        Some((
            TextRegionParams {
                sbhuff,
                sbrefine,
                sbdefpixel,
                sbcombop,
                transposed,
                refcorner,
                sbdsoffset,
                sbnuminstances,
                logsbstrips,
                sbstrips,
                sbrtemplate,
                sbrat,
            },
            offset,
        ))
    }
}

/// Decode a text region using arithmetic coding (6.4.5).
pub fn decode_text_region(
    params: &TextRegionParams,
    as_: &mut ArithState,
    image: &mut Jbig2Image,
    dicts: &[&SymbolDict],
    sbnumsyms: u32,
) -> Result<()> {
    if params.sbhuff {
        return Err(Jbig2Error::UnsupportedFeature(
            "huffman text region decoding".into(),
        ));
    }

    // Fill default pixel
    if params.sbdefpixel {
        image.clear(1);
    }

    // Create arithmetic contexts
    let mut iadt = ArithIntCtx::new();
    let mut iafs = ArithIntCtx::new();
    let mut iads = ArithIntCtx::new();
    let mut iait = ArithIntCtx::new();
    let mut iari = ArithIntCtx::new();

    let sbsymcodelen = {
        let mut n = 0u8;
        while (1u64 << n) < sbnumsyms as u64 {
            n += 1;
        }
        n
    };
    let mut iaid = ArithIaidCtx::new(sbsymcodelen)?;

    let mut stript: i32 = 0;
    let mut firsts: i32 = 0;
    let mut ninstances: u32 = 0;

    // 6.4.5 (3)
    while ninstances < params.sbnuminstances {
        // 6.4.5 (3b): Decode DT
        let dt = iadt.decode(as_)?.unwrap_or(0);
        stript += dt * params.sbstrips as i32;

        let mut first_s = true;
        let mut curs: i32 = 0;

        // 6.4.5 (3c): Inner loop — symbols within strip
        loop {
            if first_s {
                // 6.4.7: First S
                let dfs = iafs.decode(as_)?.unwrap_or(0);
                firsts += dfs;
                curs = firsts;
                first_s = false;
            } else {
                // 6.4.8: Subsequent S
                match iads.decode(as_)? {
                    None => break, // OOB → end of strip
                    Some(ids) => {
                        curs += ids + params.sbdsoffset;
                    }
                }
            }

            // 6.4.9: T within strip
            let curt = if params.sbstrips == 1 {
                0i32
            } else {
                iait.decode(as_)?.unwrap_or(0)
            };
            let t = stript + curt;

            // 6.4.10: Symbol ID
            let id = iaid.decode(as_)?;

            // Look up glyph
            let ib = lookup_glyph(dicts, id);

            // 6.4.11: Refinement check (simplified — skip actual refinement)
            let ri = if params.sbrefine {
                iari.decode(as_)?.unwrap_or(0) != 0
            } else {
                false
            };
            let _ = ri; // TODO: actual refinement

            // 6.4.11 (6): Pre-placement S adjustment
            if let Some(ref g) = ib {
                if !params.transposed && params.refcorner as u8 > 1 {
                    curs += g.width as i32 - 1;
                } else if params.transposed && (params.refcorner as u8 & 1) == 0 {
                    curs += g.height as i32 - 1;
                }
            }

            let s = curs;

            // 6.4.11 (8): Compute final (x, y)
            if let Some(ref g) = ib {
                let (x, y) = compute_placement(params, s, t, g.width, g.height);
                image.compose(g, x, y, params.sbcombop)?;
            }

            // 6.4.11 (10): Post-placement S adjustment
            if let Some(ref g) = ib {
                if !params.transposed && (params.refcorner as u8) < 2 {
                    curs += g.width as i32 - 1;
                } else if params.transposed && (params.refcorner as u8 & 1) != 0 {
                    curs += g.height as i32 - 1;
                }
            }

            ninstances += 1;
            if ninstances >= params.sbnuminstances {
                break;
            }
        }
    }

    Ok(())
}

/// Look up a glyph across concatenated dictionaries.
fn lookup_glyph(dicts: &[&SymbolDict], id: u32) -> Option<Jbig2Image> {
    let mut remaining = id;
    for dict in dicts {
        if remaining < dict.n_symbols() {
            return dict.glyph(remaining).cloned();
        }
        remaining -= dict.n_symbols();
    }
    None
}

/// Compute final (x, y) placement from S, T, TRANSPOSED, and REFCORNER.
fn compute_placement(
    params: &TextRegionParams,
    s: i32,
    t: i32,
    w: u32,
    h: u32,
) -> (i32, i32) {
    let w = w as i32;
    let h = h as i32;

    if !params.transposed {
        match params.refcorner {
            RefCorner::TopLeft => (s, t),
            RefCorner::TopRight => (s - w + 1, t),
            RefCorner::BottomLeft => (s, t - h + 1),
            RefCorner::BottomRight => (s - w + 1, t - h + 1),
        }
    } else {
        match params.refcorner {
            RefCorner::TopLeft => (t, s),
            RefCorner::TopRight => (t - w + 1, s),
            RefCorner::BottomLeft => (t, s - h + 1),
            RefCorner::BottomRight => (t - w + 1, s - h + 1),
        }
    }
}
