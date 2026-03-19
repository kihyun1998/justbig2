use crate::bitreader::BitReader;

#[test]
fn read_bits_basic() {
    // 0xA5 = 1010_0101
    let data = [0xA5];
    let mut r = BitReader::new(&data);

    assert_eq!(r.read_bit(), 1);
    assert_eq!(r.read_bit(), 0);
    assert_eq!(r.read_bit(), 1);
    assert_eq!(r.read_bit(), 0);
    assert_eq!(r.read_bit(), 0);
    assert_eq!(r.read_bit(), 1);
    assert_eq!(r.read_bit(), 0);
    assert_eq!(r.read_bit(), 1);

    // Past end → 0
    assert_eq!(r.read_bit(), 0);
}

#[test]
fn read_across_bytes() {
    // 0xFF 0x00 = 1111_1111 0000_0000
    let data = [0xFF, 0x00];
    let mut r = BitReader::new(&data);

    // Read 12 bits across byte boundary
    let val = r.read_bits(12);
    // 1111_1111_0000 = 0xFF0
    assert_eq!(val, 0xFF0);

    // Remaining 4 bits
    let val = r.read_bits(4);
    assert_eq!(val, 0x0);
}

#[test]
fn read_byte_aligned() {
    let data = [0xAB, 0xCD];
    let mut r = BitReader::new(&data);
    assert_eq!(r.read_byte(), 0xAB);
    assert_eq!(r.read_byte(), 0xCD);
}

#[test]
fn align_to_byte() {
    let data = [0xFF, 0xAB];
    let mut r = BitReader::new(&data);

    // Read 3 bits
    r.read_bits(3);
    // Align discards remaining 5 bits of first byte
    r.align();
    // Next read should get second byte
    assert_eq!(r.read_byte(), 0xAB);
}

#[test]
fn read_bits_multi_byte() {
    let data = [0x12, 0x34, 0x56, 0x78];
    let mut r = BitReader::new(&data);

    // Read 32 bits
    let val = r.read_bits(32);
    assert_eq!(val, 0x12345678);
}

#[test]
fn eof_detection() {
    let data = [0xFF];
    let mut r = BitReader::new(&data);
    assert!(!r.is_eof());
    r.read_byte();
    assert!(r.is_eof());
}
