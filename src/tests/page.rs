use crate::page::{Page, PageState};

/// Build minimal page info segment data (19 bytes).
fn make_page_info(width: u32, height: u32, striped: bool, stripe_size: u16) -> Vec<u8> {
    let mut data = vec![0u8; 19];
    data[0..4].copy_from_slice(&width.to_be_bytes());
    data[4..8].copy_from_slice(&height.to_be_bytes());
    // x_resolution, y_resolution = 0
    // flags = 0 (default white)
    data[16] = 0;
    // striping
    let striping: i16 = if striped {
        (stripe_size as i16) | -0x8000i16 // set bit 15
    } else {
        0
    };
    data[17..19].copy_from_slice(&striping.to_be_bytes());
    data
}

#[test]
fn create_page() {
    let mut page = Page::new();
    assert_eq!(page.state, PageState::Free);

    let data = make_page_info(100, 50, false, 0);
    page.parse_info(1, &data).unwrap();

    assert_eq!(page.state, PageState::New);
    assert_eq!(page.number, 1);
    assert_eq!(page.width, 100);
    assert_eq!(page.height, 50);
    assert!(!page.striped);
    assert!(page.image.is_some());
    let img = page.image.as_ref().unwrap();
    assert_eq!(img.width, 100);
    assert_eq!(img.height, 50);
}

#[test]
fn page_default_pixel() {
    // Default white (flags bit 2 = 0)
    let mut page = Page::new();
    let data = make_page_info(8, 1, false, 0);
    page.parse_info(1, &data).unwrap();
    let img = page.image.as_ref().unwrap();
    assert_eq!(img.get_pixel(0, 0), 0); // white

    // Default black (flags bit 2 = 1)
    let mut page2 = Page::new();
    let mut data2 = make_page_info(8, 1, false, 0);
    data2[16] = 0x04; // flags: default black
    page2.parse_info(1, &data2).unwrap();
    let img2 = page2.image.as_ref().unwrap();
    assert_eq!(img2.get_pixel(0, 0), 1); // black
}

#[test]
fn stripe_extend() {
    use crate::image::{ComposeOp, Jbig2Image};

    let mut page = Page::new();
    // Unknown height, striped
    let data = make_page_info(8, 0xFFFFFFFF, true, 10);
    page.parse_info(1, &data).unwrap();

    assert!(page.striped);
    assert_eq!(page.height, 0xFFFFFFFF);

    // Image should initially be stripe_size tall
    let img = page.image.as_ref().unwrap();
    assert_eq!(img.height, 10);

    // Add result beyond initial height → should grow
    let mut src = Jbig2Image::new(8, 5);
    src.set_pixel(0, 0, 1);
    page.add_result(&src, 0, 12, ComposeOp::Or).unwrap();

    let img = page.image.as_ref().unwrap();
    assert!(img.height >= 17); // 12 + 5 = 17
    assert_eq!(img.get_pixel(0, 12), 1);
}

#[test]
fn page_complete_state() {
    let mut page = Page::new();
    let data = make_page_info(8, 8, false, 0);
    page.parse_info(1, &data).unwrap();
    assert_eq!(page.state, PageState::New);

    page.complete();
    assert_eq!(page.state, PageState::Complete);
}

#[test]
fn page_end_row() {
    let mut page = Page::new();
    let data = make_page_info(8, 8, true, 8);
    page.parse_info(1, &data).unwrap();

    page.set_end_row(4);
    assert_eq!(page.end_row, 4);
    page.set_end_row(8);
    assert_eq!(page.end_row, 8);
}
