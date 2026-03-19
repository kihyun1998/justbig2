//! QM arithmetic decoder (ITU T.88 Annex E / Figure F.2).
//!
//! Ported from jbig2dec/jbig2_arith.c.

use crate::error::{Jbig2Error, Result};

/// An arithmetic coding context: 7-bit index (low bits) + 1-bit MPS (high bit).
pub type ArithCx = u8;

/// Qe table entry.
struct QeEntry {
    qe: u16,
    mps_xor: u8,
    lps_xor: u8,
}

macro_rules! mps {
    ($index:expr, $nmps:expr) => {
        ($index ^ $nmps)
    };
}

macro_rules! lps {
    ($index:expr, $nlps:expr, $swtch:expr) => {
        ($index ^ $nlps ^ ($swtch << 7))
    };
}

static QE_TABLE: [QeEntry; 47] = [
    QeEntry { qe: 0x5601, mps_xor: mps!(0, 1), lps_xor: lps!(0, 1, 1) },
    QeEntry { qe: 0x3401, mps_xor: mps!(1, 2), lps_xor: lps!(1, 6, 0) },
    QeEntry { qe: 0x1801, mps_xor: mps!(2, 3), lps_xor: lps!(2, 9, 0) },
    QeEntry { qe: 0x0AC1, mps_xor: mps!(3, 4), lps_xor: lps!(3, 12, 0) },
    QeEntry { qe: 0x0521, mps_xor: mps!(4, 5), lps_xor: lps!(4, 29, 0) },
    QeEntry { qe: 0x0221, mps_xor: mps!(5, 38), lps_xor: lps!(5, 33, 0) },
    QeEntry { qe: 0x5601, mps_xor: mps!(6, 7), lps_xor: lps!(6, 6, 1) },
    QeEntry { qe: 0x5401, mps_xor: mps!(7, 8), lps_xor: lps!(7, 14, 0) },
    QeEntry { qe: 0x4801, mps_xor: mps!(8, 9), lps_xor: lps!(8, 14, 0) },
    QeEntry { qe: 0x3801, mps_xor: mps!(9, 10), lps_xor: lps!(9, 14, 0) },
    QeEntry { qe: 0x3001, mps_xor: mps!(10, 11), lps_xor: lps!(10, 17, 0) },
    QeEntry { qe: 0x2401, mps_xor: mps!(11, 12), lps_xor: lps!(11, 18, 0) },
    QeEntry { qe: 0x1C01, mps_xor: mps!(12, 13), lps_xor: lps!(12, 20, 0) },
    QeEntry { qe: 0x1601, mps_xor: mps!(13, 29), lps_xor: lps!(13, 21, 0) },
    QeEntry { qe: 0x5601, mps_xor: mps!(14, 15), lps_xor: lps!(14, 14, 1) },
    QeEntry { qe: 0x5401, mps_xor: mps!(15, 16), lps_xor: lps!(15, 14, 0) },
    QeEntry { qe: 0x5101, mps_xor: mps!(16, 17), lps_xor: lps!(16, 15, 0) },
    QeEntry { qe: 0x4801, mps_xor: mps!(17, 18), lps_xor: lps!(17, 16, 0) },
    QeEntry { qe: 0x3801, mps_xor: mps!(18, 19), lps_xor: lps!(18, 17, 0) },
    QeEntry { qe: 0x3401, mps_xor: mps!(19, 20), lps_xor: lps!(19, 18, 0) },
    QeEntry { qe: 0x3001, mps_xor: mps!(20, 21), lps_xor: lps!(20, 19, 0) },
    QeEntry { qe: 0x2801, mps_xor: mps!(21, 22), lps_xor: lps!(21, 19, 0) },
    QeEntry { qe: 0x2401, mps_xor: mps!(22, 23), lps_xor: lps!(22, 20, 0) },
    QeEntry { qe: 0x2201, mps_xor: mps!(23, 24), lps_xor: lps!(23, 21, 0) },
    QeEntry { qe: 0x1C01, mps_xor: mps!(24, 25), lps_xor: lps!(24, 22, 0) },
    QeEntry { qe: 0x1801, mps_xor: mps!(25, 26), lps_xor: lps!(25, 23, 0) },
    QeEntry { qe: 0x1601, mps_xor: mps!(26, 27), lps_xor: lps!(26, 24, 0) },
    QeEntry { qe: 0x1401, mps_xor: mps!(27, 28), lps_xor: lps!(27, 25, 0) },
    QeEntry { qe: 0x1201, mps_xor: mps!(28, 29), lps_xor: lps!(28, 26, 0) },
    QeEntry { qe: 0x1101, mps_xor: mps!(29, 30), lps_xor: lps!(29, 27, 0) },
    QeEntry { qe: 0x0AC1, mps_xor: mps!(30, 31), lps_xor: lps!(30, 28, 0) },
    QeEntry { qe: 0x09C1, mps_xor: mps!(31, 32), lps_xor: lps!(31, 29, 0) },
    QeEntry { qe: 0x08A1, mps_xor: mps!(32, 33), lps_xor: lps!(32, 30, 0) },
    QeEntry { qe: 0x0521, mps_xor: mps!(33, 34), lps_xor: lps!(33, 31, 0) },
    QeEntry { qe: 0x0441, mps_xor: mps!(34, 35), lps_xor: lps!(34, 32, 0) },
    QeEntry { qe: 0x02A1, mps_xor: mps!(35, 36), lps_xor: lps!(35, 33, 0) },
    QeEntry { qe: 0x0221, mps_xor: mps!(36, 37), lps_xor: lps!(36, 34, 0) },
    QeEntry { qe: 0x0141, mps_xor: mps!(37, 38), lps_xor: lps!(37, 35, 0) },
    QeEntry { qe: 0x0111, mps_xor: mps!(38, 39), lps_xor: lps!(38, 36, 0) },
    QeEntry { qe: 0x0085, mps_xor: mps!(39, 40), lps_xor: lps!(39, 37, 0) },
    QeEntry { qe: 0x0049, mps_xor: mps!(40, 41), lps_xor: lps!(40, 38, 0) },
    QeEntry { qe: 0x0025, mps_xor: mps!(41, 42), lps_xor: lps!(41, 39, 0) },
    QeEntry { qe: 0x0015, mps_xor: mps!(42, 43), lps_xor: lps!(42, 40, 0) },
    QeEntry { qe: 0x0009, mps_xor: mps!(43, 44), lps_xor: lps!(43, 41, 0) },
    QeEntry { qe: 0x0005, mps_xor: mps!(44, 45), lps_xor: lps!(44, 42, 0) },
    QeEntry { qe: 0x0001, mps_xor: mps!(45, 45), lps_xor: lps!(45, 43, 0) },
    QeEntry { qe: 0x5601, mps_xor: mps!(46, 46), lps_xor: lps!(46, 46, 0) },
];

