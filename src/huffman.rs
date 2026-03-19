//! Huffman decoder (ITU T.88 Annex B).
//!
//! Ported from jbig2dec/jbig2_huffman.c and jbig2_hufftab.c.

use crate::error::{Jbig2Error, Result};

/// A single line in a Huffman table definition.
#[derive(Debug, Clone, Copy)]
pub struct HuffmanLine {
    pub preflen: u8,
    pub rangelen: u8,
    pub rangelow: i32,
}

/// Huffman table parameters (template).
#[derive(Debug, Clone)]
pub struct HuffmanParams {
    pub htoob: bool,
    pub lines: Vec<HuffmanLine>,
}

// --- Compiled table for fast lookup ---

const FLAGS_ISOOB: u8 = 1;
const FLAGS_ISLOW: u8 = 2;

const LOG_TABLE_SIZE_MAX: u8 = 16;

#[derive(Debug, Clone, Copy)]
struct HuffmanEntry {
    rangelow: i32,
    preflen: u8,
    rangelen: u8,
    flags: u8,
}

/// Compiled Huffman lookup table.
#[derive(Debug, Clone)]
pub struct HuffmanTable {
    pub(crate) log_table_size: u8,
    entries: Vec<HuffmanEntry>,
}

/// Build a fast lookup table from HuffmanParams.
pub fn build_table(params: &HuffmanParams) -> Result<HuffmanTable> {
    let lines = &params.lines;
    let n_lines = lines.len();

    if n_lines == 0 {
        return Err(Jbig2Error::InvalidData("empty huffman table".into()));
    }

    // B.3, 1. Count prefix lengths
    let mut lencount = [0i32; 256];
    let mut lenmax: i32 = 0;
    let mut log_table_size: u8 = 0;

    for line in lines.iter() {
        let pl = line.preflen as i32;
        if pl > lenmax {
            lenmax = pl;
        }
        lencount[pl as usize] += 1;

        let lts = if (line.preflen as u16 + line.rangelen as u16) > LOG_TABLE_SIZE_MAX as u16 {
            line.preflen
        } else {
            line.preflen + line.rangelen
        };
        if lts <= LOG_TABLE_SIZE_MAX && lts > log_table_size {
            log_table_size = lts;
        }
    }

    let max_j = 1u32 << log_table_size;
    let mut entries = vec![
        HuffmanEntry {
            rangelow: 0,
            preflen: 0xFF,
            rangelen: 0xFF,
            flags: 0xFF,
        };
        max_j as usize
    ];

    lencount[0] = 0;
    let mut firstcode: i32 = 0;

    for curlen in 1..=lenmax {
        let shift = log_table_size as i32 - curlen;
        firstcode = (firstcode + lencount[curlen as usize - 1]) << 1;
        let mut curcode = firstcode;

        for (curtemp, line) in lines.iter().enumerate() {
            if line.preflen as i32 == curlen {
                let rangelen = line.rangelen;
                let start_j = (curcode << shift) as u32;
                let end_j = ((curcode + 1) << shift) as u32;

                if end_j > max_j {
                    return Err(Jbig2Error::InvalidData(
                        "huffman table overflow".into(),
                    ));
                }

                let mut eflags: u8 = 0;
                if params.htoob && curtemp == n_lines - 1 {
                    eflags |= FLAGS_ISOOB;
                }
                let low_idx = n_lines - if params.htoob { 3 } else { 2 };
                if curtemp == low_idx {
                    eflags |= FLAGS_ISLOW;
                }

                if (line.preflen as u16 + rangelen as u16) > LOG_TABLE_SIZE_MAX as u16 {
                    for cur_j in start_j..end_j {
                        entries[cur_j as usize] = HuffmanEntry {
                            rangelow: line.rangelow,
                            preflen: line.preflen,
                            rangelen,
                            flags: eflags,
                        };
                    }
                } else {
                    for cur_j in start_j..end_j {
                        let htoffset = ((cur_j >> (shift - rangelen as i32)) & ((1 << rangelen) - 1)) as i32;
                        let rl = if eflags & FLAGS_ISLOW != 0 {
                            line.rangelow - htoffset
                        } else {
                            line.rangelow + htoffset
                        };
                        entries[cur_j as usize] = HuffmanEntry {
                            rangelow: rl,
                            preflen: line.preflen + rangelen,
                            rangelen: 0,
                            flags: eflags,
                        };
                    }
                }
                curcode += 1;
            }
        }
    }

    Ok(HuffmanTable {
        log_table_size,
        entries,
    })
}

