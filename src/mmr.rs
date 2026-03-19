//! MMR (Modified Modified Read / CCITT Group 4) decoder.
//!
//! Ported from jbig2dec/jbig2_mmr.c.

use crate::error::{Jbig2Error, Result};
use crate::image::Jbig2Image;

const MINUS1: u32 = u32::MAX;

// --- Bit-level MMR context ---

struct MmrCtx<'a> {
    data: &'a [u8],
    width: u32,
    data_index: usize,
    bit_index: u32,
    word: u32,
    consumed_bits: usize,
}

impl<'a> MmrCtx<'a> {
    fn new(width: u32, data: &'a [u8]) -> Self {
        let mut ctx = MmrCtx {
            data,
            width,
            data_index: 0,
            bit_index: 32,
            word: 0,
            consumed_bits: 0,
        };
        while ctx.bit_index >= 8 && ctx.data_index < ctx.data.len() {
            ctx.bit_index -= 8;
            ctx.word |= (ctx.data[ctx.data_index] as u32) << ctx.bit_index;
            ctx.data_index += 1;
        }
        ctx
    }

    fn consume(&mut self, n: u32) {
        self.consumed_bits += n as usize;
        let max = self.data.len() * 8;
        if self.consumed_bits > max {
            self.consumed_bits = max;
        }
        self.word <<= n;
        self.bit_index += n;
        while self.bit_index >= 8 && self.data_index < self.data.len() {
            self.bit_index -= 8;
            self.word |= (self.data[self.data_index] as u32) << self.bit_index;
            self.data_index += 1;
        }
    }

    fn peek(&self, n: u32) -> u32 {
        self.word >> (32 - n)
    }

    fn consumed_bytes(&self) -> usize {
        (self.consumed_bits + 7) / 8
    }
}

// --- Lookup table node ---

#[derive(Clone, Copy)]
struct MmrNode {
    val: i16,
    n_bits: i16,
}

const ERROR: i16 = -1;
const ZEROES: i16 = -2;
const UNCOMPRESSED: i16 = -3;

// --- Lookup table decoding ---

fn get_code(mmr: &mut MmrCtx, table: &[MmrNode], initial_bits: u32) -> i16 {
    let table_ix = (mmr.word >> (32 - initial_bits)) as usize;
    if table_ix >= table.len() {
        return ERROR;
    }
    let mut val = table[table_ix].val;
    let mut n_bits = table[table_ix].n_bits as u32;

    if n_bits > initial_bits {
        // Multi-level lookup
        let mask = (1u32 << (32 - initial_bits)) - 1;
        let remaining = (mmr.word & mask) >> (32 - n_bits);
        let sub_ix = val as usize + remaining as usize;
        if sub_ix >= table.len() {
            return ERROR;
        }
        val = table[sub_ix].val;
        n_bits = initial_bits + table[sub_ix].n_bits as u32;
    }

    mmr.consume(n_bits);
    val
}

fn get_run(mmr: &mut MmrCtx, table: &[MmrNode], initial_bits: u32) -> Result<u32> {
    let mut total = 0u32;
    loop {
        let val = get_code(mmr, table, initial_bits);
        if val == ERROR || val == UNCOMPRESSED || val == ZEROES {
            return Err(Jbig2Error::InvalidData("invalid MMR run code".into()));
        }
        total += val as u32;
        if val < 64 {
            break;
        }
    }
    Ok(total)
}

// --- Bit manipulation helpers ---

fn getbit(line: &[u8], x: u32) -> u8 {
    let byte_idx = (x >> 3) as usize;
    if byte_idx >= line.len() {
        return 0;
    }
    (line[byte_idx] >> (7 - (x & 7))) & 1
}

fn find_changing_element(line: Option<&[u8]>, x: u32, w: u32) -> u32 {
    let line = match line {
        Some(l) => l,
        None => return w,
    };

    let (a, mut x) = if x == MINUS1 {
        (0u8, 0u32)
    } else if x < w {
        let a = getbit(line, x);
        (a, x + 1)
    } else {
        return x;
    };

    while x < w {
        if getbit(line, x) != a {
            return x;
        }
        x += 1;
    }
    w
}

