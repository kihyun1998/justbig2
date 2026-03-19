//! Generic region decoder (ITU T.88 6.2, 7.4.6).
//!
//! Supports Templates 0-3 with optimized and unoptimized paths,
//! plus TPGD (Typical Prediction).

use crate::arith::{ArithCx, ArithState};
use crate::error::{Jbig2Error, Result};
use crate::image::Jbig2Image;

/// Generic region decoding parameters (Table 2).
#[derive(Debug, Clone)]
pub struct GenericRegionParams {
    pub mmr: bool,
    pub gb_template: u8,
    pub tpgdon: bool,
    pub use_skip: bool,
    pub gbat: [i8; 8],
}

impl GenericRegionParams {
    /// Parse from segment data at offset 17 (after region segment info).
    /// Returns (params, bytes consumed for gbat).
    pub fn parse(flags_byte: u8) -> (Self, usize) {
        let mmr = flags_byte & 1 != 0;
        let gb_template = (flags_byte >> 1) & 3;
        let tpgdon = (flags_byte >> 3) & 1 != 0;

        let gbat_size = if mmr { 0 } else if gb_template == 0 { 8 } else { 2 };

        (
            GenericRegionParams {
                mmr,
                gb_template,
                tpgdon,
                use_skip: false,
                gbat: [0i8; 8],
            },
            gbat_size,
        )
    }

    /// Fill GBAT from data slice.
    pub fn set_gbat(&mut self, data: &[u8]) {
        let n = if self.gb_template == 0 { 8 } else { 2 };
        for i in 0..n.min(data.len()) {
            self.gbat[i] = data[i] as i8;
        }
    }
}

/// Return the context stats array size for the given template.
pub fn stats_size(template: u8) -> usize {
    match template {
        0 => 1 << 16,
        1 => 1 << 13,
        _ => 1 << 10, // templates 2 and 3
    }
}

/// Default (nominal) GBAT values for each template.
const DEFAULT_GBAT_T0: [i8; 8] = [3, -1, -3, -1, 2, -2, -2, -2];
const DEFAULT_GBAT_T1: [i8; 2] = [3, -1];
const DEFAULT_GBAT_T2: [i8; 2] = [2, -1];
const DEFAULT_GBAT_T3: [i8; 2] = [2, -1];

/// Decode a generic region using arithmetic coding.
pub fn decode_generic_region(
    params: &GenericRegionParams,
    as_: &mut ArithState,
    image: &mut Jbig2Image,
    gb_stats: &mut [ArithCx],
) -> Result<()> {
    if params.tpgdon {
        return decode_tpgd(params, as_, image, gb_stats);
    }

    match params.gb_template {
        0 => {
            if !params.use_skip && params.gbat[..8] == DEFAULT_GBAT_T0 {
                decode_template0_opt(as_, image, gb_stats)
            } else {
                decode_template0_unopt(params, as_, image, gb_stats)
            }
        }
        1 => {
            if !params.use_skip && params.gbat[..2] == DEFAULT_GBAT_T1 {
                decode_template1_opt(as_, image, gb_stats)
            } else {
                decode_template1_unopt(params, as_, image, gb_stats)
            }
        }
        2 => {
            if !params.use_skip && params.gbat[..2] == DEFAULT_GBAT_T2 {
                decode_template2_opt(as_, image, gb_stats)
            } else {
                decode_template2_unopt(params, as_, image, gb_stats)
            }
        }
        3 => {
            if !params.use_skip && params.gbat[..2] == DEFAULT_GBAT_T3 {
                decode_template3_opt(as_, image, gb_stats)
            } else {
                decode_template3_unopt(params, as_, image, gb_stats)
            }
        }
        _ => Err(Jbig2Error::InvalidData("invalid GBTEMPLATE".into())),
    }
}

// =============================================================================
// Template 0: 16-pixel context (optimized)
// =============================================================================

