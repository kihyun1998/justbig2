use crate::arith::ArithState;
use crate::image::{ComposeOp, Jbig2Image};
use crate::symbol_dict::SymbolDict;
use crate::text::{decode_text_region, RefCorner, TextRegionParams};

const TEST_STREAM: &[u8] = &[
    0x84, 0xC7, 0x3B, 0xFC, 0xE1, 0xA1, 0x43, 0x04, 0x02, 0x20, 0x00, 0x00,
    0x41, 0x0D, 0xBB, 0x86, 0xF4, 0x31, 0x7F, 0xFF, 0x88, 0xFF, 0x37, 0x47,
    0x1A, 0xDB, 0x6A, 0xDF, 0xFF, 0xAC, 0x00, 0x00,
];

fn make_dict_with_glyphs() -> SymbolDict {
    let mut dict = SymbolDict::new(4);
    for i in 0..4 {
        let mut g = Jbig2Image::new(4, 6);
        // Each glyph has a distinct pattern
        g.set_pixel(i, 0, 1);
        dict.set_glyph(i, g);
    }
    dict
}

fn make_default_params(n_instances: u32) -> TextRegionParams {
    TextRegionParams {
        sbhuff: false,
        sbrefine: false,
        sbdefpixel: false,
        sbcombop: ComposeOp::Or,
        transposed: false,
        refcorner: RefCorner::TopLeft,
        sbdsoffset: 0,
        sbnuminstances: n_instances,
        logsbstrips: 0,
        sbstrips: 1,
        sbrtemplate: 0,
        sbrat: [0; 4],
    }
}

// --- Step 9.1: 파라미터 파싱 ---

#[test]
fn parse_params() {
    // flags: SBHUFF=0, SBREFINE=0, LOGSBSTRIPS=0, REFCORNER=TOPLEFT(1),
    //        TRANSPOSED=0, SBCOMBOP=OR(0), SBDEFPIXEL=0, SBDSOFFSET=0, SBRTEMPLATE=0
    // = 0b0_00000_0_00_0_0_01_00_0_0 = 0x0010
    let mut data = vec![0x00, 0x10];
    // No huffman flags, no sbrat
    // SBNUMINSTANCES = 5
    data.extend_from_slice(&5u32.to_be_bytes());

    let (params, consumed) = TextRegionParams::parse(&data).unwrap();
    assert!(!params.sbhuff);
    assert!(!params.sbrefine);
    assert_eq!(params.refcorner, RefCorner::TopLeft);
    assert!(!params.transposed);
    assert_eq!(params.sbnuminstances, 5);
    assert_eq!(consumed, 6); // 2 flags + 4 instances
}

#[test]
fn parse_params_transposed() {
    // TRANSPOSED=1 (bit 6), REFCORNER=BOTTOMRIGHT(3, bits 4-5)
    // Byte layout: flags[0] = high byte, flags[1] = low byte (big-endian)
    // bit 6 = TRANSPOSED, bits 4-5 = REFCORNER=3
    // low byte: 0b0_1_11_00_0_0 = 0x70
    let mut data = vec![0x00, 0x70];
    data.extend_from_slice(&1u32.to_be_bytes());

    let (params, _) = TextRegionParams::parse(&data).unwrap();
    assert!(params.transposed);
    assert_eq!(params.refcorner, RefCorner::TopRight);
}

#[test]
fn parse_params_with_refinement() {
    // SBREFINE=1, SBRTEMPLATE=0 → need 4 bytes sbrat
    // flags: 0b0_00000_0_00_0_0_01_00_1_0 = 0x0012
    let mut data = vec![0x00, 0x12];
    // sbrat
    data.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
    // SBNUMINSTANCES
    data.extend_from_slice(&2u32.to_be_bytes());

    let (params, consumed) = TextRegionParams::parse(&data).unwrap();
    assert!(params.sbrefine);
    assert_eq!(params.sbrat[0], -1);
    assert_eq!(consumed, 2 + 4 + 4);
}

// --- Step 9.2: 산술 코딩 텍스트 디코딩 ---

#[test]
fn decode_arithmetic_basic() {
    let dict = make_dict_with_glyphs();
    let params = make_default_params(3);
    let mut as_ = ArithState::new(TEST_STREAM).unwrap();
    let mut image = Jbig2Image::new(32, 16);

    let result = decode_text_region(&params, &mut as_, &mut image, &[&dict], 4);
    assert!(result.is_ok());
}

#[test]
fn decode_transposed() {
    let dict = make_dict_with_glyphs();
    let mut params = make_default_params(2);
    params.transposed = true;
    params.refcorner = RefCorner::BottomLeft;

    let mut as_ = ArithState::new(TEST_STREAM).unwrap();
    let mut image = Jbig2Image::new(32, 32);

    decode_text_region(&params, &mut as_, &mut image, &[&dict], 4).unwrap();
}

#[test]
fn decode_all_refcorners() {
    let dict = make_dict_with_glyphs();
    for corner in [RefCorner::TopLeft, RefCorner::TopRight, RefCorner::BottomLeft, RefCorner::BottomRight] {
        let mut params = make_default_params(2);
        params.refcorner = corner;

        let mut as_ = ArithState::new(TEST_STREAM).unwrap();
        let mut image = Jbig2Image::new(32, 16);

        decode_text_region(&params, &mut as_, &mut image, &[&dict], 4)
            .unwrap_or_else(|e| panic!("refcorner {:?} failed: {e}", corner));
    }
}

#[test]
fn decode_zero_instances() {
    let dict = make_dict_with_glyphs();
    let params = make_default_params(0);
    let mut as_ = ArithState::new(TEST_STREAM).unwrap();
    let mut image = Jbig2Image::new(8, 8);

    decode_text_region(&params, &mut as_, &mut image, &[&dict], 4).unwrap();
    // No instances → image unchanged (all white)
    assert!(image.data.iter().all(|&b| b == 0));
}

#[test]
fn decode_multi_dict() {
    let d1 = make_dict_with_glyphs();
    let mut d2 = SymbolDict::new(2);
    d2.set_glyph(0, Jbig2Image::new(3, 3));
    d2.set_glyph(1, Jbig2Image::new(5, 5));

    let params = make_default_params(2);
    let mut as_ = ArithState::new(TEST_STREAM).unwrap();
    let mut image = Jbig2Image::new(32, 16);

    // Total symbols: 4 + 2 = 6
    decode_text_region(&params, &mut as_, &mut image, &[&d1, &d2], 6).unwrap();
}

// --- Step 9.3: 허프만 텍스트 (unsupported, returns error) ---

#[test]
fn decode_huffman_returns_unsupported() {
    let dict = make_dict_with_glyphs();
    let mut params = make_default_params(1);
    params.sbhuff = true;

    let mut as_ = ArithState::new(TEST_STREAM).unwrap();
    let mut image = Jbig2Image::new(8, 8);

    let result = decode_text_region(&params, &mut as_, &mut image, &[&dict], 4);
    assert!(result.is_err());
}

// --- Step 9.4: 리파인먼트 플래그 ---

#[test]
fn decode_with_refinement_flag() {
    let dict = make_dict_with_glyphs();
    let mut params = make_default_params(2);
    params.sbrefine = true;

    let mut as_ = ArithState::new(TEST_STREAM).unwrap();
    let mut image = Jbig2Image::new(32, 16);

    // Should not panic — refinement indicator is decoded but actual refinement is skipped
    decode_text_region(&params, &mut as_, &mut image, &[&dict], 4).unwrap();
}
