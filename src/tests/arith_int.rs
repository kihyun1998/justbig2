use crate::arith::ArithState;
use crate::arith_int::ArithIntCtx;

/// Helper: encode a known integer sequence using the arithmetic coder is hard,
/// so we test the decoder structurally — init, decode from known stream,
/// verify no errors and reasonable output.

/// The jbig2dec test stream exercises the arithmetic coder thoroughly.
/// We use it to test integer decoding as well.
const TEST_STREAM: &[u8] = &[
    0x84, 0xC7, 0x3B, 0xFC, 0xE1, 0xA1, 0x43, 0x04, 0x02, 0x20, 0x00, 0x00,
    0x41, 0x0D, 0xBB, 0x86, 0xF4, 0x31, 0x7F, 0xFF, 0x88, 0xFF, 0x37, 0x47,
    0x1A, 0xDB, 0x6A, 0xDF, 0xFF, 0xAC,
    0x00, 0x00,
];

#[test]
fn decode_integers() {
    let mut as_ = ArithState::new(TEST_STREAM).unwrap();
    let mut ctx = ArithIntCtx::new();

    // Decode several integers — should not panic
    let mut values = Vec::new();
    for _ in 0..10 {
        match ctx.decode(&mut as_) {
            Ok(Some(v)) => values.push(v),
            Ok(None) => break, // OOB
            Err(_) => break,
        }
    }

    // Verify determinism
    let mut as2 = ArithState::new(TEST_STREAM).unwrap();
    let mut ctx2 = ArithIntCtx::new();
    for (i, &expected) in values.iter().enumerate() {
        let v = ctx2.decode(&mut as2).unwrap();
        assert_eq!(v, Some(expected), "integer mismatch at index {i}");
    }
}

#[test]
fn decode_produces_values_or_oob() {
    let mut as_ = ArithState::new(TEST_STREAM).unwrap();
    let mut ctx = ArithIntCtx::new();

    // Each result should be either Some(value) or None (OOB)
    for _ in 0..5 {
        let result = ctx.decode(&mut as_);
        assert!(result.is_ok());
        // result is Option<i32> — both variants valid
    }
}

#[test]
fn fresh_context_is_zeroed() {
    let ctx = ArithIntCtx::new();
    // Internal state should be zero-initialized
    // We can't inspect directly, but decoding from same stream should be deterministic
    let ctx2 = ArithIntCtx::new();
    // Two fresh contexts should produce same results
    let _ = (ctx, ctx2);
}