fn find_changing_element_of_color(line: Option<&[u8]>, x: u32, w: u32, color: u8) -> u32 {
    if line.is_none() {
        return w;
    }
    let mut pos = find_changing_element(line, x, w);
    if pos < w && getbit(line.unwrap(), pos) != color {
        pos = find_changing_element(line, pos, w);
    }
    pos
}

const LM: [u8; 8] = [0xFF, 0x7F, 0x3F, 0x1F, 0x0F, 0x07, 0x03, 0x01];
const RM: [u8; 8] = [0x00, 0x80, 0xC0, 0xE0, 0xF0, 0xF8, 0xFC, 0xFE];

fn set_bits(line: &mut [u8], x0: u32, x1: u32) {
    if x0 >= x1 {
        return;
    }
    let a0 = (x0 >> 3) as usize;
    let a1 = (x1 >> 3) as usize;
    let b0 = (x0 & 7) as usize;
    let b1 = (x1 & 7) as usize;

    if a0 == a1 {
        if a0 < line.len() {
            line[a0] |= LM[b0] & RM[b1];
        }
    } else {
        if a0 < line.len() {
            line[a0] |= LM[b0];
        }
        for a in (a0 + 1)..a1.min(line.len()) {
            line[a] = 0xFF;
        }
        if b1 != 0 && a1 < line.len() {
            line[a1] |= RM[b1];
        }
    }
}

// --- 2D line decoder ---

fn decode_mmr_line(
    mmr: &mut MmrCtx,
    ref_line: Option<&[u8]>,
    dst: &mut [u8],
) -> Result<bool> {
    let width = mmr.width;
    let mut a0: u32 = MINUS1;
    let mut c: u8 = 0; // 0=white, 1=black
    let mut eofb = false;

    loop {
        if a0 != MINUS1 && a0 >= width {
            break;
        }

        let word = mmr.word;

        // Check for various 2D mode codes
        if (word >> 29) == 0b001 {
            // Horizontal mode
            mmr.consume(3);
            if a0 == MINUS1 {
                a0 = 0;
            }

            if c == 0 {
                let white_run = get_run(mmr, &WHITE_TABLE, 8)?;
                let black_run = get_run(mmr, &BLACK_TABLE, 7)?;
                let a1 = (a0 + white_run).min(width);
                let a2 = (a1 + black_run).min(width);
                set_bits(dst, a1, a2);
                a0 = a2;
            } else {
                let black_run = get_run(mmr, &BLACK_TABLE, 7)?;
                let white_run = get_run(mmr, &WHITE_TABLE, 8)?;
                let a1 = (a0 + black_run).min(width);
                let a2 = (a1 + white_run).min(width);
                set_bits(dst, a0, a1);
                a0 = a2;
            }
        } else if (word >> 28) == 0b0001 {
            // Pass mode
            mmr.consume(4);
            if a0 == MINUS1 {
                a0 = 0;
            }
            let b1 = find_changing_element_of_color(ref_line, a0, width, 1 - c);
            let b2 = find_changing_element(ref_line, b1, width);
            if c == 1 {
                set_bits(dst, a0, b2);
            }
            a0 = b2;
        } else if (word >> 31) == 1 {
            // V(0)
            mmr.consume(1);
            if a0 == MINUS1 {
                a0 = 0;
            }
            let b1 = find_changing_element_of_color(ref_line, a0, width, 1 - c);
            if c == 1 {
                set_bits(dst, a0, b1);
            }
            a0 = b1;
            c = 1 - c;
        } else if (word >> 29) == 0b011 {
            // VR(1)
            mmr.consume(3);
            if a0 == MINUS1 {
                a0 = 0;
            }
            let b1 = find_changing_element_of_color(ref_line, a0, width, 1 - c);
            let b1 = (b1 + 1).min(width);
            if c == 1 {
                set_bits(dst, a0, b1);
            }
            a0 = b1;
            c = 1 - c;
        } else if (word >> 26) == 0b000011 {
            // VR(2)
            mmr.consume(6);
            if a0 == MINUS1 {
                a0 = 0;
            }
            let b1 = find_changing_element_of_color(ref_line, a0, width, 1 - c);
            let b1 = (b1 + 2).min(width);
            if c == 1 {
                set_bits(dst, a0, b1);
            }
            a0 = b1;
            c = 1 - c;
        } else if (word >> 25) == 0b0000011 {
            // VR(3)
            mmr.consume(7);
            if a0 == MINUS1 {
                a0 = 0;
            }
            let b1 = find_changing_element_of_color(ref_line, a0, width, 1 - c);
            let b1 = (b1 + 3).min(width);
            if c == 1 {
                set_bits(dst, a0, b1);
            }
            a0 = b1;
            c = 1 - c;
        } else if (word >> 29) == 0b010 {
            // VL(1)
            mmr.consume(3);
            if a0 == MINUS1 {
                a0 = 0;
            }
            let b1 = find_changing_element_of_color(ref_line, a0, width, 1 - c);
            let b1 = b1.saturating_sub(1);
            if c == 1 && b1 > a0 {
                set_bits(dst, a0, b1);
            }
            a0 = b1;
            c = 1 - c;
        } else if (word >> 26) == 0b000010 {
            // VL(2)
            mmr.consume(6);
            if a0 == MINUS1 {
                a0 = 0;
            }
            let b1 = find_changing_element_of_color(ref_line, a0, width, 1 - c);
            let b1 = b1.saturating_sub(2);
            if c == 1 && b1 > a0 {
                set_bits(dst, a0, b1);
            }
            a0 = b1;
            c = 1 - c;
        } else if (word >> 25) == 0b0000010 {
            // VL(3)
            mmr.consume(7);
            if a0 == MINUS1 {
                a0 = 0;
            }
            let b1 = find_changing_element_of_color(ref_line, a0, width, 1 - c);
            let b1 = b1.saturating_sub(3);
            if c == 1 && b1 > a0 {
                set_bits(dst, a0, b1);
            }
            a0 = b1;
            c = 1 - c;
        } else if (word >> 8) == 0x001001 {
            // EOFB (24 bits)
            mmr.consume(24);
            eofb = true;
            break;
        } else {
            // Unknown code — stop
            break;
        }
    }

    Ok(eofb)
}