/// QM arithmetic decoder state.
///
/// Operates over an in-memory byte slice using the word-stream approach
/// from the jbig2dec reference implementation.
pub struct ArithState<'a> {
    data: &'a [u8],
    /// C register (code register).
    c: u32,
    /// A register (interval/range register).
    a: u32,
    /// Count of valid bits remaining before next byte-in.
    ct: i32,
    /// Pre-fetched next 4 bytes (big-endian).
    next_word: u32,
    /// Number of valid bytes in next_word (1..=4).
    next_word_bytes: usize,
    /// Current read offset in data.
    offset: usize,
    /// Whether an error/end has been hit.
    err: bool,
}

impl<'a> ArithState<'a> {
    /// Create a new arithmetic decoder state from a byte slice.
    pub fn new(data: &'a [u8]) -> Result<Self> {
        if data.is_empty() {
            return Err(Jbig2Error::InvalidData(
                "empty data for arithmetic decoder".into(),
            ));
        }

        let mut state = ArithState {
            data,
            c: 0,
            a: 0,
            ct: 0,
            next_word: 0,
            next_word_bytes: 0,
            offset: 0,
            err: false,
        };

        // Read first word
        state.next_word_bytes = state.fetch_word();
        if state.next_word_bytes == 0 {
            return Err(Jbig2Error::InvalidData(
                "cannot read first byte from arithmetic stream".into(),
            ));
        }
        state.offset += state.next_word_bytes;

        // Figure F.1: init C from first byte
        state.c = (!(state.next_word >> 8)) & 0xFF0000;

        // Figure E.20 (2): first bytein
        state.bytein()?;

        // Figure E.20 (3)
        state.c <<= 7;
        state.ct -= 7;
        state.a = 0x8000;

        Ok(state)
    }

    /// Fetch up to 4 bytes from data[offset..] into next_word.
    /// Returns the number of bytes read.
    fn fetch_word(&mut self) -> usize {
        let off = self.offset;
        let remaining = self.data.len().saturating_sub(off);
        let n = remaining.min(4);
        if n == 0 {
            return 0;
        }
        let mut val = 0u32;
        for i in 0..n {
            val |= (self.data[off + i] as u32) << (24 - i * 8);
        }
        self.next_word = val;
        n
    }