fn decode_template0_opt(
    as_: &mut ArithState,
    image: &mut Jbig2Image,
    gb_stats: &mut [ArithCx],
) -> Result<()> {
    let gbw = image.width;
    let gbh = image.height;
    let rowstride = image.stride;

    if gbw == 0 {
        return Ok(());
    }

    for y in 0..gbh {
        let padded_width = (gbw + 7) & !7;

        let line_m1_init: u32 = if y >= 1 {
            image.data[((y - 1) * rowstride) as usize] as u32
        } else {
            0
        };
        let line_m2_init: u32 = if y >= 2 {
            (image.data[((y - 2) * rowstride) as usize] as u32) << 6
        } else {
            0
        };

        let mut line_m1 = line_m1_init;
        let mut line_m2 = line_m2_init;
        let mut context = (line_m1 & 0x7f0) | (line_m2 & 0xf800);

        for x in (0..padded_width).step_by(8) {
            let minor_width = if gbw - x > 8 { 8 } else { gbw - x };

            if y >= 1 {
                let next = if x + 8 < gbw {
                    image.data[((y - 1) * rowstride + (x >> 3) + 1) as usize] as u32
                } else {
                    0
                };
                line_m1 = (line_m1 << 8) | next;
            }
            if y >= 2 {
                let next = if x + 8 < gbw {
                    (image.data[((y - 2) * rowstride + (x >> 3) + 1) as usize] as u32) << 6
                } else {
                    0
                };
                line_m2 = (line_m2 << 8) | next;
            }

            let mut result = 0u8;
            for x_minor in 0..minor_width {
                let bit = as_.decode(&mut gb_stats[context as usize])? as u32;
                result |= (bit as u8) << (7 - x_minor);
                context = ((context & 0x7bf7) << 1) | bit
                    | ((line_m1 >> (7 - x_minor)) & 0x10)
                    | ((line_m2 >> (7 - x_minor)) & 0x800);
            }
            image.data[(y * rowstride + (x >> 3)) as usize] = result;
        }
    }
    Ok(())
}

fn decode_template0_unopt(
    params: &GenericRegionParams,
    as_: &mut ArithState,
    image: &mut Jbig2Image,
    gb_stats: &mut [ArithCx],
) -> Result<()> {
    let gbw = image.width;
    let gbh = image.height;

    for y in 0..gbh {
        for x in 0..gbw {
            let mut context: u32 = 0;
            // Bits 0-3: pixels at (x-1,y), (x-2,y), (x-3,y), (x-4,y)
            for i in 0..4u32 {
                context |= (image.get_pixel(x.wrapping_sub(1 + i), y) as u32) << i;
            }
            // Bit 4: adaptive pixel
            context |= (image.get_pixel(
                (x as i32 + params.gbat[0] as i32) as u32,
                (y as i32 + params.gbat[1] as i32) as u32,
            ) as u32) << 4;
            // Bits 5-9: pixels from row y-1
            for i in 0..5u32 {
                context |= (image.get_pixel(x.wrapping_sub(2).wrapping_add(i), y.wrapping_sub(1)) as u32) << (5 + i);
            }
            // Bits 10-11: adaptive pixels
            context |= (image.get_pixel(
                (x as i32 + params.gbat[2] as i32) as u32,
                (y as i32 + params.gbat[3] as i32) as u32,
            ) as u32) << 10;
            context |= (image.get_pixel(
                (x as i32 + params.gbat[4] as i32) as u32,
                (y as i32 + params.gbat[5] as i32) as u32,
            ) as u32) << 11;
            // Bits 12-14: pixels from row y-2
            for i in 0..3u32 {
                context |= (image.get_pixel(x.wrapping_sub(1).wrapping_add(i), y.wrapping_sub(2)) as u32) << (12 + i);
            }
            // Bit 15: adaptive pixel
            context |= (image.get_pixel(
                (x as i32 + params.gbat[6] as i32) as u32,
                (y as i32 + params.gbat[7] as i32) as u32,
            ) as u32) << 15;

            let bit = as_.decode(&mut gb_stats[context as usize])?;
            image.set_pixel(x, y, bit);
        }
    }
    Ok(())
}

