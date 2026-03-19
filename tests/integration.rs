use justbig2::decoder::Decoder;
use justbig2::header::MAGIC;

// --- Step 11.1: annex-h.jbig2 디코딩 ---

#[test]
fn decode_annex_h() {
    let data = include_bytes!("../vendor/jbig2dec/annex-h.jbig2");
    let mut dec = Decoder::new();
    dec.write(data).unwrap();

    let page = dec.page_out();
    assert!(page.is_some(), "should produce a decoded page");

    let img = page.unwrap();
    assert!(img.width > 0);
    assert!(img.height > 0);

    // The annex-h file is a spec example — verify dimensions match expected
    // (64x56 based on the page info segment in the file)
    assert_eq!(img.width, 64);
    assert_eq!(img.height, 56);

    // Verify not all-white (the image has content)
    let has_black = img.data.iter().any(|&b| b != 0);
    assert!(has_black, "decoded image should have some black pixels");
}

#[test]
fn decode_annex_h_deterministic() {
    let data = include_bytes!("../vendor/jbig2dec/annex-h.jbig2");

    let mut dec1 = Decoder::new();
    dec1.write(data).unwrap();
    let img1 = dec1.page_out().unwrap();

    let mut dec2 = Decoder::new();
    dec2.write(data).unwrap();
    let img2 = dec2.page_out().unwrap();

    assert_eq!(img1.data, img2.data, "decoding should be deterministic");
}

// --- Step 11.2: Embedded 모드 ---

#[test]
fn decode_embedded_stream() {
    // Build an embedded stream: page info + immediate generic region + end of page
    let mut buf = Vec::new();

    // Segment 0: Page Information (type 48), data_length=19
    buf.extend_from_slice(&0u32.to_be_bytes());
    buf.push(48);
    buf.push(0x00);
    buf.push(1);
    buf.extend_from_slice(&19u32.to_be_bytes());

    // Page info: 8x8, no stripe
    buf.extend_from_slice(&8u32.to_be_bytes());  // width
    buf.extend_from_slice(&8u32.to_be_bytes());  // height
    buf.extend_from_slice(&0u32.to_be_bytes());  // x_res
    buf.extend_from_slice(&0u32.to_be_bytes());  // y_res
    buf.push(0);                                  // flags
    buf.extend_from_slice(&0u16.to_be_bytes());  // striping

    // Segment 1: End of Page (type 49)
    buf.extend_from_slice(&1u32.to_be_bytes());
    buf.push(49);
    buf.push(0x00);
    buf.push(1);
    buf.extend_from_slice(&0u32.to_be_bytes());

    // Segment 2: End of File (type 51)
    buf.extend_from_slice(&2u32.to_be_bytes());
    buf.push(51);
    buf.push(0x00);
    buf.push(0);
    buf.extend_from_slice(&0u32.to_be_bytes());

    let mut dec = Decoder::new_embedded();
    dec.write(&buf).unwrap();

    let page = dec.page_out();
    assert!(page.is_some());
    assert_eq!(page.unwrap().width, 8);
}

// --- Step 11.3: 에러 복원 & 엣지 케이스 ---

#[test]
fn truncated_stream() {
    // Feed only the file header — not enough for a full page
    let data = include_bytes!("../vendor/jbig2dec/annex-h.jbig2");
    let header_only = &data[..13]; // Just the file header
    let mut dec = Decoder::new();
    // Should not panic
    let _ = dec.write(header_only);
    // No completed page with just a header
    assert!(dec.page_out().is_none());
}

#[test]
fn empty_page() {
    // Page info + end of page, no content segments → blank page
    let mut buf = Vec::new();
    buf.extend_from_slice(&MAGIC);
    buf.push(0x01); // sequential, pages known
    buf.extend_from_slice(&1u32.to_be_bytes());

    // Page info: 16x16
    buf.extend_from_slice(&0u32.to_be_bytes());
    buf.push(48);
    buf.push(0x00);
    buf.push(1);
    buf.extend_from_slice(&19u32.to_be_bytes());
    buf.extend_from_slice(&16u32.to_be_bytes());
    buf.extend_from_slice(&16u32.to_be_bytes());
    buf.extend_from_slice(&0u32.to_be_bytes());
    buf.extend_from_slice(&0u32.to_be_bytes());
    buf.push(0);
    buf.extend_from_slice(&0u16.to_be_bytes());

    // End of page
    buf.extend_from_slice(&1u32.to_be_bytes());
    buf.push(49);
    buf.push(0x00);
    buf.push(1);
    buf.extend_from_slice(&0u32.to_be_bytes());

    // End of file
    buf.extend_from_slice(&2u32.to_be_bytes());
    buf.push(51);
    buf.push(0x00);
    buf.push(0);
    buf.extend_from_slice(&0u32.to_be_bytes());

    let mut dec = Decoder::new();
    dec.write(&buf).unwrap();

    let page = dec.page_out().unwrap();
    assert_eq!(page.width, 16);
    assert_eq!(page.height, 16);
    // All white (no content)
    assert!(page.data.iter().all(|&b| b == 0));
}

#[test]
fn missing_reference() {
    // Bad magic → error
    let data = [0x00; 20];
    let mut dec = Decoder::new();
    let result = dec.write(&data);
    assert!(result.is_err());
}

#[test]
fn incremental_annex_h() {
    // Feed annex-h byte by byte
    let data = include_bytes!("../vendor/jbig2dec/annex-h.jbig2");
    let mut dec = Decoder::new();
    for &b in data.iter() {
        dec.write(&[b]).unwrap();
    }
    let page = dec.page_out();
    assert!(page.is_some());
    assert_eq!(page.unwrap().width, 64);
}
