use crate::arith::{ArithCx, ArithState};

/// The test stream from jbig2dec/jbig2_arith.c (TEST section).
/// This is a known-good arithmetic coded stream.
const TEST_STREAM: &[u8] = &[
    0x84, 0xC7, 0x3B, 0xFC, 0xE1, 0xA1, 0x43, 0x04, 0x02, 0x20, 0x00, 0x00,
    0x41, 0x0D, 0xBB, 0x86, 0xF4, 0x31, 0x7F, 0xFF, 0x88, 0xFF, 0x37, 0x47,
    0x1A, 0xDB, 0x6A, 0xDF, 0xFF, 0xAC,
    0x00, 0x00,
];

#[test]
fn decode_known_sequence() {
    // The jbig2dec test decodes 256 bits from a single context starting at 0.
    // We verify the decoder doesn't panic and produces consistent output.
    let mut state = ArithState::new(TEST_STREAM).unwrap();
    let mut cx: ArithCx = 0;

    let mut bits = Vec::new();
    for _ in 0..256 {
        let d = state.decode(&mut cx).unwrap();
        assert!(d <= 1);
        bits.push(d);
    }

    // Verify deterministic: decode again should produce same sequence
    let mut state2 = ArithState::new(TEST_STREAM).unwrap();
    let mut cx2: ArithCx = 0;
    for (i, &expected) in bits.iter().enumerate() {
        let d = state2.decode(&mut cx2).unwrap();
        assert_eq!(d, expected, "mismatch at bit {i}");
    }
}

#[test]
fn context_adaptation() {
    // Context should change after decoding bits (MPS/LPS transitions)
    let mut state = ArithState::new(TEST_STREAM).unwrap();
    let mut cx: ArithCx = 0;

    let initial = cx;
    // Decode several bits — context must adapt
    for _ in 0..20 {
        state.decode(&mut cx).unwrap();
    }
    // After 20 bits, the context index should have moved from 0
    // (Either the index changed or the MPS bit flipped)
    assert_ne!(cx, initial, "context should adapt after decoding");
}

#[test]
fn ff_marker_handling() {
    // The test stream contains 0xFF bytes (at offset 19: 0x7F, 0xFF, 0x88).
    // 0xFF followed by 0x88 (> 0x8F) is a terminating marker.
    // The decoder should handle this gracefully.
    let mut state = ArithState::new(TEST_STREAM).unwrap();
    let mut cx: ArithCx = 0;

    // Decode enough bits to pass the FF marker region
    for _ in 0..256 {
        let result = state.decode(&mut cx);
        assert!(result.is_ok());
    }
}

#[test]
fn multiple_contexts() {
    // Use two independent contexts — they should adapt independently
    let mut state = ArithState::new(TEST_STREAM).unwrap();
    let mut cx_a: ArithCx = 0;
    let mut cx_b: ArithCx = 0;

    // Alternate between contexts
    for _ in 0..50 {
        state.decode(&mut cx_a).unwrap();
        state.decode(&mut cx_b).unwrap();
    }

    // Both should have adapted, and they can be equal or different
    // (same decoder, same stream, alternating)
    // Main check: no panic, no error
}

#[test]
fn short_stream() {
    // Minimal 2-byte stream
    let data = [0x00, 0x00];
    let state = ArithState::new(&data);
    // Should succeed (at least init works with 2 bytes)
    assert!(state.is_ok());
}

#[test]
fn single_byte_error() {
    // Empty stream should fail
    let data: &[u8] = &[];
    let result = ArithState::new(data);
    assert!(result.is_err());
}
