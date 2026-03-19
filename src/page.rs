//! JBIG2 page management (ITU T.88 7.4.8).

use crate::error::{Jbig2Error, Result};
use crate::image::{ComposeOp, Jbig2Image};

/// Page state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageState {
    Free,
    New,
    Complete,
    Returned,
    Released,
}

/// A JBIG2 page.
#[derive(Debug, Clone)]
pub struct Page {
    pub state: PageState,
    pub number: u32,
    pub width: u32,
    pub height: u32,
    pub x_resolution: u32,
    pub y_resolution: u32,
    pub flags: u8,
    pub striped: bool,
    pub stripe_size: u16,
    pub end_row: u32,
    pub image: Option<Jbig2Image>,
}

impl Page {
    pub fn new() -> Self {
        Page {
            state: PageState::Free,
            number: 0,
            width: 0,
            height: 0xFFFFFFFF,
            x_resolution: 0,
            y_resolution: 0,
            flags: 0,
            striped: false,
            stripe_size: 0,
            end_row: 0,
            image: None,
        }
    }

    /// Parse page info segment data (7.4.8) and initialize the page.
    pub fn parse_info(&mut self, page_number: u32, data: &[u8]) -> Result<()> {
        if data.len() < 19 {
            return Err(Jbig2Error::InvalidData("page info segment too short".into()));
        }

        self.state = PageState::New;
        self.number = page_number;
        self.width = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        self.height = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        self.x_resolution = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
        self.y_resolution = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);
        self.flags = data[16];

        // Check for color segments (T.88 amendment 3)
        if self.flags & 0x80 != 0 {
            return Err(Jbig2Error::UnsupportedFeature("color segments".into()));
        }

        // 7.4.8.6 striping
        let striping = i16::from_be_bytes([data[17], data[18]]);
        if striping < 0 {
            self.striped = true;
            self.stripe_size = (striping & 0x7FFF) as u16;
        } else {
            self.striped = false;
            self.stripe_size = 0;
        }

        // Unspecified height but not striped → assume striped
        if self.height == 0xFFFFFFFF && !self.striped {
            self.striped = true;
            self.stripe_size = 0x7FFF;
        }

        self.end_row = 0;

        // Allocate image
        let img_height = if self.height == 0xFFFFFFFF {
            self.stripe_size as u32
        } else {
            self.height
        };
        let mut image = Jbig2Image::new(self.width, img_height);

        // Default pixel value (bit 2 of flags)
        if self.flags & 4 != 0 {
            image.clear(1);
        }

        self.image = Some(image);
        Ok(())
    }

    /// Composite a result image onto this page.
    pub fn add_result(&mut self, src: &Jbig2Image, x: u32, y: u32, op: ComposeOp) -> Result<()> {
        let image = self.image.as_mut().ok_or_else(|| {
            Jbig2Error::InvalidData("page has no image".into())
        })?;

        // Grow striped page if needed
        if self.striped && self.height == 0xFFFFFFFF {
            let new_height = y.checked_add(src.height).ok_or_else(|| {
                Jbig2Error::InvalidData("page height overflow".into())
            })?;
            if image.height < new_height {
                image.resize(image.width, new_height, self.flags & 4 != 0);
            }
        }

        image.compose(src, x as i32, y as i32, op)
    }

    /// Mark page as complete.
    pub fn complete(&mut self) {
        self.state = PageState::Complete;
    }

    /// Update end_row from end-of-stripe segment.
    pub fn set_end_row(&mut self, end_row: u32) {
        self.end_row = end_row;
    }
}
