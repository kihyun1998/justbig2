//! justbig2 — Pure Rust JBIG2 decoder.
//!
//! # Quick Start
//!
//! ```rust
//! use justbig2::{Decoder, Page};
//!
//! let data = include_bytes!("../vendor/jbig2dec/annex-h.jbig2");
//! let mut decoder = Decoder::new();
//! decoder.write(data).unwrap();
//!
//! if let Some(page) = decoder.page() {
//!     println!("{}x{} image, {} bytes", page.width, page.height, page.data.len());
//! }
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

use alloc::vec::Vec;

// Internal modules
pub mod error;
pub mod image;
pub mod bitreader;
pub mod arith;
pub mod arith_int;
pub mod arith_iaid;
pub mod huffman;
pub mod header;
pub mod segment;
pub mod page;
pub mod decoder;
pub mod generic;
pub mod mmr;
pub mod refinement;
pub mod symbol_dict;
pub mod text;
pub mod halftone;

// --- Public API re-exports ---

pub use error::{Jbig2Error, Result};
pub use decoder::Decoder;

/// A decoded JBIG2 page image.
#[derive(Debug, Clone)]
pub struct Page {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Row stride in bytes (= ceil(width / 8)).
    pub stride: u32,
    /// 1BPP pixel data, MSB-first, stride-aligned rows.
    pub data: Vec<u8>,
}

impl Page {
    /// Get a single pixel value (0 or 1). Returns 0 for out-of-bounds.
    pub fn get_pixel(&self, x: u32, y: u32) -> u8 {
        if x >= self.width || y >= self.height {
            return 0;
        }
        let byte_idx = (y * self.stride + x / 8) as usize;
        let bit_idx = 7 - (x % 8);
        (self.data[byte_idx] >> bit_idx) & 1
    }
}

impl Decoder {
    /// Get the next completed page as a [`Page`] struct.
    /// Returns `None` if no page is ready.
    pub fn page(&mut self) -> Option<Page> {
        self.page_out().map(|img| Page {
            width: img.width,
            height: img.height,
            stride: img.stride,
            data: img.data,
        })
    }
}

/// Convenience: decode a complete JBIG2 byte stream in one call.
pub fn decode(data: &[u8]) -> Result<Vec<Page>> {
    let mut decoder = Decoder::new();
    decoder.write(data)?;
    let mut pages = Vec::new();
    while let Some(page) = decoder.page() {
        pages.push(page);
    }
    Ok(pages)
}

/// Convenience: decode an embedded (headerless) JBIG2 stream.
pub fn decode_embedded(data: &[u8]) -> Result<Vec<Page>> {
    let mut decoder = Decoder::new_embedded();
    decoder.write(data)?;
    let mut pages = Vec::new();
    while let Some(page) = decoder.page() {
        pages.push(page);
    }
    Ok(pages)
}

#[cfg(test)]
mod tests;
