# justbig2 — Pure Rust JBIG2 Decoder Roadmap

각 Phase/Step은 대응하는 테스트를 통과해야 완료로 간주한다.
테스트: `cargo test --lib` (유닛), `cargo test --test integration` (통합)

---

## Phase 1: 기반 자료구조 & 비트맵

기본 에러 타입, 1BPP 비트맵, 합성 연산을 먼저 만든다.
이후 모든 디코딩 결과가 이 비트맵 위에 그려진다.

### Step 1.1 — 에러 타입

- `Jbig2Error` enum (InvalidData, UnsupportedFeature, InternalError)
- `Result<T> = std::result::Result<T, Jbig2Error>`

**테스트:** `tests::error::error_display`, `error_equality`, `error_is_std_error`

### Step 1.2 — 1BPP 비트맵 (`Jbig2Image`)

- `width`, `height`, `stride` (바이트 단위, 행 패딩)
- `data: Vec<u8>` — MSB-first 패킹
- `new(w, h)` → 0으로 초기화
- `get_pixel(x, y) -> u8`
- `set_pixel(x, y, v)`
- `clear(v)` — 전체 0 또는 1로 채움

**테스트:** `tests::image::new_dimensions`, `get_set_pixel`, `clear_white`, `clear_black`, `out_of_bounds`

### Step 1.3 — 비트맵 합성 (Compose)

5가지 연산자: OR, AND, XOR, XNOR, REPLACE

- `compose(dst, src, x, y, op)` — src를 dst의 (x,y)에 합성
- 경계 클리핑 처리

**테스트:** `tests::image::compose_or`, `compose_and`, `compose_xor`, `compose_xnor`, `compose_replace`, `compose_offset`, `compose_clipping`, `compose_no_overlap`

---

## Phase 2: 비트 리더 & 산술 디코더

JBIG2의 핵심 엔트로피 코딩 엔진.

### Step 2.1 — 비트 리더

- 바이트 스트림에서 MSB-first로 비트 단위 읽기
- `read_bit()`, `read_bits(n)`, `read_byte()`
- 바이트 경계 정렬 `align()`

**테스트:** `tests::bitreader::read_bits_basic`, `read_across_bytes`, `read_byte_aligned`, `read_bits_multi_byte`, `align_to_byte`, `eof_detection`

### Step 2.2 — QM 산술 디코더

- `ArithState` — C/A 레지스터, 바이트-in
- `ArithCx` — 7-bit index + 1-bit MPS 컨텍스트
- `decode(&mut state, &mut cx) -> u8` — 1비트 디코딩
- FF 마커 바이트 스터핑 처리

**테스트:** `tests::arith::decode_known_sequence`, `context_adaptation`, `ff_marker_handling`, `multiple_contexts`, `short_stream`, `single_byte_error`

### Step 2.3 — 산술 정수 디코더 (Annex A.2)

- `ArithIntCtx` — 512개 컨텍스트
- `decode(&mut state, &mut ctx) -> Option<i32>`
- OOB 감지 (None 반환)

**테스트:** `tests::arith_int::decode_integers`, `decode_produces_values_or_oob`, `fresh_context_is_zeroed`

### Step 2.4 — 산술 IAID 디코더 (Annex A.3)

- `ArithIaidCtx::new(code_len)`
- `decode(&mut state, &mut ctx) -> u32`

**테스트:** `tests::arith_iaid::decode_symbol_id`, `decode_symbol_id_deterministic`, `codelen_1`, `codelen_too_large`, `codelen_zero`

---

## Phase 3: 허프만 디코더

### Step 3.1 — 허프만 테이블 구조

- `HuffmanTable` — prefix/range/value 엔트리 배열
- `HuffmanLine { preflen, rangelen, rangelow }`
- OOB / LOW / HIGH 특수 엔트리
- `build_table(lines) -> HuffmanTable`

**테스트:** `tests::huffman::build_standard_table_a`, `build_with_oob`, `all_standard_tables_valid`

