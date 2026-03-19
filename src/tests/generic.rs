use crate::arith::ArithState;
use crate::generic::{decode_generic_region, stats_size, GenericRegionParams};
use crate::image::Jbig2Image;

/// The jbig2dec test stream — used as entropy source.
const TEST_STREAM: &[u8] = &[
    0x84, 0xC7, 0x3B, 0xFC, 0xE1, 0xA1, 0x43, 0x04, 0x02, 0x20, 0x00, 0x00,
    0x41, 0x0D, 0xBB, 0x86, 0xF4, 0x31, 0x7F, 0xFF, 0x88, 0xFF, 0x37, 0x47,
    0x1A, 0xDB, 0x6A, 0xDF, 0xFF, 0xAC, 0x00, 0x00,
];

fn make_default_params(template: u8) -> GenericRegionParams {
    let mut p = GenericRegionParams {
        mmr: false,
        gb_template: template,
        tpgdon: false,
        use_skip: false,
        gbat: [0i8; 8],
    };
    match template {
        0 => p.gbat = [3, -1, -3, -1, 2, -2, -2, -2],
        1 => { p.gbat[0] = 3; p.gbat[1] = -1; }
        2 => { p.gbat[0] = 2; p.gbat[1] = -1; }
        3 => { p.gbat[0] = 2; p.gbat[1] = -1; }
        _ => {}
    }
    p
}

// --- Step 5.1: params parsing ---

#[test]
fn parse_params() {
    let flags = 0b0000_0101; // MMR=1, GBTEMPLATE=2, TPGDON=0
    let (p, gbat_size) = GenericRegionParams::parse(flags);
    assert!(p.mmr);
    assert_eq!(p.gb_template, 2);
    assert!(!p.tpgdon);
    assert_eq!(gbat_size, 0); // MMR → no GBAT

    let flags2 = 0b0000_1000; // MMR=0, GBTEMPLATE=0, TPGDON=1
    let (p2, gbat_size2) = GenericRegionParams::parse(flags2);
    assert!(!p2.mmr);
    assert_eq!(p2.gb_template, 0);
    assert!(p2.tpgdon);
    assert_eq!(gbat_size2, 8);

    let flags3 = 0b0000_0010; // MMR=0, GBTEMPLATE=1
    let (_, gbat_size3) = GenericRegionParams::parse(flags3);
    assert_eq!(gbat_size3, 2);
}

// --- Step 5.2: Template 0 ---

#[test]
fn template0_basic() {
    let params = make_default_params(0);
    let mut as_ = ArithState::new(TEST_STREAM).unwrap();
    let mut image = Jbig2Image::new(8, 4);
    let mut gb_stats = vec![0u8; stats_size(0)];

    let result = decode_generic_region(&params, &mut as_, &mut image, &mut gb_stats);
    assert!(result.is_ok());
    // Should have decoded something (not all zeros since the stream has data)
}

#[test]
fn template0_deterministic() {
    let params = make_default_params(0);

    let mut as1 = ArithState::new(TEST_STREAM).unwrap();
    let mut img1 = Jbig2Image::new(8, 4);
    let mut stats1 = vec![0u8; stats_size(0)];
    decode_generic_region(&params, &mut as1, &mut img1, &mut stats1).unwrap();

    let mut as2 = ArithState::new(TEST_STREAM).unwrap();
    let mut img2 = Jbig2Image::new(8, 4);
    let mut stats2 = vec![0u8; stats_size(0)];
    decode_generic_region(&params, &mut as2, &mut img2, &mut stats2).unwrap();

    assert_eq!(img1.data, img2.data);
}

#[test]
fn template0_unopt() {
    // Use non-default GBAT to trigger unoptimized path
    let mut params = make_default_params(0);
    params.gbat[0] = 2; // non-default
    let mut as_ = ArithState::new(TEST_STREAM).unwrap();
    let mut image = Jbig2Image::new(8, 4);
    let mut gb_stats = vec![0u8; stats_size(0)];

    let result = decode_generic_region(&params, &mut as_, &mut image, &mut gb_stats);
    assert!(result.is_ok());
}

// --- Step 5.3: Templates 1, 2, 3 ---

#[test]
fn template1_basic() {
    let params = make_default_params(1);
    let mut as_ = ArithState::new(TEST_STREAM).unwrap();
    let mut image = Jbig2Image::new(16, 4);
    let mut gb_stats = vec![0u8; stats_size(1)];

    decode_generic_region(&params, &mut as_, &mut image, &mut gb_stats).unwrap();
}

#[test]
fn template2_basic() {
    let params = make_default_params(2);
    let mut as_ = ArithState::new(TEST_STREAM).unwrap();
    let mut image = Jbig2Image::new(16, 4);
    let mut gb_stats = vec![0u8; stats_size(2)];

    decode_generic_region(&params, &mut as_, &mut image, &mut gb_stats).unwrap();
}

#[test]
fn template3_basic() {
    let params = make_default_params(3);
    let mut as_ = ArithState::new(TEST_STREAM).unwrap();
    let mut image = Jbig2Image::new(16, 4);
    let mut gb_stats = vec![0u8; stats_size(3)];

    decode_generic_region(&params, &mut as_, &mut image, &mut gb_stats).unwrap();
}

// --- Step 5.4: TPGD ---

#[test]
fn tpgd_skip_identical_rows() {
    let mut params = make_default_params(2);
    params.tpgdon = true;

    let mut as_ = ArithState::new(TEST_STREAM).unwrap();
    let mut image = Jbig2Image::new(8, 8);
    let mut gb_stats = vec![0u8; stats_size(2)];

    let result = decode_generic_region(&params, &mut as_, &mut image, &mut gb_stats);
    assert!(result.is_ok());
}

#[test]
fn tpgd_all_templates() {
    for t in 0..4u8 {
        let mut params = make_default_params(t);
        params.tpgdon = true;

        let mut as_ = ArithState::new(TEST_STREAM).unwrap();
        let mut image = Jbig2Image::new(8, 4);
        let mut gb_stats = vec![0u8; stats_size(t)];

        decode_generic_region(&params, &mut as_, &mut image, &mut gb_stats)
            .unwrap_or_else(|e| panic!("TPGD template {t} failed: {e}"));
    }
}

// --- Stats size ---

#[test]
fn stats_size_values() {
    assert_eq!(stats_size(0), 65536);
    assert_eq!(stats_size(1), 8192);
    assert_eq!(stats_size(2), 1024);
    assert_eq!(stats_size(3), 1024);
}

// --- Zero-size images ---

#[test]
fn zero_width_image() {
    let params = make_default_params(0);
    let mut as_ = ArithState::new(TEST_STREAM).unwrap();
    let mut image = Jbig2Image::new(0, 4);
    let mut gb_stats = vec![0u8; stats_size(0)];

    decode_generic_region(&params, &mut as_, &mut image, &mut gb_stats).unwrap();
}
