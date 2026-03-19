use crate::image::Jbig2Image;
use crate::symbol_dict::{SymbolDict, SymbolDictParams};

fn make_glyph(w: u32, h: u32, fill: u8) -> Jbig2Image {
    let mut img = Jbig2Image::new(w, h);
    if fill != 0 {
        img.clear(1);
    }
    img
}

// --- Step 8.1: 사전 구조 & 관리 ---

#[test]
fn create_empty() {
    let dict = SymbolDict::new(5);
    assert_eq!(dict.n_symbols(), 5);
    // All slots should be None
    for i in 0..5 {
        assert!(dict.glyph(i).is_none());
    }
}

#[test]
fn set_and_get_glyph() {
    let mut dict = SymbolDict::new(3);
    let glyph = make_glyph(4, 4, 1);
    dict.set_glyph(1, glyph);

    assert!(dict.glyph(0).is_none());
    assert!(dict.glyph(1).is_some());
    assert_eq!(dict.glyph(1).unwrap().width, 4);
    assert!(dict.glyph(2).is_none());
}

#[test]
fn cat_two_dicts() {
    let mut d1 = SymbolDict::new(2);
    d1.set_glyph(0, make_glyph(4, 4, 0));
    d1.set_glyph(1, make_glyph(8, 8, 1));

    let mut d2 = SymbolDict::new(1);
    d2.set_glyph(0, make_glyph(6, 6, 0));

    let merged = SymbolDict::cat(&[&d1, &d2]);
    assert_eq!(merged.n_symbols(), 3);
    assert_eq!(merged.glyph(0).unwrap().width, 4);
    assert_eq!(merged.glyph(1).unwrap().width, 8);
    assert_eq!(merged.glyph(2).unwrap().width, 6);
}

#[test]
fn cat_empty_dicts() {
    let d1 = SymbolDict::new(0);
    let d2 = SymbolDict::new(0);
    let merged = SymbolDict::cat(&[&d1, &d2]);
    assert_eq!(merged.n_symbols(), 0);
}

#[test]
fn export_symbols() {
    let mut input = SymbolDict::new(2);
    input.set_glyph(0, make_glyph(4, 4, 0));
    input.set_glyph(1, make_glyph(8, 8, 1));

    let mut new_syms = SymbolDict::new(2);
    new_syms.set_glyph(0, make_glyph(6, 6, 0));
    new_syms.set_glyph(1, make_glyph(10, 10, 1));

    // Export symbols 0 (input), 2 (new[0]), 3 (new[1]) — skip symbol 1
    let flags = vec![true, false, true, true];
    let exported = SymbolDict::export(&input, &new_syms, &flags);

    assert_eq!(exported.n_symbols(), 3);
    assert_eq!(exported.glyph(0).unwrap().width, 4);   // input[0]
    assert_eq!(exported.glyph(1).unwrap().width, 6);   // new[0]
    assert_eq!(exported.glyph(2).unwrap().width, 10);  // new[1]
}

#[test]
fn export_none() {
    let input = SymbolDict::new(2);
    let new_syms = SymbolDict::new(1);
    let flags = vec![false, false, false];
    let exported = SymbolDict::export(&input, &new_syms, &flags);
    assert_eq!(exported.n_symbols(), 0);
}

// --- Step 8.2 / 8.3: 파라미터 파싱 ---

#[test]
fn parse_params_arithmetic() {
    // flags: SDHUFF=0, SDREFAGG=0, SDTEMPLATE=2 (bits 10-11 = 10), SDRTEMPLATE=0
    // bits: 0000_1000_0000_0000 = 0x0800
    let mut data = vec![0x08, 0x00];
    // SDAT for template 2: 2 bytes
    data.extend_from_slice(&[2, 0xFF]); // sdat[0]=2, sdat[1]=-1
    // SDNUMEXSYMS = 10
    data.extend_from_slice(&10u32.to_be_bytes());
    // SDNUMNEWSYMS = 5
    data.extend_from_slice(&5u32.to_be_bytes());

    let (params, consumed) = SymbolDictParams::parse(&data).unwrap();
    assert!(!params.sdhuff);
    assert!(!params.sdrefagg);
    assert_eq!(params.sdtemplate, 2);
    assert_eq!(params.sdat[0], 2);
    assert_eq!(params.sdat[1], -1);
    assert_eq!(params.sdnumexsyms, 10);
    assert_eq!(params.sdnumnewsyms, 5);
    assert_eq!(consumed, 2 + 2 + 8); // flags + sdat(2) + exsyms + newsyms
}

#[test]
fn parse_params_huffman() {
    // flags: SDHUFF=1, SDREFAGG=0
    let mut data = vec![0x00, 0x01];
    // No SDAT (huffman mode)
    // SDNUMEXSYMS = 3, SDNUMNEWSYMS = 3
    data.extend_from_slice(&3u32.to_be_bytes());
    data.extend_from_slice(&3u32.to_be_bytes());

    let (params, consumed) = SymbolDictParams::parse(&data).unwrap();
    assert!(params.sdhuff);
    assert_eq!(params.sdnumexsyms, 3);
    assert_eq!(params.sdnumnewsyms, 3);
    assert_eq!(consumed, 2 + 8); // flags + exsyms + newsyms (no sdat)
}

#[test]
fn parse_params_with_refagg() {
    // flags: SDHUFF=0, SDREFAGG=1, SDTEMPLATE=0, SDRTEMPLATE=0
    // → need 8 bytes SDAT + 4 bytes SDRAT
    let mut data = vec![0x00, 0x02]; // SDREFAGG=1
    // SDAT for template 0: 8 bytes
    data.extend_from_slice(&[3, 0xFF, 0xFD, 0xFF, 2, 0xFE, 0xFE, 0xFE]);
    // SDRAT for sdrtemplate 0: 4 bytes
    data.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
    // SDNUMEXSYMS, SDNUMNEWSYMS
    data.extend_from_slice(&1u32.to_be_bytes());
    data.extend_from_slice(&1u32.to_be_bytes());

    let (params, consumed) = SymbolDictParams::parse(&data).unwrap();
    assert!(params.sdrefagg);
    assert_eq!(params.sdtemplate, 0);
    assert_eq!(params.sdrtemplate, 0);
    assert_eq!(consumed, 2 + 8 + 4 + 8);
}
