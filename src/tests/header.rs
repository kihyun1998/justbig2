use crate::header::{FileHeader, Organization, MAGIC};

#[test]
fn parse_valid_header() {
    // Sequential, 1 page known
    let mut data = MAGIC.to_vec();
    data.push(0x01); // flags: sequential, pages known
    data.extend_from_slice(&1u32.to_be_bytes()); // 1 page
    let (hdr, consumed) = FileHeader::parse(&data).unwrap().unwrap();
    assert_eq!(hdr.organization, Organization::Sequential);
    assert_eq!(hdr.n_pages, Some(1));
    assert_eq!(consumed, 13);
}

#[test]
fn parse_random_access_unknown_pages() {
    let mut data = MAGIC.to_vec();
    data.push(0x02); // flags: random-access, pages unknown
    let (hdr, consumed) = FileHeader::parse(&data).unwrap().unwrap();
    assert_eq!(hdr.organization, Organization::RandomAccess);
    assert_eq!(hdr.n_pages, None);
    assert_eq!(consumed, 9);
}

#[test]
fn reject_bad_magic() {
    let data = [0x00; 13];
    let result = FileHeader::parse(&data);
    assert!(result.is_err());
}

#[test]
fn parse_embedded_no_header() {
    // Embedded mode doesn't have a file header — decoder starts at SequentialHeader.
    // Verify that attempting to parse non-JBIG2 data fails.
    let data = [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
    let result = FileHeader::parse(&data);
    assert!(result.is_err());
}

#[test]
fn too_short_returns_none() {
    let data = MAGIC.to_vec(); // only 8 bytes, need at least 9
    let result = FileHeader::parse(&data).unwrap();
    assert!(result.is_none());
}

#[test]
fn reject_amendment2() {
    let mut data = MAGIC.to_vec();
    data.push(0x04); // 12 adaptive template pixels
    let result = FileHeader::parse(&data);
    assert!(result.is_err());
}
