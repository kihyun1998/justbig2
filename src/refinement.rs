//! Generic Refinement Region decoder (ITU T.88 6.3, 7.4.7).
//!
//! Refines a reference image pixel-by-pixel using arithmetic coding.

use crate::arith::{ArithCx, ArithState};
use crate::error::Result;
use crate::image::Jbig2Image;

/// Refinement region parameters (Table 6).
#[derive(Debug, Clone)]
pub struct RefinementRegionParams {
    /// Template (0 or 1). Template 0: 13 contexts, Template 1: 10 contexts.
    pub gr_template: u8,
    /// Reference image.
    pub reference: Jbig2Image,
    /// Reference offset X.
    pub reference_dx: i32,
    /// Reference offset Y.
    pub reference_dy: i32,
    /// Typical Prediction on.
    pub tpgron: bool,
    /// Adaptive template pixels (4 values, only used for template 0).
    pub grat: [i8; 4],
}

/// Stats size for refinement region.
pub fn refinement_stats_size(template: u8) -> usize {
    if template != 0 { 1 << 10 } else { 1 << 13 }
}

/// Decode a generic refinement region.
pub fn decode_refinement_region(
    params: &RefinementRegionParams,
    as_: &mut ArithState,
    image: &mut Jbig2Image,
    gr_stats: &mut [ArithCx],
) -> Result<()> {
    if params.tpgron {
        return decode_refinement_tpgron(params, as_, image, gr_stats);
    }

    if params.gr_template != 0 {
        decode_refinement_template1(params, as_, image, gr_stats)
    } else {
        decode_refinement_template0(params, as_, image, gr_stats)
    }
}

// --- Template 0: 13-pixel context (3 current + 10 reference) ---

fn decode_refinement_template0(
    params: &RefinementRegionParams,
    as_: &mut ArithState,
    image: &mut Jbig2Image,
    gr_stats: &mut [ArithCx],
) -> Result<()> {
    let grw = image.width;
    let grh = image.height;
    let r = &params.reference;
    let dx = params.reference_dx;
    let dy = params.reference_dy;

    for y in 0..grh {
        for x in 0..grw {
            let ix = x as i32;
            let iy = y as i32;

            let mut context: u32 = 0;
            // Current image pixels
            context |= image.get_pixel((ix - 1) as u32, y) as u32;
            context |= (image.get_pixel((ix + 1) as u32, (iy - 1) as u32) as u32) << 1;
            context |= (image.get_pixel(x, (iy - 1) as u32) as u32) << 2;
            // Adaptive pixel
            context |= (image.get_pixel(
                (ix + params.grat[0] as i32) as u32,
                (iy + params.grat[1] as i32) as u32,
            ) as u32) << 3;
            // Reference pixels
            context |= (r.get_pixel((ix - dx + 1) as u32, (iy - dy + 1) as u32) as u32) << 4;
            context |= (r.get_pixel((ix - dx) as u32, (iy - dy + 1) as u32) as u32) << 5;
            context |= (r.get_pixel((ix - dx - 1) as u32, (iy - dy + 1) as u32) as u32) << 6;
            context |= (r.get_pixel((ix - dx + 1) as u32, (iy - dy) as u32) as u32) << 7;
            context |= (r.get_pixel((ix - dx) as u32, (iy - dy) as u32) as u32) << 8;
            context |= (r.get_pixel((ix - dx - 1) as u32, (iy - dy) as u32) as u32) << 9;
            context |= (r.get_pixel((ix - dx + 1) as u32, (iy - dy - 1) as u32) as u32) << 10;
            context |= (r.get_pixel((ix - dx) as u32, (iy - dy - 1) as u32) as u32) << 11;
            // Adaptive reference pixel
            context |= (r.get_pixel(
                (ix - dx + params.grat[2] as i32) as u32,
                (iy - dy + params.grat[3] as i32) as u32,
            ) as u32) << 12;

            let bit = as_.decode(&mut gr_stats[context as usize])?;
            image.set_pixel(x, y, bit);
        }
    }
    Ok(())
}

// --- Template 1: 10-pixel context (4 current + 6 reference) ---

fn decode_refinement_template1(
    params: &RefinementRegionParams,
    as_: &mut ArithState,
    image: &mut Jbig2Image,
    gr_stats: &mut [ArithCx],
) -> Result<()> {
    let grw = image.width;
    let grh = image.height;
    let r = &params.reference;
    let dx = params.reference_dx;
    let dy = params.reference_dy;

    for y in 0..grh {
        for x in 0..grw {
            let ix = x as i32;
            let iy = y as i32;

            let mut context: u32 = 0;
            // Current image pixels
            context |= image.get_pixel((ix - 1) as u32, y) as u32;
            context |= (image.get_pixel((ix + 1) as u32, (iy - 1) as u32) as u32) << 1;
            context |= (image.get_pixel(x, (iy - 1) as u32) as u32) << 2;
            context |= (image.get_pixel((ix - 1) as u32, (iy - 1) as u32) as u32) << 3;
            // Reference pixels
            context |= (r.get_pixel((ix - dx + 1) as u32, (iy - dy + 1) as u32) as u32) << 4;
            context |= (r.get_pixel((ix - dx) as u32, (iy - dy + 1) as u32) as u32) << 5;
            context |= (r.get_pixel((ix - dx + 1) as u32, (iy - dy) as u32) as u32) << 6;
            context |= (r.get_pixel((ix - dx) as u32, (iy - dy) as u32) as u32) << 7;
            context |= (r.get_pixel((ix - dx - 1) as u32, (iy - dy) as u32) as u32) << 8;
            context |= (r.get_pixel((ix - dx) as u32, (iy - dy - 1) as u32) as u32) << 9;

            let bit = as_.decode(&mut gr_stats[context as usize])?;
            image.set_pixel(x, y, bit);
        }
    }
    Ok(())
}

