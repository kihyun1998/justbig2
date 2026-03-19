use crate::arith::ArithState;
use crate::image::Jbig2Image;
use crate::refinement::{decode_refinement_region, refinement_stats_size, RefinementRegionParams};

const TEST_STREAM: &[u8] = &[
    0x84, 0xC7, 0x3B, 0xFC, 0xE1, 0xA1, 0x43, 0x04, 0x02, 0x20, 0x00, 0x00,
    0x41, 0x0D, 0xBB, 0x86, 0xF4, 0x31, 0x7F, 0xFF, 0x88, 0xFF, 0x37, 0x47,
    0x1A, 0xDB, 0x6A, 0xDF, 0xFF, 0xAC, 0x00, 0x00,
];

fn make_reference(w: u32, h: u32) -> Jbig2Image {
    let mut img = Jbig2Image::new(w, h);
    // Put some pattern in reference
    for x in 0..w.min(8) {
        img.set_pixel(x, 0, (x & 1) as u8);
    }
    img
}

fn make_params_t0(reference: Jbig2Image) -> RefinementRegionParams {
    RefinementRegionParams {
        gr_template: 0,
        reference,
        reference_dx: 0,
        reference_dy: 0,
        tpgron: false,
        grat: [-1, -1, -1, -1],
    }
}

fn make_params_t1(reference: Jbig2Image) -> RefinementRegionParams {
    RefinementRegionParams {
        gr_template: 1,
        reference,
        reference_dx: 0,
        reference_dy: 0,
        tpgron: false,
        grat: [0; 4],
    }
}

// --- Step 7.1: Template 0 & 1 ---

#[test]
fn template0_refine() {
    let reference = make_reference(8, 4);
    let params = make_params_t0(reference);
    let mut as_ = ArithState::new(TEST_STREAM).unwrap();
    let mut image = Jbig2Image::new(8, 4);
    let mut gr_stats = vec![0u8; refinement_stats_size(0)];

    let result = decode_refinement_region(&params, &mut as_, &mut image, &mut gr_stats);
    assert!(result.is_ok());
}

#[test]
fn template1_refine() {
    let reference = make_reference(8, 4);
    let params = make_params_t1(reference);
    let mut as_ = ArithState::new(TEST_STREAM).unwrap();
    let mut image = Jbig2Image::new(8, 4);
    let mut gr_stats = vec![0u8; refinement_stats_size(1)];

    let result = decode_refinement_region(&params, &mut as_, &mut image, &mut gr_stats);
    assert!(result.is_ok());
}

#[test]
fn template0_deterministic() {
    let ref1 = make_reference(8, 4);
    let ref2 = make_reference(8, 4);
    let params1 = make_params_t0(ref1);
    let params2 = make_params_t0(ref2);

    let mut as1 = ArithState::new(TEST_STREAM).unwrap();
    let mut img1 = Jbig2Image::new(8, 4);
    let mut stats1 = vec![0u8; refinement_stats_size(0)];
    decode_refinement_region(&params1, &mut as1, &mut img1, &mut stats1).unwrap();

    let mut as2 = ArithState::new(TEST_STREAM).unwrap();
    let mut img2 = Jbig2Image::new(8, 4);
    let mut stats2 = vec![0u8; refinement_stats_size(0)];
    decode_refinement_region(&params2, &mut as2, &mut img2, &mut stats2).unwrap();

    assert_eq!(img1.data, img2.data);
}

#[test]
fn with_offset() {
    // Reference offset should shift the reference pixels
    let reference = make_reference(8, 4);
    let mut params = make_params_t1(reference);
    params.reference_dx = 1;
    params.reference_dy = 1;

    let mut as_ = ArithState::new(TEST_STREAM).unwrap();
    let mut image = Jbig2Image::new(8, 4);
    let mut gr_stats = vec![0u8; refinement_stats_size(1)];

    decode_refinement_region(&params, &mut as_, &mut image, &mut gr_stats).unwrap();
}

#[test]
fn stats_size_values() {
    assert_eq!(refinement_stats_size(0), 8192);
    assert_eq!(refinement_stats_size(1), 1024);
}

// --- Step 7.2: TPGRON ---

#[test]
fn tpgron_prediction() {
    let reference = make_reference(8, 4);
    let mut params = make_params_t1(reference);
    params.tpgron = true;

    let mut as_ = ArithState::new(TEST_STREAM).unwrap();
    let mut image = Jbig2Image::new(8, 4);
    let mut gr_stats = vec![0u8; refinement_stats_size(1)];

    let result = decode_refinement_region(&params, &mut as_, &mut image, &mut gr_stats);
    assert!(result.is_ok());
}

#[test]
fn tpgron_template0() {
    let reference = make_reference(8, 4);
    let mut params = make_params_t0(reference);
    params.tpgron = true;

    let mut as_ = ArithState::new(TEST_STREAM).unwrap();
    let mut image = Jbig2Image::new(8, 4);
    let mut gr_stats = vec![0u8; refinement_stats_size(0)];

    decode_refinement_region(&params, &mut as_, &mut image, &mut gr_stats).unwrap();
}

#[test]
fn tpgron_implicit_value_uniform_ref() {
    // All-white reference → implicit values should all be 0
    let reference = Jbig2Image::new(8, 4); // all zeros
    let mut params = make_params_t1(reference);
    params.tpgron = true;

    let mut as_ = ArithState::new(TEST_STREAM).unwrap();
    let mut image = Jbig2Image::new(8, 4);
    let mut gr_stats = vec![0u8; refinement_stats_size(1)];

    decode_refinement_region(&params, &mut as_, &mut image, &mut gr_stats).unwrap();
}