// --- Public API ---

/// Decode a generic region using MMR compression.
pub fn decode_generic_mmr(
    data: &[u8],
    image: &mut Jbig2Image,
) -> Result<usize> {
    let mut mmr = MmrCtx::new(image.width, data);
    let stride = image.stride as usize;

    let mut ref_start: Option<usize> = None;

    for y in 0..image.height {
        let dst_start = (y * image.stride) as usize;
        let dst_end = dst_start + stride;
        // Clear line to white
        image.data[dst_start..dst_end].fill(0);

        let ref_line = ref_start.map(|s| &image.data[s..s + stride]);

        // We need to decode into a separate buffer because ref_line borrows image.data
        let mut line_buf = vec![0u8; stride];
        if let Some(rl) = ref_line {
            let eofb = decode_mmr_line(&mut mmr, Some(rl), &mut line_buf)?;
            image.data[dst_start..dst_end].copy_from_slice(&line_buf);
            if eofb {
                // Fill remaining with white
                for yy in (y + 1)..image.height {
                    let s = (yy * image.stride) as usize;
                    image.data[s..s + stride].fill(0);
                }
                break;
            }
        } else {
            let eofb = decode_mmr_line(&mut mmr, None, &mut line_buf)?;
            image.data[dst_start..dst_end].copy_from_slice(&line_buf);
            if eofb {
                break;
            }
        }

        ref_start = Some(dst_start);
    }

    Ok(mmr.consumed_bytes())
}

/// Decode halftone region using MMR (returns consumed bytes).
pub fn decode_halftone_mmr(
    data: &[u8],
    image: &mut Jbig2Image,
) -> Result<usize> {
    decode_generic_mmr(data, image)
}

// =============================================================================
// White and Black Run-Length Huffman Tables
// =============================================================================
// These are the standard CCITT T.6 (Group 4) tables.

macro_rules! mn {
    ($v:expr, $n:expr) => {
        MmrNode { val: $v, n_bits: $n }
    };
}