### Step 3.2 — 허프만 디코딩

- `HuffmanState` — 비트 위치 추적
- `get(&mut state, table) -> Result<(i32, bool)>` — (값, OOB여부)
- `get_bits(&mut state, n) -> u32`
- `align(&mut state)` — 바이트 경계 정렬

**테스트:** `tests::huffman::decode_table_a_values`, `decode_table_a_second_line`, `decode_oob_signal`, `decode_multiple_values`, `get_bits_basic`, `align_boundary`

### Step 3.3 — 표준 테이블 15종 (Annex B)

- `STANDARD_TABLES: [&[HuffmanLine]; 15]` — Table A(B.1) ~ Table O(B.15)

**테스트:** `tests::huffman::table_b1_known_values`, `table_b4_known_values`, `standard_table_line_counts`

### Step 3.4 — 사용자 정의 허프만 테이블 (세그먼트 타입 53)

- 세그먼트 데이터에서 커스텀 테이블 파싱

**테스트:** `tests::huffman::parse_user_table_basic`, `parse_user_table_with_oob`

---

## Phase 4: 스트림 파싱 & 세그먼트 디스패치

### Step 4.1 — 파일 헤더 파싱

- 8바이트 매직넘버 검증: `97 4A 42 32 0D 0A 1A 0A`
- 플래그: Sequential/Random-access, 페이지 수
- Embedded 모드 (헤더 없음)

**테스트:** `tests::header::parse_valid_header`, `parse_random_access_unknown_pages`, `reject_bad_magic`, `parse_embedded_no_header`, `too_short_returns_none`, `reject_amendment2`

### Step 4.2 — 세그먼트 헤더 파싱

- 세그먼트 번호, 타입, 페이지 연관, 참조 세그먼트, 데이터 길이
- referred-to 세그먼트 목록 파싱

**테스트:** `tests::segment::parse_segment_header`, `parse_with_referred_segments`, `parse_needs_more_data`, `parse_region_segment_info`

### Step 4.3 — 세그먼트 디스패치

- 타입별 핸들러 라우팅 (0, 4, 6, 7, 16, 20, 22, 23, 38, 39, 40, 42, 43, 48~53, 62)
- 미지원 타입은 경고 후 스킵

**테스트:** `tests::segment::dispatch_known_types`, `skip_unknown_type`


### Step 4.4 — 페이지 관리

- `Page` — 상태 (New → Complete → Returned), 비트맵, 해상도
- 스트라이프 지원 (미지정 높이, 동적 확장)
- 기본 픽셀 값 (흰색/검은색)
- End of Page / End of Stripe / End of File 처리

**테스트:** `tests::page::create_page`, `page_default_pixel`, `stripe_extend`, `page_complete_state`, `page_end_row`

### Step 4.5 — 디코더 컨텍스트

- `Jbig2Ctx` — 상태 머신 (6개 상태), 세그먼트 저장, 페이지 관리
- `data_in(buf)` — 스트리밍 입력
- `page_out() -> Option<Jbig2Image>` — 완성된 페이지 반환
- 글로벌 컨텍스트 (Embedded 모드용)

**테스트:** `tests::decoder::sequential_state_machine`, `embedded_mode`, `incremental_write`, `no_page_before_complete`

---

## Phase 5: Generic Region 디코딩

### Step 5.1 — Generic Region 파라미터 파싱

- `GenericRegionParams` — 너비, 높이, 템플릿(0-3), GBAT, TPGD, MMR 플래그

**테스트:** `tests::generic::parse_params`, `stats_size_values`

### Step 5.2 — Template 0 (16-pixel context)

- 비최적화 구현 (모든 GBAT 위치 지원)
- 최적화 구현 (기본 GBAT일 때 워드 단위 처리)

**테스트:** `tests::generic::template0_basic`, `template0_deterministic`, `template0_unopt`, `zero_width_image`

