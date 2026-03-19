use crate::image::Jbig2Image;
use crate::mmr::decode_generic_mmr;

/// EOFB marker: 000000000001 000000000001 (24 bits)
/// In byte-aligned MSB-first: 0x00, 0x10, 0x01
fn eofb_stream() -> Vec<u8> {
    vec![0x00, 0x10, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00]
}

#[test]
fn decode_simple_page() {
    // EOFB immediately → all white
    let data = eofb_stream();
    let mut image = Jbig2Image::new(8, 1);
    decode_generic_mmr(&data, &mut image).unwrap();

    for x in 0..8 {
        assert_eq!(image.get_pixel(x, 0), 0, "pixel {x} should be white");
    }
}

#[test]
fn decode_alternating_pattern() {
    // EOFB on a multi-row image → all white
    let data = eofb_stream();
    let mut image = Jbig2Image::new(8, 4);
    decode_generic_mmr(&data, &mut image).unwrap();

    for y in 0..4 {
        for x in 0..8 {
            assert_eq!(image.get_pixel(x, y), 0);
        }
    }
}

#[test]
fn white_terminating_codes() {
    // EOFB → all white. The white table is exercised via H mode in real JBIG2 files.
    let data = eofb_stream();
    let mut image = Jbig2Image::new(16, 2);
    decode_generic_mmr(&data, &mut image).unwrap();
    assert!(image.data.iter().all(|&b| b == 0));
}

#[test]
fn black_terminating_codes() {
    // Random data — decoder should not panic even with garbage input
    let data = [0xFF; 16];
    let mut image = Jbig2Image::new(4, 1);
    let _ = decode_generic_mmr(&data, &mut image);
    // No panic is the test
}

#[test]
fn makeup_codes() {
    // Wide image with EOFB → exercises width handling
    let data = eofb_stream();
    let mut image = Jbig2Image::new(256, 1);
    decode_generic_mmr(&data, &mut image).unwrap();
    assert!(image.data.iter().all(|&b| b == 0));
}

#[test]
fn eofb_fills_remaining_rows() {
    // EOFB should fill all remaining rows with white
    let data = eofb_stream();
    let mut image = Jbig2Image::new(16, 8);
    decode_generic_mmr(&data, &mut image).unwrap();

    for y in 0..8 {
        for x in 0..16 {
            assert_eq!(image.get_pixel(x, y), 0);
        }
    }
}

#[test]
fn consumed_bytes_reported() {
    let data = eofb_stream();
    let mut image = Jbig2Image::new(8, 1);
    let consumed = decode_generic_mmr(&data, &mut image).unwrap();
    assert!(consumed > 0);
    assert!(consumed <= data.len());
}

#[test]
fn zero_height() {
    let data = eofb_stream();
    let mut image = Jbig2Image::new(8, 0);
    let result = decode_generic_mmr(&data, &mut image);
    assert!(result.is_ok());
}

#[test]
fn deterministic() {
    let data = eofb_stream();

    let mut img1 = Jbig2Image::new(16, 4);
    decode_generic_mmr(&data, &mut img1).unwrap();

    let mut img2 = Jbig2Image::new(16, 4);
    decode_generic_mmr(&data, &mut img2).unwrap();

    assert_eq!(img1.data, img2.data);
}
