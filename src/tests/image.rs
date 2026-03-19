use crate::image::{ComposeOp, Jbig2Image};

// --- Step 1.2: 비트맵 기본 ---

#[test]
fn new_dimensions() {
    let img = Jbig2Image::new(17, 5);
    assert_eq!(img.width, 17);
    assert_eq!(img.height, 5);
    // stride = ceil(17/8) = 3
    assert_eq!(img.stride, 3);
    assert_eq!(img.data.len(), 15); // 3 * 5
    // 초기값 0 (흰색)
    assert!(img.data.iter().all(|&b| b == 0));
}

#[test]
fn get_set_pixel() {
    let mut img = Jbig2Image::new(16, 4);

    // 초기값은 0
    assert_eq!(img.get_pixel(0, 0), 0);
    assert_eq!(img.get_pixel(15, 3), 0);

    // 설정
    img.set_pixel(0, 0, 1);
    assert_eq!(img.get_pixel(0, 0), 1);

    img.set_pixel(7, 0, 1);
    assert_eq!(img.get_pixel(7, 0), 1);
    // 같은 바이트 내 다른 비트는 영향 없음
    assert_eq!(img.get_pixel(6, 0), 0);

    img.set_pixel(15, 3, 1);
    assert_eq!(img.get_pixel(15, 3), 1);

    // 끄기
    img.set_pixel(0, 0, 0);
    assert_eq!(img.get_pixel(0, 0), 0);
}

#[test]
fn clear_white() {
    let mut img = Jbig2Image::new(8, 2);
    img.set_pixel(3, 1, 1);
    img.clear(0);
    assert_eq!(img.get_pixel(3, 1), 0);
    assert!(img.data.iter().all(|&b| b == 0));
}

#[test]
fn clear_black() {
    let mut img = Jbig2Image::new(8, 2);
    img.clear(1);
    assert_eq!(img.get_pixel(0, 0), 1);
    assert_eq!(img.get_pixel(7, 1), 1);
    assert!(img.data.iter().all(|&b| b == 0xFF));
}

#[test]
fn out_of_bounds() {
    let mut img = Jbig2Image::new(8, 4);
    // 범위 밖 읽기 → 0
    assert_eq!(img.get_pixel(8, 0), 0);
    assert_eq!(img.get_pixel(0, 4), 0);
    assert_eq!(img.get_pixel(100, 100), 0);

    // 범위 밖 쓰기 → 무시 (패닉 안 함)
    img.set_pixel(8, 0, 1);
    img.set_pixel(0, 4, 1);
    img.set_pixel(100, 100, 1);
}

// --- Step 1.3: 합성 연산 ---

/// 8x1 테스트 패턴을 만드는 헬퍼
fn make_pattern(bits: u8) -> Jbig2Image {
    let mut img = Jbig2Image::new(8, 1);
    img.data[0] = bits;
    img
}

#[test]
fn compose_or() {
    let mut dst = make_pattern(0b1010_0000);
    let src = make_pattern(0b1100_0000);
    dst.compose(&src, 0, 0, ComposeOp::Or).unwrap();
    assert_eq!(dst.data[0], 0b1110_0000);
}

#[test]
fn compose_and() {
    let mut dst = make_pattern(0b1010_0000);
    let src = make_pattern(0b1100_0000);
    dst.compose(&src, 0, 0, ComposeOp::And).unwrap();
    assert_eq!(dst.data[0], 0b1000_0000);
}

#[test]
fn compose_xor() {
    let mut dst = make_pattern(0b1010_0000);
    let src = make_pattern(0b1100_0000);
    dst.compose(&src, 0, 0, ComposeOp::Xor).unwrap();
    assert_eq!(dst.data[0], 0b0110_0000);
}

#[test]
fn compose_xnor() {
    let mut dst = make_pattern(0b1010_0000);
    let src = make_pattern(0b1100_0000);
    dst.compose(&src, 0, 0, ComposeOp::Xnor).unwrap();
    // XNOR: ~(d ^ s) → bits that are same = 1
    // d=1010, s=1100 → xor=0110 → xnor=1001
    assert_eq!(dst.data[0] & 0xF0, 0b1001_0000);
}

#[test]
fn compose_replace() {
    let mut dst = make_pattern(0b1111_0000);
    let src = make_pattern(0b1010_0000);
    dst.compose(&src, 0, 0, ComposeOp::Replace).unwrap();
    assert_eq!(dst.data[0], 0b1010_0000);
}

#[test]
fn compose_offset() {
    // 4x2 dst, 2x1 src → src를 (2,1)에 OR
    let mut dst = Jbig2Image::new(4, 2);
    let mut src = Jbig2Image::new(2, 1);
    src.set_pixel(0, 0, 1);
    src.set_pixel(1, 0, 1);

    dst.compose(&src, 2, 1, ComposeOp::Or).unwrap();

    // (2,1)과 (3,1)이 1이어야 함
    assert_eq!(dst.get_pixel(2, 1), 1);
    assert_eq!(dst.get_pixel(3, 1), 1);
    // 나머지는 0
    assert_eq!(dst.get_pixel(0, 0), 0);
    assert_eq!(dst.get_pixel(1, 0), 0);
    assert_eq!(dst.get_pixel(0, 1), 0);
    assert_eq!(dst.get_pixel(1, 1), 0);
}

#[test]
fn compose_clipping() {
    // src가 dst 바깥으로 삐져나가도 패닉 없이 클리핑
    let mut dst = Jbig2Image::new(4, 4);
    let mut src = Jbig2Image::new(4, 4);
    src.clear(1);

    // 오른쪽 아래로 밀어서 일부만 겹침
    dst.compose(&src, 2, 2, ComposeOp::Or).unwrap();
    assert_eq!(dst.get_pixel(2, 2), 1);
    assert_eq!(dst.get_pixel(3, 3), 1);
    assert_eq!(dst.get_pixel(1, 1), 0);

    // 음수 오프셋 (왼쪽 위로 밀기)
    let mut dst2 = Jbig2Image::new(4, 4);
    dst2.compose(&src, -2, -2, ComposeOp::Or).unwrap();
    assert_eq!(dst2.get_pixel(0, 0), 1);
    assert_eq!(dst2.get_pixel(1, 1), 1);
    assert_eq!(dst2.get_pixel(2, 0), 0); // src의 (4,2) → 범위밖
}

#[test]
fn compose_no_overlap() {
    // 완전히 바깥 → 아무 변화 없음
    let mut dst = Jbig2Image::new(4, 4);
    let mut src = Jbig2Image::new(2, 2);
    src.clear(1);

    dst.compose(&src, 10, 10, ComposeOp::Or).unwrap();
    assert!(dst.data.iter().all(|&b| b == 0));

    dst.compose(&src, -10, -10, ComposeOp::Or).unwrap();
    assert!(dst.data.iter().all(|&b| b == 0));
}