// --- TPGRON: Typical Prediction for Refinement ---

/// Check if pixel (i,j) in reference has all 8 neighbors with the same value.
fn implicit_value(r: &Jbig2Image, i: i32, j: i32) -> Option<u8> {
    let m = r.get_pixel(i as u32, j as u32);
    for &(dx, dy) in &[
        (-1, -1), (0, -1), (1, -1),
        (-1, 0),           (1, 0),
        (-1, 1),  (0, 1),  (1, 1),
    ] {
        if r.get_pixel((i + dx) as u32, (j + dy) as u32) != m {
            return None;
        }
    }
    Some(m)
}

fn build_context(params: &RefinementRegionParams, image: &Jbig2Image, x: u32, y: u32) -> u32 {
    let ix = x as i32;
    let iy = y as i32;
    let r = &params.reference;
    let dx = params.reference_dx;
    let dy = params.reference_dy;

    if params.gr_template != 0 {
        // Template 1
        let mut ctx: u32 = 0;
        ctx |= image.get_pixel((ix - 1) as u32, y) as u32;
        ctx |= (image.get_pixel((ix + 1) as u32, (iy - 1) as u32) as u32) << 1;
        ctx |= (image.get_pixel(x, (iy - 1) as u32) as u32) << 2;
        ctx |= (image.get_pixel((ix - 1) as u32, (iy - 1) as u32) as u32) << 3;
        ctx |= (r.get_pixel((ix - dx + 1) as u32, (iy - dy + 1) as u32) as u32) << 4;
        ctx |= (r.get_pixel((ix - dx) as u32, (iy - dy + 1) as u32) as u32) << 5;
        ctx |= (r.get_pixel((ix - dx + 1) as u32, (iy - dy) as u32) as u32) << 6;
        ctx |= (r.get_pixel((ix - dx) as u32, (iy - dy) as u32) as u32) << 7;
        ctx |= (r.get_pixel((ix - dx - 1) as u32, (iy - dy) as u32) as u32) << 8;
        ctx |= (r.get_pixel((ix - dx) as u32, (iy - dy - 1) as u32) as u32) << 9;
        ctx
    } else {
        // Template 0
        let mut ctx: u32 = 0;
        ctx |= image.get_pixel((ix - 1) as u32, y) as u32;
        ctx |= (image.get_pixel((ix + 1) as u32, (iy - 1) as u32) as u32) << 1;
        ctx |= (image.get_pixel(x, (iy - 1) as u32) as u32) << 2;
        ctx |= (image.get_pixel((ix + params.grat[0] as i32) as u32, (iy + params.grat[1] as i32) as u32) as u32) << 3;
        ctx |= (r.get_pixel((ix - dx + 1) as u32, (iy - dy + 1) as u32) as u32) << 4;
        ctx |= (r.get_pixel((ix - dx) as u32, (iy - dy + 1) as u32) as u32) << 5;
        ctx |= (r.get_pixel((ix - dx - 1) as u32, (iy - dy + 1) as u32) as u32) << 6;
        ctx |= (r.get_pixel((ix - dx + 1) as u32, (iy - dy) as u32) as u32) << 7;
        ctx |= (r.get_pixel((ix - dx) as u32, (iy - dy) as u32) as u32) << 8;
        ctx |= (r.get_pixel((ix - dx - 1) as u32, (iy - dy) as u32) as u32) << 9;
        ctx |= (r.get_pixel((ix - dx + 1) as u32, (iy - dy - 1) as u32) as u32) << 10;
        ctx |= (r.get_pixel((ix - dx) as u32, (iy - dy - 1) as u32) as u32) << 11;
        ctx |= (r.get_pixel((ix - dx + params.grat[2] as i32) as u32, (iy - dy + params.grat[3] as i32) as u32) as u32) << 12;
        ctx
    }
}

fn decode_refinement_tpgron(
    params: &RefinementRegionParams,
    as_: &mut ArithState,
    image: &mut Jbig2Image,
    gr_stats: &mut [ArithCx],
) -> Result<()> {
    let grw = image.width;
    let grh = image.height;
    let start_context: u32 = if params.gr_template != 0 { 0x40 } else { 0x100 };
    let mut ltp = 0u8;

    for y in 0..grh {
        let bit = as_.decode(&mut gr_stats[start_context as usize])?;
        ltp ^= bit;

        if ltp == 0 {
            // Decode row normally
            for x in 0..grw {
                let ctx = build_context(params, image, x, y);
                let bit = as_.decode(&mut gr_stats[ctx as usize])?;
                image.set_pixel(x, y, bit);
            }
        } else {
            // Use implicit values where possible
            for x in 0..grw {
                let ix = x as i32;
                let iy = y as i32;
                let ri = ix - params.reference_dx;
                let rj = iy - params.reference_dy;

                if let Some(v) = implicit_value(&params.reference, ri, rj) {
                    image.set_pixel(x, y, v);
                } else {
                    let ctx = build_context(params, image, x, y);
                    let bit = as_.decode(&mut gr_stats[ctx as usize])?;
                    image.set_pixel(x, y, bit);
                }
            }
        }
    }
    Ok(())
}
