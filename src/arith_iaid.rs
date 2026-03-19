//! Arithmetic IAID (symbol ID) decoder (ITU T.88 Annex A.3).
//!
//! Ported from jbig2dec/jbig2_arith_iaid.c.

use crate::arith::{ArithCx, ArithState};
use crate::error::{Jbig2Error, Result};

/// IAID (Index of A IDentifier) decoding context.
pub struct ArithIaidCtx {
    sbsymcodelen: u8,
    iaidx: Vec<ArithCx>,
}

impl ArithIaidCtx {
    /// Create a new IAID context for the given symbol code length.
    /// `sbsymcodelen` must be <= 30.
    pub fn new(sbsymcodelen: u8) -> Result<Self> {
        if sbsymcodelen > 30 {
            return Err(Jbig2Error::InvalidData(
                "SBSYMCODELEN too large for IAID context".into(),
            ));
        }
        let size = 1usize << sbsymcodelen;
        Ok(ArithIaidCtx {
            sbsymcodelen,
            iaidx: vec![0u8; size],
        })
    }

    /// Decode a symbol ID. Returns a value in [0, 2^SBSYMCODELEN).
    pub fn decode(&mut self, as_: &mut ArithState) -> Result<u32> {
        let mut prev: usize = 1;

        for _ in 0..self.sbsymcodelen {
            let d = as_.decode(&mut self.iaidx[prev])? as usize;
            prev = (prev << 1) | d;
        }

        prev -= 1usize << self.sbsymcodelen;
        Ok(prev as u32)
    }
}