// --- Huffman decoder state ---

/// Huffman bitstream reader.
pub struct HuffmanState<'a> {
    data: &'a [u8],
    this_word: u32,
    next_word: u32,
    offset_bits: u32,
    offset: usize,
}

impl<'a> HuffmanState<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        let this_word = Self::read_word(data, 0);
        let next_word = Self::read_word(data, 4);
        HuffmanState {
            data,
            this_word,
            next_word,
            offset_bits: 0,
            offset: 0,
        }
    }

    fn read_word(data: &[u8], offset: usize) -> u32 {
        let mut val = 0u32;
        for i in 0..4 {
            if offset + i < data.len() {
                val |= (data[offset + i] as u32) << (24 - i * 8);
            }
        }
        val
    }

    fn refill(&mut self) {
        if self.offset_bits >= 32 {
            self.this_word = self.next_word;
            self.offset += 4;
            self.offset_bits -= 32;
            self.next_word = Self::read_word(self.data, self.offset + 4);
            if self.offset_bits > 0 {
                self.this_word =
                    (self.this_word << self.offset_bits) | (self.next_word >> (32 - self.offset_bits));
            }
        }
    }

    /// Read `n` raw bits (no table decode).
    pub fn get_bits(&mut self, n: u8) -> u32 {
        let result = self.this_word >> (32 - n);
        self.offset_bits += n as u32;

        if self.offset_bits >= 32 {
            self.offset += 4;
            self.offset_bits -= 32;
            self.this_word = self.next_word;
            self.next_word = Self::read_word(self.data, self.offset + 4);
            if self.offset_bits > 0 {
                self.this_word =
                    (self.this_word << self.offset_bits) | (self.next_word >> (32 - self.offset_bits));
            }
        } else {
            self.this_word = (self.this_word << n) | (self.next_word >> (32 - self.offset_bits));
        }

        result
    }

    /// Decode a value from a compiled Huffman table.
    /// Returns `(value, is_oob)`.
    pub fn get(&mut self, table: &HuffmanTable) -> Result<(i32, bool)> {
        let lts = table.log_table_size;
        let idx = if lts > 0 {
            (self.this_word >> (32 - lts)) as usize
        } else {
            0
        };

        if idx >= table.entries.len() {
            return Err(Jbig2Error::InvalidData("huffman index out of range".into()));
        }

        let entry = &table.entries[idx];
        if entry.flags == 0xFF {
            return Err(Jbig2Error::InvalidData(
                "unpopulated huffman table entry".into(),
            ));
        }

        let preflen = entry.preflen;
        let rangelen = entry.rangelen;
        let flags = entry.flags;

        self.offset_bits += preflen as u32;
        if self.offset_bits >= 32 {
            self.this_word = self.next_word;
            self.offset += 4;
            self.next_word = Self::read_word(self.data, self.offset + 4);
            self.offset_bits -= 32;
            if self.offset_bits > 0 {
                self.this_word = (self.this_word << self.offset_bits)
                    | (self.next_word >> (32 - self.offset_bits));
            }
        } else if preflen > 0 {
            self.this_word = (self.this_word << preflen)
                | (self.next_word >> (32 - self.offset_bits));
        }

        let mut result = entry.rangelow;
        if rangelen > 0 {
            let htoffset = (self.this_word >> (32 - rangelen)) as i32;
            if flags & FLAGS_ISLOW != 0 {
                result -= htoffset;
            } else {
                result += htoffset;
            }
            self.offset_bits += rangelen as u32;
            if self.offset_bits >= 32 {
                self.this_word = self.next_word;
                self.offset += 4;
                self.next_word = Self::read_word(self.data, self.offset + 4);
                self.offset_bits -= 32;
                if self.offset_bits > 0 {
                    self.this_word = (self.this_word << self.offset_bits)
                        | (self.next_word >> (32 - self.offset_bits));
                }
            } else {
                self.this_word = (self.this_word << rangelen)
                    | (self.next_word >> (32 - self.offset_bits));
            }
        }

        let is_oob = flags & FLAGS_ISOOB != 0;
        Ok((result, is_oob))
    }

    /// Skip to next byte boundary.
    pub fn align(&mut self) {
        let bits = self.offset_bits & 7;
        if bits != 0 {
            let skip = 8 - bits;
            self.offset_bits += skip;
            self.this_word =
                (self.this_word << skip) | (self.next_word >> (32 - self.offset_bits));
            self.refill();
        }
    }

    /// Current byte offset in the stream.
    pub fn offset(&self) -> usize {
        self.offset + (self.offset_bits as usize >> 3)
    }
}