### Step 5.3 — Template 1, 2, 3

- 각 템플릿 구현 (13/10/10 픽셀 컨텍스트)

**테스트:** `tests::generic::template1_basic`, `template2_basic`, `template3_basic`

### Step 5.4 — TPGD (Typical Prediction)

- 동일 행 스킵 최적화

**테스트:** `tests::generic::tpgd_skip_identical_rows`, `tpgd_all_templates`

### Step 5.5 — Generic Region MMR 모드

- 산술 코딩 대신 MMR로 디코딩 (Phase 6 의존)

**테스트:** `tests::generic::mmr_mode_decode`

---

## Phase 6: MMR (Modified Modified Read) 디코딩

### Step 6.1 — 런-렝스 코드 테이블

- 흰색/검은색 terminating 코드 (0~63)
- Makeup 코드 (64~2560+)
- EOL 마커

**테스트:** `tests::mmr::white_terminating_codes`, `black_terminating_codes`, `makeup_codes`

### Step 6.2 — 2D 디코딩

- Pass / Horizontal / Vertical 모드
- 참조 행 기반 디코딩

**테스트:** `tests::mmr::decode_simple_page`, `decode_alternating_pattern`

---

## Phase 7: Refinement Region 디코딩

### Step 7.1 — Refinement Template 0 & 1

- Template 0: 13픽셀 컨텍스트 (현재 3 + 참조 10)
- Template 1: 10픽셀 컨텍스트
- GRAT adaptive 파라미터

**테스트:** `tests::refinement::template0_refine`, `template1_refine`

### Step 7.2 — TPGRON (Typical Prediction)

**테스트:** `tests::refinement::tpgron_prediction`

---

## Phase 8: Symbol Dictionary

### Step 8.1 — 사전 구조 & 관리

- `SymbolDict` — 글리프 이미지 배열
- 사전 상속 (referred-to 세그먼트에서 심볼 가져오기)
- 사전 연결 (`cat`)
- 내보내기 심볼 선택

**테스트:** `tests::symbol_dict::create_empty`, `cat_two_dicts`, `export_symbols`

### Step 8.2 — 산술 코딩 심볼 사전 디코딩

- SDHUFF=false 경로
- 템플릿 0~3 선택, GBAT
- Refinement aggregation (SDREFAGG)

**테스트:** `tests::symbol_dict::decode_arithmetic`

### Step 8.3 — 허프만 코딩 심볼 사전 디코딩

- SDHUFF=true 경로
- DH/DW/BMSIZE/AGGINST 허프만 테이블

**테스트:** `tests::symbol_dict::decode_huffman`

---

## Phase 9: Text Region

### Step 9.1 — 텍스트 영역 파라미터 파싱

- SBHUFF, SBREFINE, TRANSPOSED, REFCORNER, SBCOMBOP
- 스트립 파라미터, 인스턴스 수

**테스트:** `tests::text::parse_params`

### Step 9.2 — 산술 코딩 텍스트 디코딩

- 10개 산술 컨텍스트 (IADT/IAFS/IADS/IAIT/IAID/IARI/...)
- 인스턴스별 글리프 배치 (델타 위치)
- REFCORNER 4종, TRANSPOSED

**테스트:** `tests::text::decode_arithmetic_basic`, `decode_transposed`

### Step 9.3 — 허프만 코딩 텍스트 디코딩

- 8개 허프만 테이블 (FS/DS/DT/RDW/RDH/RDX/RDY/RSIZE)

**테스트:** `tests::text::decode_huffman_basic`

### Step 9.4 — 텍스트 리파인먼트

- SBREFINE=true일 때 배치 시 글리프 정제

**테스트:** `tests::text::decode_with_refinement`

---

## Phase 10: Halftone Region

### Step 10.1 — 패턴 사전 (세그먼트 타입 16)

- 타일 크기 (HDPW×HDPH), 그레이 레벨 수 (GRAYMAX)
- MMR/Arithmetic 디코딩

