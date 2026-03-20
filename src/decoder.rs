//! JBIG2 decoder context — top-level state machine.

use alloc::vec;
use alloc::vec::Vec;

use crate::arith::{ArithCx, ArithState};
use crate::error::{Jbig2Error, Result};
use crate::generic::{self, GenericRegionParams};
use crate::header::{FileHeader, Organization};
use crate::image::{ComposeOp, Jbig2Image};
use crate::mmr;
use crate::page::{Page, PageState};
use crate::segment::{RegionSegmentInfo, SegmentHeader, SegmentType};
use crate::symbol_dict::{SymbolDict, SymbolDictParams};

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

/// A stored segment (header + data + optional result).
#[derive(Debug, Clone)]
pub struct StoredSegment {
    pub header: SegmentHeader,
    pub data: Vec<u8>,
    /// Decoded symbol dictionary result, if any.
    pub symbol_dict: Option<SymbolDict>,
}

/// Top-level JBIG2 decoder context.
pub struct Decoder {
    pub(crate) state: DecoderState,
    pub(crate) buf: Vec<u8>,
    pub(crate) segments: Vec<StoredSegment>,
    pub(crate) segment_index: usize,
    pub(crate) pages: Vec<Page>,
    pub(crate) current_page: usize,
    pub(crate) file_header: Option<FileHeader>,
    pub(crate) global_segments: Vec<StoredSegment>,
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder {
    /// Create a new decoder for a full JBIG2 file stream.
    pub fn new() -> Self {
        let mut pages = Vec::with_capacity(4);
        pages.push(Page::new());
        Decoder {
            state: DecoderState::FileHeader,
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

    /// Load global segment data (e.g., from a PDF JBIG2Globals stream).
    ///
    /// In PDF, JBIG2 images can share global segments (typically symbol
    /// dictionaries) across multiple embedded streams. Call this before
    /// [`write()`](Decoder::write) on the embedded stream.
    ///
    /// # Example
    /// ```ignore
    /// let mut decoder = Decoder::new_embedded();
    /// decoder.set_globals(globals_data)?;
    /// decoder.write(stream_data)?;
    /// ```
    pub fn set_globals(&mut self, globals_data: &[u8]) -> Result<()> {
        let mut tmp = Decoder::new_embedded();
        tmp.write(globals_data)?;
        self.global_segments = tmp.segments;
        Ok(())
    }

    /// Set pre-parsed global segments directly.
    ///
    /// Useful when the same globals are shared across many images —
    /// parse once with [`parse_globals()`](Decoder::parse_globals),
    /// then reuse with each decoder.
    pub fn set_global_segments(&mut self, segments: Vec<StoredSegment>) {
        self.global_segments = segments;
    }

    /// Parse global data and return the resulting segments for caching.
    ///
    /// Use with [`set_global_segments()`](Decoder::set_global_segments)
    /// to avoid re-parsing the same globals for every image.
    pub fn parse_globals(globals_data: &[u8]) -> Result<Vec<StoredSegment>> {
        let mut tmp = Decoder::new_embedded();
        tmp.write(globals_data)?;
        Ok(tmp.segments)
    }

    /// Find a stored segment by number, searching local segments first,
    /// then global segments.
    fn find_segment(&self, seg_number: u32) -> Option<&StoredSegment> {
        self.segments
            .iter()
            .find(|s| s.header.number == seg_number)
            .or_else(|| {
                self.global_segments
                    .iter()
                    .find(|s| s.header.number == seg_number)
            })
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
                        None => return Ok(()),
                        Some((header, consumed)) => {
                            self.buf.drain(..consumed);
                            let is_eof = header.seg_type == Some(SegmentType::EndOfFile);
                            self.segments.push(StoredSegment {
                                header,
                                data: Vec::new(),
                                symbol_dict: None,
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
                    let need = if data_length == 0xFFFFFFFF {
                        self.buf.len()
                    } else {
                        data_length as usize
                    };

                    if self.buf.len() < need {
                        return Ok(());
                    }

                    let seg_data: Vec<u8> = self.buf.drain(..need).collect();
                    self.segments[self.segment_index].data = seg_data;

                    self.dispatch_segment(self.segment_index)?;
                    self.segment_index += 1;

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
                self.ensure_page(page_assoc);
                self.pages[self.current_page].parse_info(page_assoc, &data)?;
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
            Some(SegmentType::ImmediateGenericRegion)
            | Some(SegmentType::ImmediateLosslessGenericRegion) => {
                self.decode_immediate_generic(seg_idx)?;
            }
            Some(SegmentType::SymbolDictionary) => {
                self.decode_symbol_dictionary(seg_idx)?;
            }
            Some(SegmentType::ImmediateTextRegion)
            | Some(SegmentType::ImmediateLosslessTextRegion) => {
                self.decode_immediate_text(seg_idx)?;
            }
            Some(SegmentType::Profile) | Some(SegmentType::Extension) => {}
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
            None => {}
            _ => {
                // Other segment types: skip for now
            }
        }

        Ok(())
    }

    /// Decode an immediate generic region segment (type 38/39).
    fn decode_immediate_generic(&mut self, seg_idx: usize) -> Result<()> {
        let data = &self.segments[seg_idx].data;
        if data.len() < 18 {
            return Err(Jbig2Error::InvalidData("generic region segment too short".into()));
        }

        let rsi = RegionSegmentInfo::parse(&data[..17])?;
        let seg_flags = data[17];
        let (mut params, gbat_size) = GenericRegionParams::parse(seg_flags);

        if !params.mmr && data.len() >= 18 + gbat_size {
            params.set_gbat(&data[18..18 + gbat_size]);
        }

        let offset = 18 + gbat_size;
        let region_data = &data[offset..];

        let mut image = Jbig2Image::new(rsi.width, rsi.height);

        if params.mmr {
            mmr::decode_generic_mmr(region_data, &mut image)?;
        } else {
            let stats_size = generic::stats_size(params.gb_template);
            let mut gb_stats = vec![0u8 as ArithCx; stats_size];
            let mut as_ = ArithState::new(region_data)?;
            generic::decode_generic_region(&params, &mut as_, &mut image, &mut gb_stats)?;
        }

        let op = compose_op_from_u8(rsi.op);
        self.pages[self.current_page].add_result(&image, rsi.x, rsi.y, op)?;

        Ok(())
    }

    /// Decode a symbol dictionary segment (type 0) — simplified.
    fn decode_symbol_dictionary(&mut self, seg_idx: usize) -> Result<()> {
        let data = self.segments[seg_idx].data.clone();
        let referred = self.segments[seg_idx].header.referred_to_segments.clone();

        // Collect input symbol dictionaries from referred segments (local + global)
        let mut input_dicts: Vec<SymbolDict> = Vec::new();
        for ref_seg_num in &referred {
            if let Some(seg) = self.find_segment(*ref_seg_num) {
                if let Some(ref sd) = seg.symbol_dict {
                    input_dicts.push(sd.clone());
                }
            }
        }
        let sdnuminsyms: u32 = input_dicts.iter().map(|d| d.n_symbols()).sum();

        if let Some((mut params, _offset)) = SymbolDictParams::parse(&data) {
            params.sdnuminsyms = sdnuminsyms;
            let dict = SymbolDict::new(params.sdnumnewsyms);
            self.segments[seg_idx].symbol_dict = Some(dict);
        }
        Ok(())
    }

    /// Decode an immediate text region segment (type 6/7) — simplified.
    fn decode_immediate_text(&mut self, seg_idx: usize) -> Result<()> {
        let data = &self.segments[seg_idx].data;
        if data.len() < 17 {
            return Err(Jbig2Error::InvalidData("text region segment too short".into()));
        }

        let rsi = RegionSegmentInfo::parse(&data[..17])?;

        // Collect referred-to symbol dictionaries (local + global)
        let referred = self.segments[seg_idx].header.referred_to_segments.clone();
        let mut dicts: Vec<SymbolDict> = Vec::new();
        for ref_seg_num in &referred {
            if let Some(seg) = self.find_segment(*ref_seg_num) {
                if let Some(ref sd) = seg.symbol_dict {
                    dicts.push(sd.clone());
                }
            }
        }

        // Parse text region params from data[17..]
        if let Some((params, offset)) = crate::text::TextRegionParams::parse(&data[17..]) {
            let region_data = &data[17 + offset..];
            let dict_refs: Vec<&SymbolDict> = dicts.iter().collect();
            let sbnumsyms: u32 = dict_refs.iter().map(|d| d.n_symbols()).sum();

            let mut image = Jbig2Image::new(rsi.width, rsi.height);

            if !params.sbhuff && sbnumsyms > 0 {
                let mut as_ = ArithState::new(region_data)?;
                crate::text::decode_text_region(
                    &params, &mut as_, &mut image, &dict_refs, sbnumsyms,
                )?;
            }

            let op = compose_op_from_u8(rsi.op);
            self.pages[self.current_page].add_result(&image, rsi.x, rsi.y, op)?;
        }

        Ok(())
    }

    fn ensure_page(&mut self, _page_number: u32) {
        for (i, page) in self.pages.iter().enumerate() {
            if page.state == PageState::Free {
                self.current_page = i;
                return;
            }
        }
        self.current_page = self.pages.len();
        self.pages.push(Page::new());
    }
}

fn compose_op_from_u8(v: u8) -> ComposeOp {
    match v & 7 {
        0 => ComposeOp::Or,
        1 => ComposeOp::And,
        2 => ComposeOp::Xor,
        3 => ComposeOp::Xnor,
        4 => ComposeOp::Replace,
        _ => ComposeOp::Or,
    }
}
