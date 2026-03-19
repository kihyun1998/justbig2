use crate::{decode, decode_embedded, Decoder, Page};
use crate::header::MAGIC;

#[test]
fn basic_decode_flow() {
    let data = include_bytes!("../../vendor/jbig2dec/annex-h.jbig2");
    let pages = decode(data).unwrap();

    assert!(pages.len() >= 1, "should have at least 1 page, got {}", pages.len());
    let page = &pages[0];
    assert_eq!(page.width, 64);
    assert_eq!(page.height, 56);
    assert_eq!(page.stride, 8); // ceil(64/8) = 8
    assert!(page.data.len() > 0);

    // Verify get_pixel works
    let _ = page.get_pixel(0, 0);
    let _ = page.get_pixel(63, 55);
    // Out of bounds returns 0
    assert_eq!(page.get_pixel(64, 0), 0);
    assert_eq!(page.get_pixel(0, 56), 0);
}

#[test]
fn api_embedded_flow() {
    // Build minimal embedded stream
    let mut buf = Vec::new();

    // Page info
    buf.extend_from_slice(&0u32.to_be_bytes());
    buf.push(48);
    buf.push(0x00);
    buf.push(1);
    buf.extend_from_slice(&19u32.to_be_bytes());
    buf.extend_from_slice(&4u32.to_be_bytes());  // width
    buf.extend_from_slice(&4u32.to_be_bytes());  // height
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

    let pages = decode_embedded(&buf).unwrap();
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].width, 4);
    assert_eq!(pages[0].height, 4);
}

#[test]
fn decoder_method_page() {
    let data = include_bytes!("../../vendor/jbig2dec/annex-h.jbig2");
    let mut dec = Decoder::new();
    dec.write(data).unwrap();

    // Use the Page API (not page_out)
    let page: Option<Page> = dec.page();
    assert!(page.is_some());
    let p = page.unwrap();
    assert_eq!(p.width, 64);
    assert_eq!(p.height, 56);
}

#[test]
fn decode_returns_empty_on_no_pages() {
    // File header only, no segments → no pages
    let mut buf = MAGIC.to_vec();
    buf.push(0x03); // sequential, pages unknown
    let pages = decode(&buf).unwrap();
    assert!(pages.is_empty());
}