// =============================================================================
// Template 1: 13-pixel context
// =============================================================================

fn decode_template1_opt(
    as_: &mut ArithState,
    image: &mut Jbig2Image,
    gb_stats: &mut [ArithCx],
) -> Result<()> {
    let gbw = image.width;
    let gbh = image.height;
    let rowstride = image.stride;

    if gbw == 0 { return Ok(()); }

    for y in 0..gbh {
        let padded_width = (gbw + 7) & !7;
        let mut line_m1: u32 = if y >= 1 { image.data[((y - 1) * rowstride) as usize] as u32 } else { 0 };
        let mut line_m2: u32 = if y >= 2 { (image.data[((y - 2) * rowstride) as usize] as u32) << 5 } else { 0 };
        let mut context = ((line_m1 >> 1) & 0x1f8) | ((line_m2 >> 1) & 0x1e00);

        for x in (0..padded_width).step_by(8) {
            let minor_width = if gbw - x > 8 { 8 } else { gbw - x };
            if y >= 1 {
                let next = if x + 8 < gbw { image.data[((y - 1) * rowstride + (x >> 3) + 1) as usize] as u32 } else { 0 };
                line_m1 = (line_m1 << 8) | next;
            }
            if y >= 2 {
                let next = if x + 8 < gbw { (image.data[((y - 2) * rowstride + (x >> 3) + 1) as usize] as u32) << 5 } else { 0 };
                line_m2 = (line_m2 << 8) | next;
            }

            let mut result = 0u8;
            for x_minor in 0..minor_width {
                let bit = as_.decode(&mut gb_stats[context as usize])? as u32;
                result |= (bit as u8) << (7 - x_minor);
                context = ((context & 0xefb) << 1) | bit
                    | ((line_m1 >> (8 - x_minor)) & 0x8)
                    | ((line_m2 >> (8 - x_minor)) & 0x200);
            }
            image.data[(y * rowstride + (x >> 3)) as usize] = result;
        }
    }
    Ok(())
}

fn decode_template1_unopt(
    params: &GenericRegionParams,
    as_: &mut ArithState,
    image: &mut Jbig2Image,
    gb_stats: &mut [ArithCx],
) -> Result<()> {
    let gbw = image.width;
    let gbh = image.height;

    for y in 0..gbh {
        for x in 0..gbw {
            let mut context: u32 = 0;
            // Bits 0-2: (x-1,y), (x-2,y), (x-3,y)
            for i in 0..3u32 {
                context |= (image.get_pixel(x.wrapping_sub(1 + i), y) as u32) << i;
            }
            // Bit 3: adaptive
            context |= (image.get_pixel(
                (x as i32 + params.gbat[0] as i32) as u32,
                (y as i32 + params.gbat[1] as i32) as u32,
            ) as u32) << 3;
            // Bits 4-8: row y-1
            for i in 0..5u32 {
                context |= (image.get_pixel(x.wrapping_sub(2).wrapping_add(i), y.wrapping_sub(1)) as u32) << (4 + i);
            }
            // Bits 9-12: row y-2
            for i in 0..4u32 {
                context |= (image.get_pixel(x.wrapping_sub(1).wrapping_add(i), y.wrapping_sub(2)) as u32) << (9 + i);
            }

            let bit = as_.decode(&mut gb_stats[context as usize])?;
            image.set_pixel(x, y, bit);
        }
    }
    Ok(())
}

// =============================================================================
// Template 2: 10-pixel context
// =============================================================================

