use crate::segment::{SegmentHeader, SegmentType};

/// Build a minimal segment header bytes:
/// 4 bytes number + 1 byte flags + 1 byte rtscarf + ref_segs + 1 byte page + 4 bytes data_length
fn make_segment_header(number: u32, seg_type: u8, referred: &[u8], page: u8, data_len: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&number.to_be_bytes());
    buf.push(seg_type); // flags (type in lower 6 bits)
    // rtscarf: count in upper 3 bits (short form, max 4)
    let count = referred.len() as u8;
    buf.push(count << 5);
    // referred-to segment numbers (1 byte each since number <= 256)
    buf.extend_from_slice(referred);
    buf.push(page);
    buf.extend_from_slice(&data_len.to_be_bytes());
    buf
}

#[test]
fn parse_segment_header() {
    let data = make_segment_header(1, 48, &[], 1, 19); // PageInformation
    let result = SegmentHeader::parse(&data).unwrap();
    let (hdr, consumed) = result.unwrap();
    assert_eq!(hdr.number, 1);
    assert_eq!(hdr.seg_type, Some(SegmentType::PageInformation));
    assert_eq!(hdr.referred_to_segments.len(), 0);
    assert_eq!(hdr.page_association, 1);
    assert_eq!(hdr.data_length, 19);
    assert_eq!(consumed, data.len());
}

#[test]
fn parse_with_referred_segments() {
    // Segment 5 refers to segments 1 and 3
    let data = make_segment_header(5, 6, &[1, 3], 1, 100); // ImmediateTextRegion
    let result = SegmentHeader::parse(&data).unwrap();
    let (hdr, _) = result.unwrap();
    assert_eq!(hdr.number, 5);
    assert_eq!(hdr.seg_type, Some(SegmentType::ImmediateTextRegion));
    assert_eq!(hdr.referred_to_segments, vec![1, 3]);
    assert_eq!(hdr.page_association, 1);
    assert_eq!(hdr.data_length, 100);
}

#[test]
fn dispatch_known_types() {
    // Verify all known segment types parse correctly
    let known_types: &[(u8, SegmentType)] = &[
        (0, SegmentType::SymbolDictionary),
        (4, SegmentType::IntermediateTextRegion),
        (6, SegmentType::ImmediateTextRegion),
        (7, SegmentType::ImmediateLosslessTextRegion),
        (16, SegmentType::PatternDictionary),
        (38, SegmentType::ImmediateGenericRegion),
        (39, SegmentType::ImmediateLosslessGenericRegion),
        (42, SegmentType::ImmediateGenericRefinementRegion),
        (48, SegmentType::PageInformation),
        (49, SegmentType::EndOfPage),
        (50, SegmentType::EndOfStripe),
        (51, SegmentType::EndOfFile),
        (52, SegmentType::Profile),
        (53, SegmentType::CodeTable),
        (62, SegmentType::Extension),
    ];

    for &(type_val, expected_type) in known_types {
        let data = make_segment_header(1, type_val, &[], 1, 0);
        let (hdr, _) = SegmentHeader::parse(&data).unwrap().unwrap();
        assert_eq!(hdr.seg_type, Some(expected_type), "type {type_val} mismatch");
    }
}

#[test]
fn skip_unknown_type() {
    // Type 10 is not a valid segment type
    let data = make_segment_header(1, 10, &[], 1, 0);
    let (hdr, _) = SegmentHeader::parse(&data).unwrap().unwrap();
    assert_eq!(hdr.seg_type, None);
}

#[test]
fn parse_needs_more_data() {
    // Too short — should return None (not error)
    let data = [0u8; 5];
    let result = SegmentHeader::parse(&data).unwrap();
    assert!(result.is_none());
}

#[test]
fn parse_region_segment_info() {
    use crate::segment::RegionSegmentInfo;

    let mut data = vec![0u8; 17];
    // width=64, height=32
    data[0..4].copy_from_slice(&64u32.to_be_bytes());
    data[4..8].copy_from_slice(&32u32.to_be_bytes());
    // x=10, y=20
    data[8..12].copy_from_slice(&10u32.to_be_bytes());
    data[12..16].copy_from_slice(&20u32.to_be_bytes());
    // flags: op=OR (0)
    data[16] = 0;

    let info = RegionSegmentInfo::parse(&data).unwrap();
    assert_eq!(info.width, 64);
    assert_eq!(info.height, 32);
    assert_eq!(info.x, 10);
    assert_eq!(info.y, 20);
    assert_eq!(info.op, 0);
}
