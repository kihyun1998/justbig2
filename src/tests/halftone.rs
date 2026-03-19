use crate::halftone::{
    decode_halftone_region, HalftoneRegionParams, PatternDict, PatternDictParams,
};
use crate::image::{ComposeOp, Jbig2Image};

// --- Step 10.1: 패턴 사전 ---

#[test]
fn parse_pattern_dict() {
    // flags=0 (HDMMR=0, HDTEMPLATE=0), HDPW=4, HDPH=4, GRAYMAX=3
    let data = [0x00, 4, 4, 0x00, 0x00, 0x00, 0x03];
    let (params, consumed) = PatternDictParams::parse(&data).unwrap();
    assert!(!params.hdmmr);
    assert_eq!(params.hdtemplate, 0);
    assert_eq!(params.hdpw, 4);
    assert_eq!(params.hdph, 4);
    assert_eq!(params.graymax, 3);
    assert_eq!(consumed, 7);
}

#[test]
fn parse_pattern_dict_mmr() {
    let data = [0x01, 8, 8, 0x00, 0x00, 0x00, 0x0F]; // HDMMR=1, 16 patterns
    let (params, _) = PatternDictParams::parse(&data).unwrap();
    assert!(params.hdmmr);
    assert_eq!(params.graymax, 15);
}

#[test]
fn decode_patterns() {
    // Create a collective bitmap: 4 patterns of 4x4, laid out horizontally = 16x4
    let mut collective = Jbig2Image::new(16, 4);
    // Pattern 0: all white (default)
    // Pattern 1: pixel (0,0) = 1
    collective.set_pixel(4, 0, 1);
    // Pattern 2: pixel (1,1) = 1
    collective.set_pixel(9, 1, 1);
    // Pattern 3: all black in first row
    for x in 12..16 {
        collective.set_pixel(x, 0, 1);
    }

    let pdict = PatternDict::from_collective(&collective, 4, 4, 4);
    assert_eq!(pdict.patterns.len(), 4);
    assert_eq!(pdict.hpw, 4);
    assert_eq!(pdict.hph, 4);

    // Verify patterns
    assert_eq!(pdict.patterns[0].get_pixel(0, 0), 0); // white
    assert_eq!(pdict.patterns[1].get_pixel(0, 0), 1); // black pixel
    assert_eq!(pdict.patterns[2].get_pixel(1, 1), 1);
    assert_eq!(pdict.patterns[3].get_pixel(0, 0), 1);
    assert_eq!(pdict.patterns[3].get_pixel(3, 0), 1);
}

// --- Step 10.2: 하프톤 영역 디코딩 ---

#[test]
fn parse_halftone_region() {
    // flags=0x00, HGW=2, HGH=2, HGX=0, HGY=0, HRX=0x0100(=1.0), HRY=0x0100
    let mut data = vec![0x00]; // flags
    data.extend_from_slice(&2u32.to_be_bytes()); // HGW
    data.extend_from_slice(&2u32.to_be_bytes()); // HGH
    data.extend_from_slice(&0i32.to_be_bytes()); // HGX
    data.extend_from_slice(&0i32.to_be_bytes()); // HGY
    data.extend_from_slice(&0x0100u16.to_be_bytes()); // HRX (1.0 in 8.8)
    data.extend_from_slice(&0x0100u16.to_be_bytes()); // HRY

    let (params, consumed) = HalftoneRegionParams::parse(&data).unwrap();
    assert!(!params.hmmr);
    assert_eq!(params.hgw, 2);
    assert_eq!(params.hgh, 2);
    assert_eq!(params.hgx, 0);
    assert_eq!(params.hgy, 0);
    assert_eq!(params.hrx, 0x0100);
    assert_eq!(params.hry, 0x0100);
    assert_eq!(consumed, 21);
}

#[test]
fn decode_halftone_simple() {
    // 2x2 grid of 4x4 patterns → 8x8 output
    let mut collective = Jbig2Image::new(8, 4);
    // Pattern 0: all white, Pattern 1: top-left pixel black
    collective.set_pixel(4, 0, 1);

    let pdict = PatternDict::from_collective(&collective, 4, 4, 2);

    // HRX = horizontal step between columns, HRY = vertical step between rows
    // For a simple grid: HRX = pattern_width (in 8.8), HRY = pattern_height (in 8.8)
    // Coordinate formula: x = (HGX + mg*HRY + ng*HRX) >> 8
    //                     y = (HGY + mg*HRX - ng*HRY) >> 8
    // For orthogonal grid with no rotation: HRY=0 doesn't work because mg*HRX controls y.
    // Actually HRX controls both x-step for ng and y-step for mg.
    // For a 4x4 non-rotated grid: HRX=4<<8, HRY=0
    //   cell(0,0): x=(0+0+0)>>8=0, y=(0+0-0)>>8=0
    //   cell(1,0): x=(0+0+1024)>>8=4, y=(0+0-0)>>8=0
    //   cell(0,1): x=(0+0+0)>>8=0, y=(0+1024-0)>>8=4
    let params = HalftoneRegionParams {
        hmmr: false,
        htemplate: 0,
        henableskip: false,
        hcombop: ComposeOp::Or,
        hdefpixel: false,
        hgw: 2,
        hgh: 2,
        hgx: 0,
        hgy: 0,
        hrx: 4 << 8,
        hry: 0,
    };

    let mut image = Jbig2Image::new(8, 8);
    // Gray values: [[0,1],[1,0]] — checkerboard pattern selection
    let gray_vals = vec![vec![0u32, 1], vec![1, 0]];

    decode_halftone_region(&params, &mut image, &pdict, &gray_vals).unwrap();

    // Cell (0,0) = pattern 0 (white) at (0,0)
    assert_eq!(image.get_pixel(0, 0), 0);
    // Cell (1,0) = pattern 1 (has black at 0,0) placed at (4,0)
    assert_eq!(image.get_pixel(4, 0), 1);
}

#[test]
fn halftone_with_skip() {
    let mut collective = Jbig2Image::new(4, 4);
    collective.set_pixel(0, 0, 1);
    let pdict = PatternDict::from_collective(&collective, 4, 4, 1);

    let params = HalftoneRegionParams {
        hmmr: false,
        htemplate: 0,
        henableskip: true,
        hcombop: ComposeOp::Or,
        hdefpixel: false,
        hgw: 3,
        hgh: 1,
        hgx: 0,
        hgy: 0,
        hrx: 4 << 8,
        hry: 0,
    };

    // Image only 8 wide, third cell at x=8 should be skipped
    let mut image = Jbig2Image::new(8, 4);
    let gray_vals = vec![vec![0u32], vec![0], vec![0]];

    decode_halftone_region(&params, &mut image, &pdict, &gray_vals).unwrap();
}

#[test]
fn halftone_default_pixel() {
    let pdict = PatternDict {
        patterns: vec![Jbig2Image::new(4, 4)],
        hpw: 4,
        hph: 4,
    };

    let params = HalftoneRegionParams {
        hmmr: false,
        htemplate: 0,
        henableskip: false,
        hcombop: ComposeOp::Or,
        hdefpixel: true, // fill black
        hgw: 0,
        hgh: 0,
        hgx: 0,
        hgy: 0,
        hrx: 0,
        hry: 0,
    };

    let mut image = Jbig2Image::new(8, 8);
    decode_halftone_region(&params, &mut image, &pdict, &[]).unwrap();

    // Should be filled with black (default pixel = 1)
    assert_eq!(image.get_pixel(0, 0), 1);
    assert_eq!(image.get_pixel(7, 7), 1);
}