fn decode_template2_opt(
    as_: &mut ArithState,
    image: &mut Jbig2Image,
    gb_stats: &mut [ArithCx],
) -> Result<()> {
    let gbw = image.width;
    let gbh = image.height;
    let rowstride = image.stride;

    if gbw == 0 { return Ok(()); }

    for y in 0..gbh {
        let padded_width = (gbw + 7) & !7;
        let mut line_m1: u32 = if y >= 1 { image.data[((y - 1) * rowstride) as usize] as u32 } else { 0 };
        let mut line_m2: u32 = if y >= 2 { (image.data[((y - 2) * rowstride) as usize] as u32) << 4 } else { 0 };
        let mut context = ((line_m1 >> 3) & 0x7c) | ((line_m2 >> 3) & 0x380);

        for x in (0..padded_width).step_by(8) {
            let minor_width = if gbw - x > 8 { 8 } else { gbw - x };
            if y >= 1 {
                let next = if x + 8 < gbw { image.data[((y - 1) * rowstride + (x >> 3) + 1) as usize] as u32 } else { 0 };
                line_m1 = (line_m1 << 8) | next;
            }
            if y >= 2 {
                let next = if x + 8 < gbw { (image.data[((y - 2) * rowstride + (x >> 3) + 1) as usize] as u32) << 4 } else { 0 };
                line_m2 = (line_m2 << 8) | next;
            }

            let mut result = 0u8;
            for x_minor in 0..minor_width {
                let bit = as_.decode(&mut gb_stats[context as usize])? as u32;
                result |= (bit as u8) << (7 - x_minor);
                context = ((context & 0x1bd) << 1) | bit
                    | ((line_m1 >> (10 - x_minor)) & 0x4)
                    | ((line_m2 >> (10 - x_minor)) & 0x80);
            }
            image.data[(y * rowstride + (x >> 3)) as usize] = result;
        }
    }
    Ok(())
}

fn decode_template2_unopt(
    params: &GenericRegionParams,
    as_: &mut ArithState,
    image: &mut Jbig2Image,
    gb_stats: &mut [ArithCx],
) -> Result<()> {
    let gbw = image.width;
    let gbh = image.height;

    for y in 0..gbh {
        for x in 0..gbw {
            let mut context: u32 = 0;
            // Bits 0-1: (x-1,y), (x-2,y)
            context |= image.get_pixel(x.wrapping_sub(1), y) as u32;
            context |= (image.get_pixel(x.wrapping_sub(2), y) as u32) << 1;
            // Bit 2: adaptive
            context |= (image.get_pixel(
                (x as i32 + params.gbat[0] as i32) as u32,
                (y as i32 + params.gbat[1] as i32) as u32,
            ) as u32) << 2;
            // Bits 3-6: row y-1
            for i in 0..4u32 {
                context |= (image.get_pixel(x.wrapping_sub(1).wrapping_add(i), y.wrapping_sub(1)) as u32) << (3 + i);
            }
            // Bits 7-9: row y-2
            for i in 0..3u32 {
                context |= (image.get_pixel(x.wrapping_sub(1).wrapping_add(i), y.wrapping_sub(2)) as u32) << (7 + i);
            }

            let bit = as_.decode(&mut gb_stats[context as usize])?;
            image.set_pixel(x, y, bit);
        }
    }
    Ok(())
}

// =============================================================================
// Template 3: 10-pixel context (single row reference)
// =============================================================================

fn decode_template3_opt(
    as_: &mut ArithState,
    image: &mut Jbig2Image,
    gb_stats: &mut [ArithCx],
) -> Result<()> {
    let gbw = image.width;
    let gbh = image.height;
    let rowstride = image.stride;

    if gbw == 0 { return Ok(()); }

    for y in 0..gbh {
        let padded_width = (gbw + 7) & !7;
        let mut line_m1: u32 = if y >= 1 { image.data[((y - 1) * rowstride) as usize] as u32 } else { 0 };
        let mut context = (line_m1 >> 1) & 0x3f0;

        for x in (0..padded_width).step_by(8) {
            let minor_width = if gbw - x > 8 { 8 } else { gbw - x };
            if y >= 1 {
                let next = if x + 8 < gbw { image.data[((y - 1) * rowstride + (x >> 3) + 1) as usize] as u32 } else { 0 };
                line_m1 = (line_m1 << 8) | next;
            }

            let mut result = 0u8;
            for x_minor in 0..minor_width {
                let bit = as_.decode(&mut gb_stats[context as usize])? as u32;
                result |= (bit as u8) << (7 - x_minor);
                context = ((context & 0x1f7) << 1) | bit
                    | ((line_m1 >> (8 - x_minor)) & 0x10);
            }
            image.data[(y * rowstride + (x >> 3)) as usize] = result;
        }
    }
    Ok(())
}