// =============================================================================
// Standard Huffman Tables (Annex B, Tables B.1 through B.15)
// =============================================================================

macro_rules! hl {
    ($p:expr, $r:expr, $l:expr) => {
        HuffmanLine { preflen: $p, rangelen: $r, rangelow: $l }
    };
}

/// Table A (B.1)
pub const TABLE_A: &[HuffmanLine] = &[
    hl!(1, 4, 0), hl!(2, 8, 16), hl!(3, 16, 272),
    hl!(0, 32, -1),    // low
    hl!(3, 32, 65808), // high
];

/// Table B (B.2)
pub const TABLE_B: &[HuffmanLine] = &[
    hl!(1, 0, 0), hl!(2, 0, 1), hl!(3, 0, 2), hl!(4, 3, 3), hl!(5, 6, 11),
    hl!(0, 32, -1),  // low
    hl!(6, 32, 75),  // high
    hl!(6, 0, 0),    // OOB
];

/// Table C (B.3)
pub const TABLE_C: &[HuffmanLine] = &[
    hl!(8, 8, -256), hl!(1, 0, 0), hl!(2, 0, 1), hl!(3, 0, 2), hl!(4, 3, 3), hl!(5, 6, 11),
    hl!(8, 32, -257),  // low
    hl!(7, 32, 75),    // high
    hl!(6, 0, 0),      // OOB
];

/// Table D (B.4)
pub const TABLE_D: &[HuffmanLine] = &[
    hl!(1, 0, 1), hl!(2, 0, 2), hl!(3, 0, 3), hl!(4, 3, 4), hl!(5, 6, 12),
    hl!(0, 32, -1),  // low
    hl!(5, 32, 76),  // high
];

/// Table E (B.5)
pub const TABLE_E: &[HuffmanLine] = &[
    hl!(7, 8, -255), hl!(1, 0, 1), hl!(2, 0, 2), hl!(3, 0, 3), hl!(4, 3, 4), hl!(5, 6, 12),
    hl!(7, 32, -256), // low
    hl!(6, 32, 76),   // high
];

/// Table F (B.6)
pub const TABLE_F: &[HuffmanLine] = &[
    hl!(5, 10, -2048), hl!(4, 9, -1024), hl!(4, 8, -512), hl!(4, 7, -256),
    hl!(5, 6, -128), hl!(5, 5, -64), hl!(4, 5, -32), hl!(2, 7, 0),
    hl!(3, 7, 128), hl!(3, 8, 256), hl!(4, 9, 512), hl!(4, 10, 1024),
    hl!(6, 32, -2049), // low
    hl!(6, 32, 2048),  // high
];

/// Table G (B.7)
pub const TABLE_G: &[HuffmanLine] = &[
    hl!(4, 9, -1024), hl!(3, 8, -512), hl!(4, 7, -256), hl!(5, 6, -128),
    hl!(5, 5, -64), hl!(4, 5, -32), hl!(4, 5, 0), hl!(5, 5, 32),
    hl!(5, 6, 64), hl!(4, 7, 128), hl!(3, 8, 256), hl!(3, 9, 512),
    hl!(3, 10, 1024),
    hl!(5, 32, -1025), // low
    hl!(5, 32, 2048),  // high
];

