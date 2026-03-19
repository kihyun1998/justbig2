# justbig2

Pure Rust JBIG2 image decoder with `no_std` support.

Decodes JBIG2 (ITU T.88) compressed bi-level images used in PDF documents, fax transmissions, and document archival systems.

## Features

- **Full JBIG2 decoding** — Generic, Text, Halftone, Refinement regions
- **Arithmetic & MMR coding** — QM arithmetic decoder + CCITT Group 4
- **Symbol dictionaries** — Glyph reuse across pages
- **Streaming API** — Feed data incrementally or all at once
- **`no_std` compatible** — Uses `alloc` only, no OS dependencies
- **Zero unsafe code** — Pure safe Rust

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
justbig2 = "0.1"
```

Decode a JBIG2 file:

```rust
use justbig2::{decode, Page};

let data = std::fs::read("input.jb2").unwrap();
let pages = decode(&data).unwrap();

for page in &pages {
    println!("{}x{} pixels, {} bytes", page.width, page.height, page.data.len());
    // page.data is 1BPP, MSB-first, stride-aligned
    // page.get_pixel(x, y) returns 0 or 1
}
```

## API

### One-shot decoding

```rust
// Full JBIG2 file (with file header)
let pages = justbig2::decode(&data)?;

// Embedded stream (no file header, used in PDF)
let pages = justbig2::decode_embedded(&data)?;
```

### Streaming decoder

```rust
use justbig2::Decoder;

let mut decoder = Decoder::new();

// Feed data in chunks
decoder.write(&chunk1)?;
decoder.write(&chunk2)?;

// Retrieve completed pages
while let Some(page) = decoder.page() {
    // process page
}
```

### Page struct

```rust
pub struct Page {
    pub width: u32,     // Image width in pixels
    pub height: u32,    // Image height in pixels
    pub stride: u32,    // Row stride in bytes (= ceil(width / 8))
    pub data: Vec<u8>,  // 1BPP pixel data, MSB-first
}

// Get individual pixel (0 = white, 1 = black)
let pixel = page.get_pixel(x, y);
```

## Supported Segment Types

| Type | Segment | Status |
|------|---------|--------|
| 0 | Symbol Dictionary | Supported |
| 4, 6, 7 | Text Region | Supported (arithmetic) |
| 16 | Pattern Dictionary | Supported |
| 20, 22, 23 | Halftone Region | Supported |
| 38, 39 | Generic Region | Supported |
| 40, 42, 43 | Refinement Region | Supported |
| 48 | Page Information | Supported |
| 49, 50, 51 | End of Page/Stripe/File | Supported |
| 52 | Profile | Parsed (informational) |
| 53 | Code Table | Supported |
| 62 | Extension | Parsed (comments) |

## Limitations

- **Huffman-coded text regions** — Only arithmetic coding path is implemented. Arithmetic coding is used by the vast majority of real-world JBIG2 files.
- **Color Palette segments (type 54)** — Not implemented. Defined in the spec but rarely used in practice.
- **Intermediate Generic Region (type 36)** — Not implemented. Not seen in real-world files.
- **12 adaptive template pixels (T.88 amendment 2)** — Not supported.
- **Colored region segments (T.88 amendment 3)** — Not supported.

## `no_std` Usage

Disable the default `std` feature:

```toml
[dependencies]
justbig2 = { version = "0.1", default-features = false }
```

The library uses `alloc` for `Vec` and `String`. The `std::error::Error` impl is only available with the `std` feature.

## License

Licensed under the Apache License, Version 2.0 ([LICENSE](LICENSE) or http://www.apache.org/licenses/LICENSE-2.0).
