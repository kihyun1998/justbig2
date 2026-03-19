use alloc::vec;
use alloc::vec::Vec;

use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposeOp {
    Or = 0,
    And = 1,
    Xor = 2,
    Xnor = 3,
    Replace = 4,
}

#[derive(Debug, Clone)]
pub struct Jbig2Image {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub data: Vec<u8>,
}

impl Jbig2Image {
    pub fn new(width: u32, height: u32) -> Self {
        let stride = (width + 7) / 8;
        let len = (stride * height) as usize;
        Jbig2Image {
            width,
            height,
            stride,
            data: vec![0u8; len],
        }
    }

    pub fn get_pixel(&self, x: u32, y: u32) -> u8 {
        if x >= self.width || y >= self.height {
            return 0;
        }
        let byte_idx = (y * self.stride + x / 8) as usize;
        let bit_idx = 7 - (x % 8);
        (self.data[byte_idx] >> bit_idx) & 1
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, v: u8) {
        if x >= self.width || y >= self.height {
            return;
        }
        let byte_idx = (y * self.stride + x / 8) as usize;
        let bit_idx = 7 - (x % 8);
        if v != 0 {
            self.data[byte_idx] |= 1 << bit_idx;
        } else {
            self.data[byte_idx] &= !(1 << bit_idx);
        }
    }

    pub fn clear(&mut self, v: u8) {
        let fill = if v != 0 { 0xFF } else { 0x00 };
        self.data.fill(fill);
    }

    /// Resize the image, preserving existing content. New rows are filled with `fill_value`.
    pub fn resize(&mut self, new_width: u32, new_height: u32, fill_black: bool) {
        let new_stride = (new_width + 7) / 8;
        let new_len = (new_stride * new_height) as usize;
        let fill = if fill_black { 0xFF } else { 0x00 };

        if new_stride == self.stride {
            // Same stride — just extend/truncate
            self.data.resize(new_len, fill);
        } else {
            // Different stride — need to copy row by row
            let mut new_data = vec![fill; new_len];
            let copy_rows = self.height.min(new_height);
            let copy_bytes = self.stride.min(new_stride) as usize;
            for row in 0..copy_rows {
                let src_off = (row * self.stride) as usize;
                let dst_off = (row * new_stride) as usize;
                new_data[dst_off..dst_off + copy_bytes]
                    .copy_from_slice(&self.data[src_off..src_off + copy_bytes]);
            }
            self.data = new_data;
        }

        self.width = new_width;
        self.height = new_height;
        self.stride = new_stride;
    }

    pub fn compose(&mut self, src: &Jbig2Image, dx: i32, dy: i32, op: ComposeOp) -> Result<()> {
        let src_x_start = if dx < 0 { (-dx) as u32 } else { 0 };
        let src_y_start = if dy < 0 { (-dy) as u32 } else { 0 };
        let dst_x_start = dx.max(0) as u32;
        let dst_y_start = dy.max(0) as u32;

        let copy_w = src.width.saturating_sub(src_x_start).min(self.width.saturating_sub(dst_x_start));
        let copy_h = src.height.saturating_sub(src_y_start).min(self.height.saturating_sub(dst_y_start));

        if copy_w == 0 || copy_h == 0 {
            return Ok(());
        }

        for row in 0..copy_h {
            for col in 0..copy_w {
                let sv = src.get_pixel(src_x_start + col, src_y_start + row);
                let sx = dst_x_start + col;
                let sy = dst_y_start + row;
                let dv = self.get_pixel(sx, sy);
                let result = match op {
                    ComposeOp::Or => dv | sv,
                    ComposeOp::And => dv & sv,
                    ComposeOp::Xor => dv ^ sv,
                    ComposeOp::Xnor => (dv ^ sv) ^ 1,
                    ComposeOp::Replace => sv,
                };
                self.set_pixel(sx, sy, result);
            }
        }
        Ok(())
    }
}