/// Table H (B.8)
pub const TABLE_H: &[HuffmanLine] = &[
    hl!(8, 3, -15), hl!(9, 1, -7), hl!(8, 1, -5), hl!(9, 0, -3),
    hl!(7, 0, -2), hl!(4, 0, -1), hl!(2, 1, 0), hl!(5, 0, 2),
    hl!(6, 0, 3), hl!(3, 4, 4), hl!(6, 1, 20), hl!(4, 4, 22),
    hl!(4, 5, 38), hl!(5, 6, 70), hl!(5, 7, 134), hl!(6, 7, 262),
    hl!(7, 8, 390), hl!(6, 10, 646),
    hl!(9, 32, -16),   // low
    hl!(9, 32, 1670),  // high
    hl!(2, 0, 0),      // OOB
];

/// Table I (B.9)
pub const TABLE_I: &[HuffmanLine] = &[
    hl!(8, 4, -31), hl!(9, 2, -15), hl!(8, 2, -11), hl!(9, 1, -7),
    hl!(7, 1, -5), hl!(4, 1, -3), hl!(3, 1, -1), hl!(3, 1, 1),
    hl!(5, 1, 3), hl!(6, 1, 5), hl!(3, 5, 7), hl!(6, 2, 39),
    hl!(4, 5, 43), hl!(4, 6, 75), hl!(5, 7, 139), hl!(5, 8, 267),
    hl!(6, 8, 523), hl!(7, 9, 779), hl!(6, 11, 1291),
    hl!(9, 32, -32),   // low
    hl!(9, 32, 3339),  // high
    hl!(2, 0, 0),      // OOB
];

/// Table J (B.10)
pub const TABLE_J: &[HuffmanLine] = &[
    hl!(7, 4, -21), hl!(8, 0, -5), hl!(7, 0, -4), hl!(5, 0, -3),
    hl!(2, 2, -2), hl!(5, 0, 2), hl!(6, 0, 3), hl!(7, 0, 4),
    hl!(8, 0, 5), hl!(2, 6, 6), hl!(5, 5, 70), hl!(6, 5, 102),
    hl!(6, 6, 134), hl!(6, 7, 198), hl!(6, 8, 326), hl!(6, 9, 582),
    hl!(6, 10, 1094), hl!(7, 11, 2118),
    hl!(8, 32, -22),   // low
    hl!(8, 32, 4166),  // high
    hl!(2, 0, 0),      // OOB
];

/// Table K (B.11)
pub const TABLE_K: &[HuffmanLine] = &[
    hl!(1, 0, 1), hl!(2, 1, 2), hl!(4, 0, 4), hl!(4, 1, 5),
    hl!(5, 1, 7), hl!(5, 2, 9), hl!(6, 2, 13), hl!(7, 2, 17),
    hl!(7, 3, 21), hl!(7, 4, 29), hl!(7, 5, 45), hl!(7, 6, 77),
    hl!(0, 32, -1),   // low
    hl!(7, 32, 141),  // high
];

/// Table L (B.12)
pub const TABLE_L: &[HuffmanLine] = &[
    hl!(1, 0, 1), hl!(2, 0, 2), hl!(3, 1, 3), hl!(5, 0, 5),
    hl!(5, 1, 6), hl!(6, 1, 8), hl!(7, 0, 10), hl!(7, 1, 11),
    hl!(7, 2, 13), hl!(7, 3, 17), hl!(7, 4, 25), hl!(8, 5, 41),
    hl!(8, 32, 73),
    hl!(0, 32, -1), // low
    hl!(0, 32, 0),  // high — NOTE: jbig2dec has PREFLEN=0 for both low and high
];

/// Table M (B.13)
pub const TABLE_M: &[HuffmanLine] = &[
    hl!(1, 0, 1), hl!(3, 0, 2), hl!(4, 0, 3), hl!(5, 0, 4),
    hl!(4, 1, 5), hl!(3, 3, 7), hl!(6, 1, 15), hl!(6, 2, 17),
    hl!(6, 3, 21), hl!(6, 4, 29), hl!(6, 5, 45), hl!(7, 6, 77),
    hl!(0, 32, -1),   // low
    hl!(7, 32, 141),  // high
];

/// Table N (B.14)
pub const TABLE_N: &[HuffmanLine] = &[
    hl!(3, 0, -2), hl!(3, 0, -1), hl!(1, 0, 0), hl!(3, 0, 1), hl!(3, 0, 2),
    hl!(0, 32, -1), // low
    hl!(0, 32, 3),  // high
];

