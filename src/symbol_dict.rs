//! Symbol Dictionary (ITU T.88 6.5, 7.4.2).
//!
//! A symbol dictionary contains an array of glyph bitmaps that can be
//! referenced by text region segments for character placement.

use crate::image::Jbig2Image;

/// A symbol dictionary — an array of glyph images.
#[derive(Debug, Clone)]
pub struct SymbolDict {
    pub glyphs: Vec<Option<Jbig2Image>>,
}

impl SymbolDict {
    /// Create a new empty symbol dictionary with `n` slots.
    pub fn new(n: u32) -> Self {
        SymbolDict {
            glyphs: vec![None; n as usize],
        }
    }

    /// Number of symbols in this dictionary.
    pub fn n_symbols(&self) -> u32 {
        self.glyphs.len() as u32
    }

    /// Get a glyph by index.
    pub fn glyph(&self, id: u32) -> Option<&Jbig2Image> {
        self.glyphs.get(id as usize).and_then(|g| g.as_ref())
    }

    /// Set a glyph at the given index.
    pub fn set_glyph(&mut self, id: u32, image: Jbig2Image) {
        if (id as usize) < self.glyphs.len() {
            self.glyphs[id as usize] = Some(image);
        }
    }

    /// Concatenate multiple symbol dictionaries into one.
    pub fn cat(dicts: &[&SymbolDict]) -> Self {
        let total: usize = dicts.iter().map(|d| d.glyphs.len()).sum();
        let mut result = SymbolDict {
            glyphs: Vec::with_capacity(total),
        };
        for dict in dicts {
            for glyph in &dict.glyphs {
                result.glyphs.push(glyph.clone());
            }
        }
        result
    }

    /// Select exported symbols according to the JBIG2 export algorithm.
    ///
    /// Given the input symbols and new symbols, produces the export list
    /// by toggling an export flag for runs of symbols.
    /// `flags[i]` = true means symbol i is exported.
    pub fn export(
        input_syms: &SymbolDict,
        new_syms: &SymbolDict,
        export_flags: &[bool],
    ) -> Self {
        let total = input_syms.n_symbols() + new_syms.n_symbols();
        let mut result = Vec::new();

        for i in 0..total as usize {
            if i < export_flags.len() && export_flags[i] {
                let glyph = if (i as u32) < input_syms.n_symbols() {
                    input_syms.glyph(i as u32).cloned()
                } else {
                    new_syms.glyph(i as u32 - input_syms.n_symbols()).cloned()
                };
                result.push(glyph);
            }
        }

        SymbolDict { glyphs: result }
    }
}

/// Symbol dictionary segment flags (Table 13).
#[derive(Debug, Clone)]
pub struct SymbolDictParams {
    pub sdhuff: bool,
    pub sdrefagg: bool,
    pub sdtemplate: u8,
    pub sdrtemplate: u8,
    pub sdat: [i8; 8],
    pub sdrat: [i8; 4],
    pub sdnuminsyms: u32,
    pub sdnumnewsyms: u32,
    pub sdnumexsyms: u32,
}

impl SymbolDictParams {
    /// Parse symbol dictionary flags from segment data.
    /// Returns (params, bytes consumed after region header).
    pub fn parse(data: &[u8]) -> Option<(Self, usize)> {
        // Minimum: 2 bytes flags + 4 bytes SDNUMEXSYMS + 4 bytes SDNUMNEWSYMS = 10
        if data.len() < 10 {
            return None;
        }

        let flags = u16::from_be_bytes([data[0], data[1]]);
        let sdhuff = flags & 1 != 0;
        let sdrefagg = (flags >> 1) & 1 != 0;
        let sdtemplate = ((flags >> 10) & 3) as u8;
        let sdrtemplate = ((flags >> 12) & 1) as u8;

        let mut offset = 2;

        // SDAT (adaptive template)
        let mut sdat = [0i8; 8];
        if !sdhuff {
            let n = if sdtemplate == 0 { 8 } else { 2 };
            if data.len() < offset + n {
                return None;
            }
            for i in 0..n {
                sdat[i] = data[offset + i] as i8;
            }
            offset += n;
        }

        // SDRAT (refinement adaptive template)
        let mut sdrat = [0i8; 4];
        if sdrefagg && sdrtemplate == 0 {
            if data.len() < offset + 4 {
                return None;
            }
            for i in 0..4 {
                sdrat[i] = data[offset + i] as i8;
            }
            offset += 4;
        }

        // SDNUMEXSYMS and SDNUMNEWSYMS
        if data.len() < offset + 8 {
            return None;
        }
        let sdnumexsyms = u32::from_be_bytes([
            data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
        ]);
        offset += 4;
        let sdnumnewsyms = u32::from_be_bytes([
            data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
        ]);
        offset += 4;

        Some((
            SymbolDictParams {
                sdhuff,
                sdrefagg,
                sdtemplate,
                sdrtemplate,
                sdat,
                sdrat,
                sdnuminsyms: 0, // set by caller from referred segments
                sdnumnewsyms,
                sdnumexsyms,
            },
            offset,
        ))
    }
}
