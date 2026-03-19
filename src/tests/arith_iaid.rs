use crate::arith::ArithState;
use crate::arith_iaid::ArithIaidCtx;

const TEST_STREAM: &[u8] = &[
    0x84, 0xC7, 0x3B, 0xFC, 0xE1, 0xA1, 0x43, 0x04, 0x02, 0x20, 0x00, 0x00,
    0x41, 0x0D, 0xBB, 0x86, 0xF4, 0x31, 0x7F, 0xFF, 0x88, 0xFF, 0x37, 0x47,
    0x1A, 0xDB, 0x6A, 0xDF, 0xFF, 0xAC,
    0x00, 0x00,
];

#[test]
fn decode_symbol_id() {
    let mut as_ = ArithState::new(TEST_STREAM).unwrap();
    // 4-bit symbol code → up to 16 symbols
    let mut ctx = ArithIaidCtx::new(4).unwrap();

    let id = ctx.decode(&mut as_).unwrap();
    assert!(id < 16, "symbol ID should be in [0, 16), got {id}");

    // Decode more — all should be in range
    for _ in 0..10 {
        let id = ctx.decode(&mut as_).unwrap();
        assert!(id < 16, "symbol ID should be in [0, 16), got {id}");
    }
}

#[test]
fn decode_symbol_id_deterministic() {
    let mut as1 = ArithState::new(TEST_STREAM).unwrap();
    let mut ctx1 = ArithIaidCtx::new(6).unwrap();

    let mut as2 = ArithState::new(TEST_STREAM).unwrap();
    let mut ctx2 = ArithIaidCtx::new(6).unwrap();

    for i in 0..8 {
        let a = ctx1.decode(&mut as1).unwrap();
        let b = ctx2.decode(&mut as2).unwrap();
        assert_eq!(a, b, "IAID mismatch at decode {i}");
        assert!(a < 64); // 2^6 = 64
    }
}

#[test]
fn codelen_1() {
    // Minimal: 1 bit → 2 symbols (0 or 1)
    let mut as_ = ArithState::new(TEST_STREAM).unwrap();
    let mut ctx = ArithIaidCtx::new(1).unwrap();

    let id = ctx.decode(&mut as_).unwrap();
    assert!(id < 2);
}

#[test]
fn codelen_too_large() {
    let result = ArithIaidCtx::new(31);
    assert!(result.is_err());
}

#[test]
fn codelen_zero() {
    // 0 bits → 1 symbol (always 0), but context size = 1
    let mut as_ = ArithState::new(TEST_STREAM).unwrap();
    let mut ctx = ArithIaidCtx::new(0).unwrap();
    let id = ctx.decode(&mut as_).unwrap();
    assert_eq!(id, 0);
}