/// Table O (B.15)
pub const TABLE_O: &[HuffmanLine] = &[
    hl!(7, 4, -24), hl!(6, 2, -8), hl!(5, 1, -4), hl!(4, 0, -2),
    hl!(3, 0, -1), hl!(1, 0, 0), hl!(3, 0, 1), hl!(4, 0, 2),
    hl!(5, 1, 3), hl!(6, 2, 5), hl!(7, 4, 9),
    hl!(7, 32, -25), // low
    hl!(7, 32, 25),  // high
];

/// All 15 standard tables indexed 0..14 (A=0, B=1, ... O=14).
pub const STANDARD_TABLES: [&[HuffmanLine]; 15] = [
    TABLE_A, TABLE_B, TABLE_C, TABLE_D, TABLE_E,
    TABLE_F, TABLE_G, TABLE_H, TABLE_I, TABLE_J,
    TABLE_K, TABLE_L, TABLE_M, TABLE_N, TABLE_O,
];

/// Whether a standard table has HTOOB.
pub const STANDARD_TABLE_HTOOB: [bool; 15] = [
    false, true, true, false, false,
    false, false, true, true, true,
    false, false, false, false, false,
];

/// Build a compiled table from a standard table index (0=A, 1=B, ... 14=O).
pub fn build_standard_table(index: usize) -> Result<HuffmanTable> {
    if index >= 15 {
        return Err(Jbig2Error::InvalidData("invalid standard table index".into()));
    }
    let params = HuffmanParams {
        htoob: STANDARD_TABLE_HTOOB[index],
        lines: STANDARD_TABLES[index].to_vec(),
    };
    build_table(&params)
}

// --- User-defined table (segment type 53) parsing ---

/// Parse a user-defined Huffman table from segment data.
pub fn parse_user_table(data: &[u8]) -> Result<HuffmanParams> {
    if data.len() < 10 {
        return Err(Jbig2Error::InvalidData("user table segment too short".into()));
    }

    let flags = data[0];
    let htoob = (flags & 0x01) != 0;
    let htps = ((flags >> 1) & 0x07) as u8 + 1;
    let htrs = ((flags >> 4) & 0x07) as u8 + 1;

    let htlow = i32::from_be_bytes([data[1], data[2], data[3], data[4]]);
    let hthigh = i32::from_be_bytes([data[5], data[6], data[7], data[8]]);

    if htlow >= hthigh {
        return Err(Jbig2Error::InvalidData("invalid user table range".into()));
    }

    let lines_data = &data[9..];
    let mut bit_offset: usize = 0;
    let mut lines = Vec::new();
    let mut currangelow = htlow;

    // B.2 5)
    while currangelow < hthigh {
        let preflen = read_bits_from(lines_data, &mut bit_offset, htps) as u8;
        let rangelen = read_bits_from(lines_data, &mut bit_offset, htrs) as u8;
        lines.push(HuffmanLine {
            preflen,
            rangelen,
            rangelow: currangelow,
        });
        currangelow += 1i32 << rangelen;
    }

    // B.2 6-7) lower range
    let preflen = read_bits_from(lines_data, &mut bit_offset, htps) as u8;
    lines.push(HuffmanLine { preflen, rangelen: 32, rangelow: htlow - 1 });

    // B.2 8-9) upper range
    let preflen = read_bits_from(lines_data, &mut bit_offset, htps) as u8;
    lines.push(HuffmanLine { preflen, rangelen: 32, rangelow: hthigh });

    // B.2 10) OOB
    if htoob {
        let preflen = read_bits_from(lines_data, &mut bit_offset, htps) as u8;
        lines.push(HuffmanLine { preflen, rangelen: 0, rangelow: 0 });
    }

    Ok(HuffmanParams { htoob, lines })
}

fn read_bits_from(data: &[u8], bit_offset: &mut usize, n: u8) -> u32 {
    let mut result = 0u32;
    for _ in 0..n {
        let byte_idx = *bit_offset / 8;
        let bit_idx = 7 - (*bit_offset % 8);
        if byte_idx < data.len() {
            result = (result << 1) | ((data[byte_idx] >> bit_idx) & 1) as u32;
        } else {
            result <<= 1;
        }
        *bit_offset += 1;
    }
    result
}