#[rustfmt::skip]
static WHITE_TABLE: [MmrNode; 472] = [
    mn!(256,12), mn!(272,12), mn!(29,8), mn!(30,8), mn!(45,8), mn!(46,8), mn!(22,7), mn!(22,7),
    mn!(23,7), mn!(23,7), mn!(47,8), mn!(48,8), mn!(13,6), mn!(13,6), mn!(13,6), mn!(13,6),
    mn!(20,7), mn!(20,7), mn!(33,8), mn!(34,8), mn!(35,8), mn!(36,8), mn!(37,8), mn!(38,8),
    mn!(19,7), mn!(19,7), mn!(31,8), mn!(32,8), mn!(1,6), mn!(1,6), mn!(1,6), mn!(1,6),
    mn!(12,6), mn!(12,6), mn!(12,6), mn!(12,6), mn!(53,8), mn!(54,8), mn!(26,7), mn!(26,7),
    mn!(39,8), mn!(40,8), mn!(41,8), mn!(42,8), mn!(43,8), mn!(44,8), mn!(21,7), mn!(21,7),
    mn!(28,7), mn!(28,7), mn!(61,8), mn!(62,8), mn!(63,8), mn!(0,8), mn!(320,8), mn!(384,8),
    mn!(10,5), mn!(10,5), mn!(10,5), mn!(10,5), mn!(10,5), mn!(10,5), mn!(10,5), mn!(10,5),
    mn!(11,5), mn!(11,5), mn!(11,5), mn!(11,5), mn!(11,5), mn!(11,5), mn!(11,5), mn!(11,5),
    mn!(27,7), mn!(27,7), mn!(59,8), mn!(60,8), mn!(288,9), mn!(290,9), mn!(18,7), mn!(18,7),
    mn!(24,7), mn!(24,7), mn!(49,8), mn!(50,8), mn!(51,8), mn!(52,8), mn!(25,7), mn!(25,7),
    mn!(55,8), mn!(56,8), mn!(57,8), mn!(58,8), mn!(192,6), mn!(192,6), mn!(192,6), mn!(192,6),
    mn!(1664,6), mn!(1664,6), mn!(1664,6), mn!(1664,6), mn!(448,8), mn!(512,8), mn!(292,9), mn!(640,8),
    mn!(576,8), mn!(294,9), mn!(296,9), mn!(298,9), mn!(300,9), mn!(302,9), mn!(256,7), mn!(256,7),
    mn!(2,4), mn!(2,4), mn!(2,4), mn!(2,4), mn!(2,4), mn!(2,4), mn!(2,4), mn!(2,4),
    mn!(2,4), mn!(2,4), mn!(2,4), mn!(2,4), mn!(2,4), mn!(2,4), mn!(2,4), mn!(2,4),
    mn!(3,4), mn!(3,4), mn!(3,4), mn!(3,4), mn!(3,4), mn!(3,4), mn!(3,4), mn!(3,4),
    mn!(3,4), mn!(3,4), mn!(3,4), mn!(3,4), mn!(3,4), mn!(3,4), mn!(3,4), mn!(3,4),
    mn!(128,5), mn!(128,5), mn!(128,5), mn!(128,5), mn!(128,5), mn!(128,5), mn!(128,5), mn!(128,5),
    mn!(8,5), mn!(8,5), mn!(8,5), mn!(8,5), mn!(8,5), mn!(8,5), mn!(8,5), mn!(8,5),
    mn!(9,5), mn!(9,5), mn!(9,5), mn!(9,5), mn!(9,5), mn!(9,5), mn!(9,5), mn!(9,5),
    mn!(16,6), mn!(16,6), mn!(16,6), mn!(16,6), mn!(17,6), mn!(17,6), mn!(17,6), mn!(17,6),
    mn!(4,4), mn!(4,4), mn!(4,4), mn!(4,4), mn!(4,4), mn!(4,4), mn!(4,4), mn!(4,4),
    mn!(4,4), mn!(4,4), mn!(4,4), mn!(4,4), mn!(4,4), mn!(4,4), mn!(4,4), mn!(4,4),
    mn!(5,4), mn!(5,4), mn!(5,4), mn!(5,4), mn!(5,4), mn!(5,4), mn!(5,4), mn!(5,4),
    mn!(5,4), mn!(5,4), mn!(5,4), mn!(5,4), mn!(5,4), mn!(5,4), mn!(5,4), mn!(5,4),
    mn!(14,6), mn!(14,6), mn!(14,6), mn!(14,6), mn!(15,6), mn!(15,6), mn!(15,6), mn!(15,6),
    mn!(64,5), mn!(64,5), mn!(64,5), mn!(64,5), mn!(64,5), mn!(64,5), mn!(64,5), mn!(64,5),
    mn!(6,4), mn!(6,4), mn!(6,4), mn!(6,4), mn!(6,4), mn!(6,4), mn!(6,4), mn!(6,4),
    mn!(6,4), mn!(6,4), mn!(6,4), mn!(6,4), mn!(6,4), mn!(6,4), mn!(6,4), mn!(6,4),
    mn!(7,4), mn!(7,4), mn!(7,4), mn!(7,4), mn!(7,4), mn!(7,4), mn!(7,4), mn!(7,4),
    mn!(7,4), mn!(7,4), mn!(7,4), mn!(7,4), mn!(7,4), mn!(7,4), mn!(7,4), mn!(7,4),
    // Extended entries (subtable for codes > 8 bits)
    mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0),
    mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0),
    mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0),
    mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0),
    mn!(-2,3), mn!(-2,3), mn!(-2,3), mn!(-2,3), mn!(-2,3), mn!(-2,3), mn!(-2,3), mn!(-2,3),
    mn!(-2,3), mn!(-2,3), mn!(-2,3), mn!(-2,3), mn!(-2,3), mn!(-2,3), mn!(-2,3), mn!(-2,3),
    mn!(-2,3), mn!(-2,3), mn!(-2,3), mn!(-2,3), mn!(-2,3), mn!(-2,3), mn!(-2,3), mn!(-2,3),
    mn!(-2,3), mn!(-2,3), mn!(-2,3), mn!(-2,3), mn!(-2,3), mn!(-2,3), mn!(-2,3), mn!(-2,3),
    mn!(-3,4), mn!(-3,4), mn!(-3,4), mn!(-3,4), mn!(-3,4), mn!(-3,4), mn!(-3,4), mn!(-3,4),
    mn!(-3,4), mn!(-3,4), mn!(-3,4), mn!(-3,4), mn!(-3,4), mn!(-3,4), mn!(-3,4), mn!(-3,4),
    mn!(304,9), mn!(304,9), mn!(304,9), mn!(304,9), mn!(304,9), mn!(304,9), mn!(304,9), mn!(304,9),
    mn!(306,9), mn!(306,9), mn!(306,9), mn!(306,9), mn!(306,9), mn!(306,9), mn!(306,9), mn!(306,9),
    mn!(308,9), mn!(308,9), mn!(308,9), mn!(308,9), mn!(308,9), mn!(308,9), mn!(308,9), mn!(308,9),
    mn!(310,9), mn!(310,9), mn!(310,9), mn!(310,9), mn!(310,9), mn!(310,9), mn!(310,9), mn!(310,9),
    // makeup codes (10-13 bit codes stored in subtable)
    mn!(352,12), mn!(390,12), mn!(416,12), mn!(418,12), mn!(414,12), mn!(412,12), mn!(410,12), mn!(408,12),
    mn!(406,12), mn!(404,12), mn!(402,12), mn!(400,12), mn!(398,12), mn!(396,12), mn!(394,12), mn!(392,12),
    mn!(1792,12), mn!(1856,12), mn!(1920,12), mn!(-1,0), mn!(1600,12), mn!(1536,12), mn!(1472,12), mn!(1408,12),
    mn!(1344,12), mn!(1280,12), mn!(1216,12), mn!(1152,12), mn!(1088,12), mn!(1024,12), mn!(960,12), mn!(896,12),
    mn!(1728,11), mn!(1728,11), mn!(704,11), mn!(704,11), mn!(768,11), mn!(768,11), mn!(832,11), mn!(832,11),
    // additional entries
    mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0),
    mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0),
    mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0),
    mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0),
    mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0),
    mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0),
    mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0),
    mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0),
];

