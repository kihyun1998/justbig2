/// MSB-first bit reader over a byte slice.
pub struct BitReader<'a> {
    data: &'a [u8],
    /// Byte offset into data.
    byte_offset: usize,
    /// Bits remaining in current byte (MSB-first). 0 means need next byte.
    bits_left: u8,
    /// Current byte cache.
    current: u8,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        BitReader {
            data,
            byte_offset: 0,
            bits_left: 0,
            current: 0,
        }
    }

    /// Read a single bit (MSB-first). Returns 0 or 1.
    /// Returns 0 if past end of data.
    #[inline]
    pub fn read_bit(&mut self) -> u8 {
        if self.bits_left == 0 {
            if self.byte_offset >= self.data.len() {
                return 0;
            }
            self.current = self.data[self.byte_offset];
            self.byte_offset += 1;
            self.bits_left = 8;
        }
        self.bits_left -= 1;
        (self.current >> self.bits_left) & 1
    }

    /// Read `n` bits (MSB-first) as u32. Max 32 bits.
    pub fn read_bits(&mut self, n: u8) -> u32 {
        let mut val = 0u32;
        for _ in 0..n {
            val = (val << 1) | self.read_bit() as u32;
        }
        val
    }

    /// Read a full byte (8 bits, MSB-first).
    pub fn read_byte(&mut self) -> u8 {
        self.read_bits(8) as u8
    }

    /// Align to the next byte boundary. Discards remaining bits in current byte.
    pub fn align(&mut self) {
        self.bits_left = 0;
    }

    /// Number of bytes consumed so far (including partially read byte).
    pub fn bytes_consumed(&self) -> usize {
        self.byte_offset
    }

    /// Whether we've exhausted all data.
    pub fn is_eof(&self) -> bool {
        self.byte_offset >= self.data.len() && self.bits_left == 0
    }
}
