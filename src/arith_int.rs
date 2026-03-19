//! Arithmetic integer decoder (ITU T.88 Annex A.2).
//!
//! Ported from jbig2dec/jbig2_arith_int.c.

use crate::arith::{ArithCx, ArithState};
use crate::error::Result;

/// Arithmetic integer decoding context (512 sub-contexts).
pub struct ArithIntCtx {
    iax: [ArithCx; 512],
}

impl ArithIntCtx {
    pub fn new() -> Self {
        ArithIntCtx { iax: [0u8; 512] }
    }

    /// Decode an integer value. Returns `None` for OOB (Out of Bounds).
    pub fn decode(&mut self, as_: &mut ArithState) -> Result<Option<i32>> {
        let iax = &mut self.iax;
        let mut prev: usize = 1;

        // Decode sign bit S
        let s = as_.decode(&mut iax[prev])? as usize;
        prev = (prev << 1) | s;

        // Binary tree to determine tail length and offset
        let bit = as_.decode(&mut iax[prev])? as usize;
        prev = (prev << 1) | bit;

        let (n_tail, offset): (u8, i32) = if bit == 0 {
            (2, 0)
        } else {
            let bit = as_.decode(&mut iax[prev])? as usize;
            prev = (prev << 1) | bit;
            if bit == 0 {
                (4, 4)
            } else {
                let bit = as_.decode(&mut iax[prev])? as usize;
                prev = (prev << 1) | bit;
                if bit == 0 {
                    (6, 20)
                } else {
                    let bit = as_.decode(&mut iax[prev])? as usize;
                    prev = (prev << 1) | bit;
                    if bit == 0 {
                        (8, 84)
                    } else {
                        let bit = as_.decode(&mut iax[prev])? as usize;
                        prev = (prev << 1) | bit;
                        if bit == 0 {
                            (12, 340)
                        } else {
                            (32, 4436)
                        }
                    }
                }
            }
        };

        // Decode V from n_tail bits
        let mut v: i64 = 0;
        for _ in 0..n_tail {
            let bit = as_.decode(&mut iax[prev])? as usize;
            prev = ((prev << 1) & 511) | (prev & 256) | bit;
            v = (v << 1) | bit as i64;
        }

        // Clamp to i32 range
        v += offset as i64;
        let v = v.min(i32::MAX as i64) as i32;

        // Apply sign
        let v = if s != 0 { -v } else { v };

        // OOB: S=1 and V=0
        if s != 0 && v == 0 {
            Ok(None)
        } else {
            Ok(Some(v))
        }
    }
}