    /// BYTEIN procedure (Figure F.3).
    fn bytein(&mut self) -> Result<()> {
        if self.err || self.next_word_bytes == 0 {
            return Err(Jbig2Error::InvalidData(
                "read past end of arithmetic stream".into(),
            ));
        }

        let b = ((self.next_word >> 24) & 0xFF) as u8;

        if b == 0xFF {
            if self.next_word_bytes <= 1 {
                // Need to fetch more data to check marker
                let n = {
                    let remaining = self.data.len().saturating_sub(self.offset);
                    let count = remaining.min(4);
                    if count > 0 {
                        let mut val = 0u32;
                        for i in 0..count {
                            val |= (self.data[self.offset + i] as u32) << (24 - i * 8);
                        }
                        self.next_word = val;
                    }
                    count
                };
                self.next_word_bytes = n;

                if n == 0 {
                    // Assume terminating marker
                    self.next_word = 0xFF90_0000;
                    self.next_word_bytes = 2;
                    self.c += 0xFF00;
                    self.ct = 8;
                    return Ok(());
                }

                self.offset += n;
                let b1 = ((self.next_word >> 24) & 0xFF) as u8;
                if b1 > 0x8F {
                    // Terminating marker code
                    self.ct = 8;
                    self.next_word = 0xFF00_0000 | (self.next_word >> 8);
                    self.next_word_bytes = 2;
                    self.offset -= 1;
                } else {
                    self.c = self.c.wrapping_add(0xFE00u32.wrapping_sub((b1 as u32) << 9));
                    self.ct = 7;
                }
            } else {
                let b1 = ((self.next_word >> 16) & 0xFF) as u8;
                if b1 > 0x8F {
                    // Terminating marker
                    self.ct = 8;
                } else {
                    self.next_word_bytes -= 1;
                    self.next_word <<= 8;
                    self.c = self.c.wrapping_add(0xFE00u32.wrapping_sub((b1 as u32) << 9));
                    self.ct = 7;
                }
            }
        } else {
            // Normal byte
            self.next_word <<= 8;
            self.next_word_bytes -= 1;

            if self.next_word_bytes == 0 {
                let remaining = self.data.len().saturating_sub(self.offset);
                let n = remaining.min(4);
                if n > 0 {
                    let mut val = 0u32;
                    for i in 0..n {
                        val |= (self.data[self.offset + i] as u32) << (24 - i * 8);
                    }
                    self.next_word = val;
                    self.next_word_bytes = n;
                    self.offset += n;
                } else {
                    // Assume terminating marker
                    self.next_word = 0xFF90_0000;
                    self.next_word_bytes = 2;
                    self.c += 0xFF00;
                    self.ct = 8;
                    return Ok(());
                }
            }

            let nb = ((self.next_word >> 24) & 0xFF) as u8;
            self.c = self.c.wrapping_add(0xFF00u32.wrapping_sub((nb as u32) << 8));
            self.ct = 8;
        }

        Ok(())
    }

    /// Renormalization (Figure E.18).
    fn renormd(&mut self) -> Result<()> {
        loop {
            if self.ct == 0 {
                self.bytein()?;
            }
            self.a <<= 1;
            self.c <<= 1;
            self.ct -= 1;
            if (self.a & 0x8000) != 0 {
                break;
            }
        }
        Ok(())
    }

    /// Decode a single bit (Figure F.2).
    pub fn decode(&mut self, cx: &mut ArithCx) -> Result<u8> {
        let index = (*cx & 0x7F) as usize;
        if index >= QE_TABLE.len() {
            return Err(Jbig2Error::InvalidData(
                "arithmetic context index out of range".into(),
            ));
        }

        let pqe = &QE_TABLE[index];
        let qe = pqe.qe as u32;

        self.a -= qe;
        if (self.c >> 16) < self.a {
            // MPS path
            if (self.a & 0x8000) == 0 {
                // MPS_EXCHANGE (Figure E.16)
                let d = if self.a < qe {
                    // Conditional exchange: output LPS
                    let d = 1 - (*cx >> 7);
                    *cx ^= pqe.lps_xor;
                    d
                } else {
                    let d = *cx >> 7;
                    *cx ^= pqe.mps_xor;
                    d
                };
                self.renormd()?;
                Ok(d)
            } else {
                Ok(*cx >> 7)
            }
        } else {
            // LPS path
            self.c -= (self.a) << 16;
            // LPS_EXCHANGE (Figure E.17)
            let d = if self.a < qe {
                self.a = qe;
                let d = *cx >> 7;
                *cx ^= pqe.mps_xor;
                d
            } else {
                self.a = qe;
                let d = 1 - (*cx >> 7);
                *cx ^= pqe.lps_xor;
                d
            };
            self.renormd()?;
            Ok(d)
        }
    }
}