**테스트:** `tests::halftone::parse_pattern_dict`, `decode_patterns`

### Step 10.2 — 하프톤 영역 디코딩

- 그리드 배치 (HGW, HGH, HGX, HGY, HRX, HRY)
- Skip 마스크 (HENABLESKIP)
- 합성 연산자 적용

**테스트:** `tests::halftone::decode_halftone_region`, `halftone_with_skip`

---

## Phase 11: 통합 & Annex-H 테스트

### Step 11.1 — annex-h.jbig2 디코딩

- jbig2dec 번들 테스트 파일 (`annex-h.jbig2`)
- Full file header → 세그먼트 파싱 → 페이지 출력
- 디코딩 결과 비트맵 해시 검증

**테스트:** `tests::integration::decode_annex_h`

### Step 11.2 — Embedded 모드 (PDF 내장)

- 헤더 없는 스트림, 글로벌 심볼 사전
- `Jbig2GlobalCtx` → `Jbig2Ctx` 전달

**테스트:** `tests::integration::decode_embedded_stream`

### Step 11.3 — 에러 복원 & 엣지 케이스

- 잘린 스트림 처리
- data_length=0xFFFFFFFF (Xerox 호환)
- 빈 페이지
- 참조 세그먼트 누락 시 에러

**테스트:** `tests::integration::truncated_stream`, `xerox_compat`, `empty_page`, `missing_reference`

---

## Phase 12: 공개 API & 최적화

### Step 12.1 — 공개 API 정리

```rust
pub struct Decoder { /* ... */ }

impl Decoder {
    pub fn new() -> Self;
    pub fn new_embedded(global: GlobalContext) -> Self;
    pub fn write(&mut self, data: &[u8]) -> Result<()>;
    pub fn page(&mut self) -> Option<Page>;
}

pub struct Page {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,  // 1BPP, stride-aligned
    pub stride: u32,
}
```

**테스트:** `tests::api::basic_decode_flow`, `api_embedded_flow`

### Step 12.2 — 성능 최적화

- Generic Region 워드 단위 최적화 (Template 0~3)
- Compose 바이트/워드 정렬 최적화
- 벤치마크 (`cargo bench`)

**테스트:** 벤치마크 회귀 확인 (기준값 대비)

### Step 12.3 — `no_std` 지원 (선택)

- `alloc` crate 사용, std 의존 제거
- feature flag: `default = ["std"]`

**테스트:** `cargo test --no-default-features`

---

## 의존성 그래프

```
Phase 1 (비트맵)
  └─→ Phase 2 (산술 디코더)
  │     └─→ Phase 5 (Generic Region)
  │     └─→ Phase 7 (Refinement)
  │     └─→ Phase 8 (Symbol Dict) → Phase 9 (Text)
  │     └─→ Phase 10 (Halftone)
  └─→ Phase 3 (허프만)
  │     └─→ Phase 8, 9, 10
  └─→ Phase 4 (스트림 파싱)
  │     └─→ Phase 11 (통합)
  └─→ Phase 6 (MMR)
        └─→ Phase 5.5, Phase 10
```

## 테스트 실행 가이드

```bash
# 전체 유닛 테스트
cargo test --lib

# 특정 Phase 테스트
cargo test tests::image       # Phase 1
cargo test tests::arith       # Phase 2
cargo test tests::huffman     # Phase 3
cargo test tests::header       # Phase 4
cargo test tests::segment     # Phase 4
cargo test tests::page         # Phase 4
cargo test tests::decoder     # Phase 4
cargo test tests::generic     # Phase 5
cargo test tests::mmr         # Phase 6
cargo test tests::refinement  # Phase 7
cargo test tests::symbol_dict # Phase 8
cargo test tests::text        # Phase 9
cargo test tests::halftone    # Phase 10

# 통합 테스트
cargo test --test integration  # Phase 11

# 벤치마크
cargo bench                    # Phase 12
```