fn decode_template3_unopt(
    params: &GenericRegionParams,
    as_: &mut ArithState,
    image: &mut Jbig2Image,
    gb_stats: &mut [ArithCx],
) -> Result<()> {
    let gbw = image.width;
    let gbh = image.height;

    for y in 0..gbh {
        for x in 0..gbw {
            let mut context: u32 = 0;
            // Bits 0-3: (x-1,y) through (x-4,y)
            for i in 0..4u32 {
                context |= (image.get_pixel(x.wrapping_sub(1 + i), y) as u32) << i;
            }
            // Bit 4: adaptive
            context |= (image.get_pixel(
                (x as i32 + params.gbat[0] as i32) as u32,
                (y as i32 + params.gbat[1] as i32) as u32,
            ) as u32) << 4;
            // Bits 5-9: row y-1
            for i in 0..5u32 {
                context |= (image.get_pixel(x.wrapping_sub(2).wrapping_add(i), y.wrapping_sub(1)) as u32) << (5 + i);
            }

            let bit = as_.decode(&mut gb_stats[context as usize])?;
            image.set_pixel(x, y, bit);
        }
    }
    Ok(())
}

// =============================================================================
// TPGD: Typical Prediction
// =============================================================================

/// TPGD decision context values per template.
const TPGD_CONTEXT: [u32; 4] = [0x9B25, 0x0795, 0xE5, 0x0195];

fn copy_prev_row(image: &mut Jbig2Image, y: u32) {
    if y == 0 {
        let start = 0usize;
        let end = image.stride as usize;
        image.data[start..end].fill(0);
    } else {
        let stride = image.stride as usize;
        let src_start = ((y - 1) * image.stride) as usize;
        let dst_start = (y * image.stride) as usize;
        // Safe copy via temporary
        let row: Vec<u8> = image.data[src_start..src_start + stride].to_vec();
        image.data[dst_start..dst_start + stride].copy_from_slice(&row);
    }
}

fn decode_tpgd(
    params: &GenericRegionParams,
    as_: &mut ArithState,
    image: &mut Jbig2Image,
    gb_stats: &mut [ArithCx],
) -> Result<()> {
    let gbh = image.height;
    let tpgd_cx_idx = params.gb_template.min(3) as usize;
    let mut ltp = 0u8;

    for y in 0..gbh {
        let bit = as_.decode(&mut gb_stats[TPGD_CONTEXT[tpgd_cx_idx] as usize])?;
        ltp ^= bit;

        if ltp != 0 {
            copy_prev_row(image, y);
        } else {
            // Decode row normally using the appropriate template
            decode_single_row(params, as_, image, gb_stats, y)?;
        }
    }
    Ok(())
}

/// Decode a single row using the appropriate template (unoptimized, for TPGD).
fn decode_single_row(
    params: &GenericRegionParams,
    as_: &mut ArithState,
    image: &mut Jbig2Image,
    gb_stats: &mut [ArithCx],
    y: u32,
) -> Result<()> {
    let gbw = image.width;

    for x in 0..gbw {
        let context = match params.gb_template {
            0 => build_context_t0(image, params, x, y),
            1 => build_context_t1(image, params, x, y),
            2 => build_context_t2(image, params, x, y),
            3 => build_context_t3(image, params, x, y),
            _ => 0,
        };
        let bit = as_.decode(&mut gb_stats[context as usize])?;
        image.set_pixel(x, y, bit);
    }
    Ok(())
}

