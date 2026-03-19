use crate::decoder::{Decoder, DecoderState};
use crate::header::MAGIC;

/// Build a minimal JBIG2 file: file header + page info + end of page + end of file.
fn make_minimal_jbig2(width: u32, height: u32) -> Vec<u8> {
    let mut buf = Vec::new();

    // File header: sequential, 1 page known
    buf.extend_from_slice(&MAGIC);
    buf.push(0x01); // flags: sequential, pages known
    buf.extend_from_slice(&1u32.to_be_bytes());

    // Segment 0: Page Information (type 48)
    // Header: number=0, flags=48, rtscarf=0 refs, page=1, data_length=19
    buf.extend_from_slice(&0u32.to_be_bytes()); // segment number
    buf.push(48); // flags (type = PageInformation)
    buf.push(0x00); // rtscarf: 0 referred-to segments
    buf.push(1); // page association
    buf.extend_from_slice(&19u32.to_be_bytes()); // data length

    // Page info data (19 bytes)
    buf.extend_from_slice(&width.to_be_bytes());
    buf.extend_from_slice(&height.to_be_bytes());
    buf.extend_from_slice(&0u32.to_be_bytes()); // x_resolution
    buf.extend_from_slice(&0u32.to_be_bytes()); // y_resolution
    buf.push(0); // flags
    buf.extend_from_slice(&0u16.to_be_bytes()); // striping (none)

    // Segment 1: End of Page (type 49)
    buf.extend_from_slice(&1u32.to_be_bytes()); // segment number
    buf.push(49); // flags
    buf.push(0x00);
    buf.push(1); // page association
    buf.extend_from_slice(&0u32.to_be_bytes()); // data length = 0

    // Segment 2: End of File (type 51)
    buf.extend_from_slice(&2u32.to_be_bytes());
    buf.push(51);
    buf.push(0x00);
    buf.push(0); // page association = 0
    buf.extend_from_slice(&0u32.to_be_bytes());

    buf
}

#[test]
fn sequential_state_machine() {
    let data = make_minimal_jbig2(8, 8);
    let mut dec = Decoder::new();

    dec.write(&data).unwrap();

    assert_eq!(dec.state, DecoderState::Eof);

    // Should have a completed page
    let page = dec.page_out();
    assert!(page.is_some());
    let img = page.unwrap();
    assert_eq!(img.width, 8);
    assert_eq!(img.height, 8);
}

#[test]
fn embedded_mode() {
    let mut dec = Decoder::new_embedded();
    assert_eq!(dec.state, DecoderState::SequentialHeader);

    // Feed page info segment directly (no file header)
    let mut buf = Vec::new();

    // Segment 0: Page Information
    buf.extend_from_slice(&0u32.to_be_bytes());
    buf.push(48);
    buf.push(0x00);
    buf.push(1);
    buf.extend_from_slice(&19u32.to_be_bytes());

    // Page info data
    buf.extend_from_slice(&16u32.to_be_bytes()); // width
    buf.extend_from_slice(&4u32.to_be_bytes());  // height
    buf.extend_from_slice(&0u32.to_be_bytes());  // x_res
    buf.extend_from_slice(&0u32.to_be_bytes());  // y_res
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

    dec.write(&buf).unwrap();

    let page = dec.page_out();
    assert!(page.is_some());
    let img = page.unwrap();
    assert_eq!(img.width, 16);
    assert_eq!(img.height, 4);
}

#[test]
fn incremental_write() {
    // Feed data byte by byte
    let data = make_minimal_jbig2(4, 4);
    let mut dec = Decoder::new();

    for &b in &data {
        dec.write(&[b]).unwrap();
    }

    assert_eq!(dec.state, DecoderState::Eof);
    assert!(dec.page_out().is_some());
}

#[test]
fn no_page_before_complete() {
    // Feed only file header + page info, but no end of page
    let mut buf = Vec::new();
    buf.extend_from_slice(&MAGIC);
    buf.push(0x01);
    buf.extend_from_slice(&1u32.to_be_bytes());

    let mut dec = Decoder::new();
    dec.write(&buf).unwrap();

    // No completed page yet
    assert!(dec.page_out().is_none());
}
