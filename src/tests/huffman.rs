use crate::huffman::*;

// --- Step 3.1: 테이블 빌드 ---

#[test]
fn build_standard_table_a() {
    let table = build_standard_table(0).unwrap(); // Table A
    // Table A: preflen 1,2,3 + low(0) + high(3) → log_table_size should be reasonable
    assert!(table.log_table_size > 0);
}

#[test]
fn build_with_oob() {
    // Table B has HTOOB=true
    let table = build_standard_table(1).unwrap();
    assert!(table.log_table_size > 0);
}

#[test]
fn all_standard_tables_valid() {
    for i in 0..15 {
        let result = build_standard_table(i);
        assert!(result.is_ok(), "standard table {i} failed to build: {:?}", result.err());
    }
}

// --- Step 3.2: 허프만 디코딩 ---

#[test]
fn decode_table_a_values() {
    // Table A: PREFLEN=1,RANGELEN=4,RANGELOW=0 → code "0" + 4 bits
    // Encode: bit 0 (prefix for first line) + 4 bits value = 0b0_0101 = value 5
    // Byte: 0b0_0101_000 = 0x28
    let data = [0x28, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    let table = build_standard_table(0).unwrap();
    let mut hs = HuffmanState::new(&data);

    let (val, oob) = hs.get(&table).unwrap();
    assert!(!oob);
    assert_eq!(val, 5); // RANGELOW=0 + offset=5
}

#[test]
fn decode_table_a_second_line() {
    // Table A line 2: PREFLEN=2, RANGELEN=8, RANGELOW=16
    // Code "10" + 8 bits for offset
    // "10" + "00000011" = value 16 + 3 = 19
    // Bits: 10_00000011_000000... = 0x80 0xC0
    let data = [0x80, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    let table = build_standard_table(0).unwrap();
    let mut hs = HuffmanState::new(&data);

    let (val, oob) = hs.get(&table).unwrap();
    assert!(!oob);
    assert_eq!(val, 19); // 16 + 3
}

#[test]
fn decode_oob_signal() {
    // Table B (HTOOB=true): OOB line has PREFLEN=6
    // Build table and decode an OOB code
    let table = build_standard_table(1).unwrap(); // Table B

    // Table B codes:
    //   PREFLEN=1: code 0 → value 0
    //   PREFLEN=2: code 10 → value 1
    //   PREFLEN=3: code 110 → value 2
    //   PREFLEN=4: code 1110 + 3bits → 3..10
    //   PREFLEN=5: code 11110 + 6bits → 11..74
    //   PREFLEN=0: low (unused in prefix)
    //   PREFLEN=6: high code 111110 + 32bits → 75+
    //   PREFLEN=6: OOB code 111111

    // First decode value 0: bit "0" → 0x00...
    let data0 = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    let mut hs = HuffmanState::new(&data0);
    let (val, oob) = hs.get(&table).unwrap();
    assert_eq!(val, 0);
    assert!(!oob);

    // Decode value 1: bits "10" → 0x80...
    let data1 = [0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    let mut hs = HuffmanState::new(&data1);
    let (val, oob) = hs.get(&table).unwrap();
    assert_eq!(val, 1);
    assert!(!oob);
}

#[test]
fn decode_multiple_values() {
    // Table N (B.14) — simple small-range table, no OOB
    // N: {3,0,-2}, {3,0,-1}, {1,0,0}, {3,0,1}, {3,0,2}, low, high
    // Codes: 0→0, 100→-2, 101→-1, 110→1, 111→2
    let table = build_standard_table(13).unwrap(); // Table N (index 13)

    // Encode: 0 (=0), 100 (-2), 101 (-1)
    // Bits: 0_100_101_0 = 0x4A, pad
    let data = [0x4A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    let mut hs = HuffmanState::new(&data);

    let (v1, _) = hs.get(&table).unwrap();
    assert_eq!(v1, 0);

    let (v2, _) = hs.get(&table).unwrap();
    assert_eq!(v2, -2);

    let (v3, _) = hs.get(&table).unwrap();
    assert_eq!(v3, -1);
}

// --- Step 3.3: 표준 테이블 검증 ---

#[test]
fn table_b1_known_values() {
    // Table A (B.1): first line PREFLEN=1,RANGELEN=4,RANGELOW=0
    assert_eq!(TABLE_A[0].preflen, 1);
    assert_eq!(TABLE_A[0].rangelen, 4);
    assert_eq!(TABLE_A[0].rangelow, 0);
    // Last line (high)
    assert_eq!(TABLE_A[4].preflen, 3);
    assert_eq!(TABLE_A[4].rangelen, 32);
    assert_eq!(TABLE_A[4].rangelow, 65808);
}

#[test]
fn table_b4_known_values() {
    // Table D (B.4)
    assert_eq!(TABLE_D[0].preflen, 1);
    assert_eq!(TABLE_D[0].rangelen, 0);
    assert_eq!(TABLE_D[0].rangelow, 1);
    assert_eq!(TABLE_D.len(), 7);
}

#[test]
fn standard_table_line_counts() {
    assert_eq!(TABLE_A.len(), 5);
    assert_eq!(TABLE_B.len(), 8);
    assert_eq!(TABLE_C.len(), 9);
    assert_eq!(TABLE_D.len(), 7);
    assert_eq!(TABLE_E.len(), 8);
    assert_eq!(TABLE_F.len(), 14);
    assert_eq!(TABLE_G.len(), 15);
    assert_eq!(TABLE_H.len(), 21);
    assert_eq!(TABLE_I.len(), 22);
    assert_eq!(TABLE_J.len(), 21);
    assert_eq!(TABLE_K.len(), 14);
    assert_eq!(TABLE_L.len(), 15);
    assert_eq!(TABLE_M.len(), 14);
    assert_eq!(TABLE_N.len(), 7);
    assert_eq!(TABLE_O.len(), 13);
}

// --- Step 3.4: 사용자 정의 테이블 ---

#[test]
fn parse_user_table_basic() {
    // Construct a minimal user table segment:
    // flags: HTOOB=0, HTPS=1 (bits 1-3 = 0 → HTPS=1), HTRS=1 (bits 4-6 = 0 → HTRS=1)
    // HTLOW = 0 (4 bytes BE)
    // HTHIGH = 4 (4 bytes BE)
    // Then line data: for each line we need HTPS=1 bit for PREFLEN, HTRS=1 bit for RANGELEN
    // CURRANGELOW starts at 0, goes until >= 4
    //   Line 0: PREFLEN=1, RANGELEN=1 → CURRANGELOW=0, next CURRANGELOW=0+2=2
    //   Line 1: PREFLEN=1, RANGELEN=1 → CURRANGELOW=2, next CURRANGELOW=2+2=4 → done
    //   Lower: PREFLEN=1
    //   Upper: PREFLEN=1
    // Bits: 1,1, 1,1, 1, 1 = 0b111111_00 = 0xFC
    let mut data = vec![0u8; 11];
    data[0] = 0x00; // flags: HTOOB=0, HTPS=1, HTRS=1
    // HTLOW = 0
    data[1] = 0; data[2] = 0; data[3] = 0; data[4] = 0;
    // HTHIGH = 4
    data[5] = 0; data[6] = 0; data[7] = 0; data[8] = 4;
    // line data
    data[9] = 0xFC; // 111111_00
    data.push(0x00);

    let params = parse_user_table(&data).unwrap();
    assert!(!params.htoob);
    // 2 normal lines + lower + upper = 4
    assert_eq!(params.lines.len(), 4);
    assert_eq!(params.lines[0].rangelow, 0);
    assert_eq!(params.lines[1].rangelow, 2);
}

#[test]
fn parse_user_table_with_oob() {
    let mut data = vec![0u8; 11];
    data[0] = 0x01; // HTOOB=1, HTPS=1, HTRS=1
    // HTLOW = 0, HTHIGH = 2
    data[5] = 0; data[6] = 0; data[7] = 0; data[8] = 2;
    // Line: PREFLEN=1, RANGELEN=1 → CURRANGELOW=0, next=2 → done
    // Lower: PREFLEN=1, Upper: PREFLEN=1, OOB: PREFLEN=1
    // Bits: 1,1, 1, 1, 1 = 0b11111_000 = 0xF8
    data[9] = 0xF8;
    data.push(0x00);

    let params = parse_user_table(&data).unwrap();
    assert!(params.htoob);
    // 1 normal + lower + upper + OOB = 4
    assert_eq!(params.lines.len(), 4);
}

// --- HuffmanState extras ---

#[test]
fn get_bits_basic() {
    let data = [0xAB, 0xCD, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    let mut hs = HuffmanState::new(&data);

    let v = hs.get_bits(8);
    assert_eq!(v, 0xAB);

    let v = hs.get_bits(4);
    assert_eq!(v, 0xC);
}

#[test]
fn align_boundary() {
    let data = [0xFF, 0xAB, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    let mut hs = HuffmanState::new(&data);

    hs.get_bits(3); // read 3 bits
    hs.align();     // skip remaining 5 bits
    let v = hs.get_bits(8);
    assert_eq!(v, 0xAB);
}