#[rustfmt::skip]
static BLACK_TABLE: [MmrNode; 256] = [
    mn!(128,12), mn!(160,13), mn!(224,12), mn!(256,12), mn!(10,7), mn!(11,7), mn!(288,12), mn!(12,7),
    mn!(9,6), mn!(9,6), mn!(8,6), mn!(8,6), mn!(7,5), mn!(7,5), mn!(7,5), mn!(7,5),
    mn!(6,4), mn!(6,4), mn!(6,4), mn!(6,4), mn!(6,4), mn!(6,4), mn!(6,4), mn!(6,4),
    mn!(5,4), mn!(5,4), mn!(5,4), mn!(5,4), mn!(5,4), mn!(5,4), mn!(5,4), mn!(5,4),
    mn!(1,3), mn!(1,3), mn!(1,3), mn!(1,3), mn!(1,3), mn!(1,3), mn!(1,3), mn!(1,3),
    mn!(1,3), mn!(1,3), mn!(1,3), mn!(1,3), mn!(1,3), mn!(1,3), mn!(1,3), mn!(1,3),
    mn!(4,3), mn!(4,3), mn!(4,3), mn!(4,3), mn!(4,3), mn!(4,3), mn!(4,3), mn!(4,3),
    mn!(4,3), mn!(4,3), mn!(4,3), mn!(4,3), mn!(4,3), mn!(4,3), mn!(4,3), mn!(4,3),
    mn!(3,2), mn!(3,2), mn!(3,2), mn!(3,2), mn!(3,2), mn!(3,2), mn!(3,2), mn!(3,2),
    mn!(3,2), mn!(3,2), mn!(3,2), mn!(3,2), mn!(3,2), mn!(3,2), mn!(3,2), mn!(3,2),
    mn!(3,2), mn!(3,2), mn!(3,2), mn!(3,2), mn!(3,2), mn!(3,2), mn!(3,2), mn!(3,2),
    mn!(3,2), mn!(3,2), mn!(3,2), mn!(3,2), mn!(3,2), mn!(3,2), mn!(3,2), mn!(3,2),
    mn!(2,2), mn!(2,2), mn!(2,2), mn!(2,2), mn!(2,2), mn!(2,2), mn!(2,2), mn!(2,2),
    mn!(2,2), mn!(2,2), mn!(2,2), mn!(2,2), mn!(2,2), mn!(2,2), mn!(2,2), mn!(2,2),
    mn!(2,2), mn!(2,2), mn!(2,2), mn!(2,2), mn!(2,2), mn!(2,2), mn!(2,2), mn!(2,2),
    mn!(2,2), mn!(2,2), mn!(2,2), mn!(2,2), mn!(2,2), mn!(2,2), mn!(2,2), mn!(2,2),
    // Extended/subtable entries for longer codes
    mn!(13,11), mn!(13,11), mn!(23,12), mn!(54,12), mn!(55,12), mn!(56,12), mn!(57,12), mn!(58,12),
    mn!(59,12), mn!(60,12), mn!(320,12), mn!(384,12), mn!(448,12), mn!(17,12), mn!(18,12), mn!(19,12),
    mn!(20,12), mn!(21,12), mn!(22,12), mn!(23,12), mn!(24,12), mn!(25,12), mn!(26,12), mn!(27,12),
    mn!(28,12), mn!(29,12), mn!(30,12), mn!(31,12), mn!(32,12), mn!(33,12), mn!(34,12), mn!(35,12),
    mn!(36,12), mn!(37,12), mn!(38,12), mn!(39,12), mn!(40,12), mn!(41,12), mn!(42,12), mn!(43,12),
    mn!(44,12), mn!(45,12), mn!(46,12), mn!(47,12), mn!(48,12), mn!(49,12), mn!(50,12), mn!(51,12),
    mn!(52,12), mn!(53,12), mn!(-1,0), mn!(-1,0), mn!(512,12), mn!(576,12), mn!(640,12), mn!(704,12),
    mn!(768,12), mn!(832,12), mn!(896,12), mn!(960,12), mn!(1024,12), mn!(1088,12), mn!(1152,12), mn!(1216,12),
    mn!(1280,12), mn!(1344,12), mn!(1408,12), mn!(1472,12), mn!(1536,12), mn!(1600,12), mn!(1664,12), mn!(1728,12),
    mn!(1792,12), mn!(1856,12), mn!(1920,12), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0),
    mn!(14,10), mn!(14,10), mn!(14,10), mn!(14,10), mn!(15,11), mn!(15,11), mn!(16,12), mn!(0,12),
    mn!(192,12), mn!(64,10), mn!(64,10), mn!(64,10), mn!(64,10), mn!(-1,0), mn!(-1,0), mn!(-1,0),
    mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0),
    mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0),
    mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0),
    mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0), mn!(-1,0),
];
