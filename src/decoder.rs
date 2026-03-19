//! JBIG2 decoder context — top-level state machine.

use crate::error::{Jbig2Error, Result};
use crate::header::{FileHeader, Organization};
use crate::image::Jbig2Image;
use crate::page::{Page, PageState};
use crate::segment::{SegmentHeader, SegmentType};

/// Decoder state machine states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecoderState {
    FileHeader,
    SequentialHeader,
    SequentialBody,
    RandomHeaders,
    RandomBodies,
    Eof,
}

/// A stored segment (header + data).
#[derive(Debug, Clone)]
pub struct StoredSegment {
    pub header: SegmentHeader,
    pub data: Vec<u8>,
}

/// Top-level JBIG2 decoder context.
pub struct Decoder {
    pub(crate) state: DecoderState,
    pub(crate) embedded: bool,
    pub(crate) buf: Vec<u8>,
    pub(crate) segments: Vec<StoredSegment>,
    pub(crate) segment_index: usize,
    pub(crate) pages: Vec<Page>,
    pub(crate) current_page: usize,
    pub(crate) file_header: Option<FileHeader>,
    /// Global segments (from embedded mode global context).
    pub(crate) global_segments: Vec<StoredSegment>,
}

impl Decoder {
    /// Create a new decoder for a full JBIG2 file stream.
    pub fn new() -> Self {
        let mut pages = Vec::with_capacity(4);
        pages.push(Page::new());
        Decoder {
            state: DecoderState::FileHeader,
            embedded: false,
            buf: Vec::new(),
            segments: Vec::new(),
            segment_index: 0,
            pages,
            current_page: 0,
            file_header: None,
            global_segments: Vec::new(),
        }
    }

    /// Create a new decoder for an embedded (headerless) stream.
    pub fn new_embedded() -> Self {
        let mut pages = Vec::with_capacity(4);
        pages.push(Page::new());
        Decoder {
            state: DecoderState::SequentialHeader,
            embedded: true,
            buf: Vec::new(),
            segments: Vec::new(),
            segment_index: 0,
            pages,
            current_page: 0,
            file_header: None,
            global_segments: Vec::new(),
        }
    }

    /// Submit data for decoding.
    pub fn write(&mut self, data: &[u8]) -> Result<()> {
        self.buf.extend_from_slice(data);
        self.process()
    }

    /// Get the next completed page, if any.
    pub fn page_out(&mut self) -> Option<Jbig2Image> {
        for page in &mut self.pages {
            if page.state == PageState::Complete {
                page.state = PageState::Returned;
                return page.image.clone();
            }
        }
        None
    }

    /// Internal: process buffered data through the state machine.
    fn process(&mut self) -> Result<()> {
        loop {
            match self.state {
                DecoderState::FileHeader => {
                    let Some((header, consumed)) = FileHeader::parse(&self.buf)? else {
                        return Ok(());
                    };
                    let next_state = match header.organization {
                        Organization::Sequential => DecoderState::SequentialHeader,
                        Organization::RandomAccess => DecoderState::RandomHeaders,
                    };
                    self.file_header = Some(header);
                    self.buf.drain(..consumed);
                    self.state = next_state;
                }
                DecoderState::SequentialHeader | DecoderState::RandomHeaders => {
                    match SegmentHeader::parse(&self.buf)? {
                        None => return Ok(()), // need more data
                        Some((header, consumed)) => {
                            self.buf.drain(..consumed);

                            let is_eof = header.seg_type == Some(SegmentType::EndOfFile);

                            self.segments.push(StoredSegment {
                                header,
                                data: Vec::new(),
                            });

                            if self.state == DecoderState::RandomHeaders {
                                if is_eof {
                                    self.state = DecoderState::RandomBodies;
                                }
                            } else {
                                self.state = DecoderState::SequentialBody;
                            }
                        }
                    }
                }
                DecoderState::SequentialBody | DecoderState::RandomBodies => {
                    if self.segment_index >= self.segments.len() {
                        self.state = DecoderState::Eof;
                        return Ok(());
                    }

                    let data_length = self.segments[self.segment_index].header.data_length;

                    // Handle unknown length (0xFFFFFFFF) — for now just take all remaining
                    let need = if data_length == 0xFFFFFFFF {
                        // Scan for marker
                        self.buf.len() // TODO: proper marker scanning
                    } else {
                        data_length as usize
                    };

                    if self.buf.len() < need {
                        return Ok(()); // need more data
                    }

                    let seg_data: Vec<u8> = self.buf.drain(..need).collect();
                    self.segments[self.segment_index].data = seg_data;

                    // Dispatch segment
                    self.dispatch_segment(self.segment_index)?;
                    self.segment_index += 1;

                    // Don't override Eof set by dispatch (e.g. EndOfFile segment)
                    if self.state != DecoderState::Eof {
                        if self.state == DecoderState::RandomBodies {
                            if self.segment_index >= self.segments.len() {
                                self.state = DecoderState::Eof;
                            }
                        } else {
                            self.state = DecoderState::SequentialHeader;
                        }
                    }
                }
                DecoderState::Eof => {
                    return Ok(());
                }
            }
        }
    }

    /// Dispatch a single segment by index.
    fn dispatch_segment(&mut self, seg_idx: usize) -> Result<()> {
        let seg_type = self.segments[seg_idx].header.seg_type;
        let page_assoc = self.segments[seg_idx].header.page_association;

        match seg_type {
            Some(SegmentType::PageInformation) => {
                let data = self.segments[seg_idx].data.clone();
                // Find or create page
                self.ensure_page(page_assoc);
                let page = &mut self.pages[self.current_page];
                page.parse_info(page_assoc, &data)?;
            }
            Some(SegmentType::EndOfPage) => {
                self.pages[self.current_page].complete();
            }
            Some(SegmentType::EndOfStripe) => {
                let data = &self.segments[seg_idx].data;
                if data.len() >= 4 {
                    let end_row = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
                    self.pages[self.current_page].set_end_row(end_row);
                }
            }
            Some(SegmentType::EndOfFile) => {
                self.state = DecoderState::Eof;
            }
            Some(SegmentType::Profile) | Some(SegmentType::Extension) => {
                // Informational — skip
            }
            Some(SegmentType::IntermediateGenericRegion) => {
                return Err(Jbig2Error::UnsupportedFeature(
                    "intermediate generic region (type 36)".into(),
                ));
            }
            Some(SegmentType::ColorPalette) => {
                return Err(Jbig2Error::UnsupportedFeature(
                    "color palette (type 54)".into(),
                ));
            }
            None => {
                // Unknown segment type — skip with warning
            }
            _ => {
                // TODO: dispatch to actual decoders (generic, text, halftone, refinement, symbol dict, code table)
                // For now, store segment data for later use
            }
        }

        Ok(())
    }

    /// Ensure a page slot exists for the given page number.
    fn ensure_page(&mut self, _page_number: u32) {
        // Find a free page slot or create one
        for (i, page) in self.pages.iter().enumerate() {
            if page.state == PageState::Free {
                self.current_page = i;
                return;
            }
        }
        // No free slot — add one
        self.current_page = self.pages.len();
        self.pages.push(Page::new());
    }
}
