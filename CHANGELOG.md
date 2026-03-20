# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.2] - 2026-03-20

### Added
- GitHub Actions CI (check, test, fmt, clippy, no_std, MSRV)
- MSRV (Minimum Supported Rust Version) set to 1.56.0

### Changed
- License changed from Apache-2.0 to dual MIT OR Apache-2.0

## [0.1.1] - 2026-03-19

### Fixed
- Corrected license identifier in Cargo.toml (`MIT OR Apache-2.0`)

## [0.1.0] - 2026-03-19

### Added
- Initial release
- Full JBIG2 decoder (ITU T.88) with arithmetic and MMR coding
- Generic Region decoding (Templates 0-3 with TPGD)
- Text Region decoding (arithmetic, all REFCORNER modes, TRANSPOSED)
- Halftone Region decoding with pattern dictionaries
- Refinement Region decoding (Templates 0-1 with TPGRON)
- Symbol Dictionary management (create, concat, export)
- MMR (CCITT Group 4) decoder
- Huffman decoder with 15 standard tables + user-defined tables
- QM arithmetic decoder with integer and IAID contexts
- Streaming decoder API (`Decoder::write` / `Decoder::page`)
- One-shot API (`decode()` / `decode_embedded()`)
- `Page` struct with `get_pixel()` accessor
- File header parsing (Sequential, Random-access, Embedded modes)
- Segment header parsing with 22 segment types
- Page management with stripe support
- `no_std` support via `alloc` crate
- Criterion benchmarks (annex-h decode ~26µs)