fn build_context_t0(image: &Jbig2Image, params: &GenericRegionParams, x: u32, y: u32) -> u32 {
    let mut ctx: u32 = 0;
    for i in 0..4u32 {
        ctx |= (image.get_pixel(x.wrapping_sub(1 + i), y) as u32) << i;
    }
    ctx |= (image.get_pixel((x as i32 + params.gbat[0] as i32) as u32, (y as i32 + params.gbat[1] as i32) as u32) as u32) << 4;
    for i in 0..5u32 {
        ctx |= (image.get_pixel(x.wrapping_sub(2).wrapping_add(i), y.wrapping_sub(1)) as u32) << (5 + i);
    }
    ctx |= (image.get_pixel((x as i32 + params.gbat[2] as i32) as u32, (y as i32 + params.gbat[3] as i32) as u32) as u32) << 10;
    ctx |= (image.get_pixel((x as i32 + params.gbat[4] as i32) as u32, (y as i32 + params.gbat[5] as i32) as u32) as u32) << 11;
    for i in 0..3u32 {
        ctx |= (image.get_pixel(x.wrapping_sub(1).wrapping_add(i), y.wrapping_sub(2)) as u32) << (12 + i);
    }
    ctx |= (image.get_pixel((x as i32 + params.gbat[6] as i32) as u32, (y as i32 + params.gbat[7] as i32) as u32) as u32) << 15;
    ctx
}

fn build_context_t1(image: &Jbig2Image, params: &GenericRegionParams, x: u32, y: u32) -> u32 {
    let mut ctx: u32 = 0;
    for i in 0..3u32 {
        ctx |= (image.get_pixel(x.wrapping_sub(1 + i), y) as u32) << i;
    }
    ctx |= (image.get_pixel((x as i32 + params.gbat[0] as i32) as u32, (y as i32 + params.gbat[1] as i32) as u32) as u32) << 3;
    for i in 0..5u32 {
        ctx |= (image.get_pixel(x.wrapping_sub(2).wrapping_add(i), y.wrapping_sub(1)) as u32) << (4 + i);
    }
    for i in 0..4u32 {
        ctx |= (image.get_pixel(x.wrapping_sub(1).wrapping_add(i), y.wrapping_sub(2)) as u32) << (9 + i);
    }
    ctx
}

fn build_context_t2(image: &Jbig2Image, params: &GenericRegionParams, x: u32, y: u32) -> u32 {
    let mut ctx: u32 = 0;
    ctx |= image.get_pixel(x.wrapping_sub(1), y) as u32;
    ctx |= (image.get_pixel(x.wrapping_sub(2), y) as u32) << 1;
    ctx |= (image.get_pixel((x as i32 + params.gbat[0] as i32) as u32, (y as i32 + params.gbat[1] as i32) as u32) as u32) << 2;
    for i in 0..4u32 {
        ctx |= (image.get_pixel(x.wrapping_sub(1).wrapping_add(i), y.wrapping_sub(1)) as u32) << (3 + i);
    }
    for i in 0..3u32 {
        ctx |= (image.get_pixel(x.wrapping_sub(1).wrapping_add(i), y.wrapping_sub(2)) as u32) << (7 + i);
    }
    ctx
}

fn build_context_t3(image: &Jbig2Image, params: &GenericRegionParams, x: u32, y: u32) -> u32 {
    let mut ctx: u32 = 0;
    for i in 0..4u32 {
        ctx |= (image.get_pixel(x.wrapping_sub(1 + i), y) as u32) << i;
    }
    ctx |= (image.get_pixel((x as i32 + params.gbat[0] as i32) as u32, (y as i32 + params.gbat[1] as i32) as u32) as u32) << 4;
    for i in 0..5u32 {
        ctx |= (image.get_pixel(x.wrapping_sub(2).wrapping_add(i), y.wrapping_sub(1)) as u32) << (5 + i);
    }
    ctx
}
