/// Compact bit vector with rank and select operations.
///
/// Space: n bits + O(n / 64) overhead for rank index.
/// Time: rank O(1), select O(log n).
pub struct BitVec {
    /// Packed bits, 64 per word.
    data: Vec<u64>,
    /// Total number of bits.
    len: usize,
    /// Cumulative popcount at each 64-bit block boundary.
    rank_index: Vec<u32>,
}

impl BitVec {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            len: 0,
            rank_index: vec![0],
        }
    }

    pub fn from_bits(bits: &[bool]) -> Self {
        let len = bits.len();
        let num_words = len.div_ceil(64);
        let mut data = vec![0u64; num_words];

        for (i, &bit) in bits.iter().enumerate() {
            if bit {
                data[i / 64] |= 1u64 << (i % 64);
            }
        }

        let mut rank_index = Vec::with_capacity(num_words + 1);
        rank_index.push(0);
        let mut cumulative = 0u32;
        for &word in &data {
            cumulative += word.count_ones();
            rank_index.push(cumulative);
        }

        Self {
            data,
            len,
            rank_index,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get the bit at position `pos`.
    pub fn get(&self, pos: usize) -> bool {
        if pos >= self.len {
            return false;
        }
        (self.data[pos / 64] >> (pos % 64)) & 1 == 1
    }

    /// Count of 1-bits in [0, pos).
    pub fn rank1(&self, pos: usize) -> usize {
        if pos == 0 {
            return 0;
        }
        let pos = pos.min(self.len);
        let block = pos / 64;
        let remainder = pos % 64;

        let mut count = self.rank_index[block] as usize;
        if remainder > 0 {
            let mask = (1u64 << remainder) - 1;
            count += (self.data[block] & mask).count_ones() as usize;
        }
        count
    }

    /// Count of 0-bits in [0, pos).
    pub fn rank0(&self, pos: usize) -> usize {
        let pos = pos.min(self.len);
        pos - self.rank1(pos)
    }

    /// Position of the i-th 1-bit (0-indexed).
    pub fn select1(&self, i: usize) -> Option<usize> {
        let total_ones = self.count_ones();
        if i >= total_ones {
            return None;
        }

        // Binary search on rank_index to find the block
        let mut lo = 0usize;
        let mut hi = self.data.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            if (self.rank_index[mid + 1] as usize) <= i {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }

        let block = lo;
        let mut remaining = i - self.rank_index[block] as usize;
        let mut word = self.data[block];

        // Find the position within the word
        loop {
            if word == 0 {
                return None;
            }
            let tz = word.trailing_zeros() as usize;
            if remaining == 0 {
                let pos = block * 64 + tz;
                return if pos < self.len { Some(pos) } else { None };
            }
            remaining -= 1;
            word &= word - 1; // clear lowest set bit
        }
    }

    /// Position of the i-th 0-bit (0-indexed).
    pub fn select0(&self, i: usize) -> Option<usize> {
        let total_zeros = self.len - self.count_ones();
        if i >= total_zeros {
            return None;
        }

        // Binary search: find block where cumulative zeros >= i+1
        let mut lo = 0usize;
        let mut hi = self.data.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            let zeros_before = (mid + 1) * 64 - self.rank_index[mid + 1] as usize;
            if zeros_before <= i {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }

        let block = lo;
        let zeros_before_block = block * 64 - self.rank_index[block] as usize;
        let mut remaining = i - zeros_before_block;
        let mut word = !self.data[block];

        // Mask out bits beyond len
        let bits_in_block = if block * 64 + 64 <= self.len {
            64
        } else {
            self.len - block * 64
        };
        if bits_in_block < 64 {
            word &= (1u64 << bits_in_block) - 1;
        }

        loop {
            if word == 0 {
                return None;
            }
            let tz = word.trailing_zeros() as usize;
            if remaining == 0 {
                let pos = block * 64 + tz;
                return if pos < self.len { Some(pos) } else { None };
            }
            remaining -= 1;
            word &= word - 1;
        }
    }

    /// Total number of 1-bits.
    pub fn count_ones(&self) -> usize {
        *self.rank_index.last().unwrap_or(&0) as usize
    }

    /// Approximate heap memory used by this BitVec in bytes.
    ///
    /// Does not include the size of the struct itself (stack allocation),
    /// only the heap-allocated backing storage.
    pub fn heap_size_in_bytes(&self) -> usize {
        self.data.capacity() * std::mem::size_of::<u64>()
            + self.rank_index.capacity() * std::mem::size_of::<u32>()
    }
}

impl BitVec {
    /// Serialize this BitVec to a byte buffer.
    ///
    /// Format:
    /// - `len` as u64 LE (8 bytes)
    /// - `data.len()` as u64 LE (8 bytes)
    /// - `data` as raw u64 LE words
    /// - `rank_index` as raw u32 LE words (length is data.len() + 1)
    pub fn to_bytes_into(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&(self.len as u64).to_le_bytes());
        out.extend_from_slice(&(self.data.len() as u64).to_le_bytes());
        for &word in &self.data {
            out.extend_from_slice(&word.to_le_bytes());
        }
        for &r in &self.rank_index {
            out.extend_from_slice(&r.to_le_bytes());
        }
    }

    /// Deserialize a BitVec from a byte slice, returning (bitvec, bytes_consumed).
    ///
    /// Returns `None` if the data is too short or malformed.
    pub fn from_bytes(data: &[u8]) -> Option<(Self, usize)> {
        if data.len() < 16 {
            return None;
        }
        let len = u64::from_le_bytes(data[0..8].try_into().ok()?) as usize;
        let data_len = u64::from_le_bytes(data[8..16].try_into().ok()?) as usize;

        let words_bytes = data_len * 8;
        let rank_count = data_len + 1;
        let rank_bytes = rank_count * 4;
        let total = 16 + words_bytes + rank_bytes;

        if data.len() < total {
            return None;
        }

        let mut offset = 16;
        let mut words = Vec::with_capacity(data_len);
        for _ in 0..data_len {
            words.push(u64::from_le_bytes(
                data[offset..offset + 8].try_into().ok()?,
            ));
            offset += 8;
        }

        let mut rank_index = Vec::with_capacity(rank_count);
        for _ in 0..rank_count {
            rank_index.push(u32::from_le_bytes(
                data[offset..offset + 4].try_into().ok()?,
            ));
            offset += 4;
        }

        Some((
            BitVec {
                data: words,
                len,
                rank_index,
            },
            total,
        ))
    }
}

impl Default for BitVec {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_bitvec() {
        let bv = BitVec::new();
        assert_eq!(bv.len(), 0);
        assert_eq!(bv.count_ones(), 0);
        assert_eq!(bv.rank1(0), 0);
    }

    #[test]
    fn from_bits_basic() {
        let bv = BitVec::from_bits(&[true, false, true, true, false]);
        assert_eq!(bv.len(), 5);
        assert!(bv.get(0));
        assert!(!bv.get(1));
        assert!(bv.get(2));
        assert!(bv.get(3));
        assert!(!bv.get(4));
        assert_eq!(bv.count_ones(), 3);
    }

    #[test]
    fn rank1_basic() {
        let bv = BitVec::from_bits(&[true, false, true, true, false, true]);
        assert_eq!(bv.rank1(0), 0);
        assert_eq!(bv.rank1(1), 1);
        assert_eq!(bv.rank1(2), 1);
        assert_eq!(bv.rank1(3), 2);
        assert_eq!(bv.rank1(4), 3);
        assert_eq!(bv.rank1(5), 3);
        assert_eq!(bv.rank1(6), 4);
    }

    #[test]
    fn rank0_basic() {
        let bv = BitVec::from_bits(&[true, false, true, true, false, true]);
        assert_eq!(bv.rank0(0), 0);
        assert_eq!(bv.rank0(1), 0);
        assert_eq!(bv.rank0(2), 1);
        assert_eq!(bv.rank0(5), 2);
    }

    #[test]
    fn select1_basic() {
        let bv = BitVec::from_bits(&[true, false, true, true, false, true]);
        assert_eq!(bv.select1(0), Some(0));
        assert_eq!(bv.select1(1), Some(2));
        assert_eq!(bv.select1(2), Some(3));
        assert_eq!(bv.select1(3), Some(5));
        assert_eq!(bv.select1(4), None);
    }

    #[test]
    fn select0_basic() {
        let bv = BitVec::from_bits(&[true, false, true, true, false, true]);
        assert_eq!(bv.select0(0), Some(1));
        assert_eq!(bv.select0(1), Some(4));
        assert_eq!(bv.select0(2), None);
    }

    #[test]
    fn large_bitvec() {
        let mut bits = vec![false; 200];
        bits[0] = true;
        bits[63] = true;
        bits[64] = true;
        bits[127] = true;
        bits[199] = true;
        let bv = BitVec::from_bits(&bits);

        assert_eq!(bv.count_ones(), 5);
        assert_eq!(bv.rank1(64), 2);
        assert_eq!(bv.rank1(65), 3);
        assert_eq!(bv.select1(2), Some(64));
        assert_eq!(bv.select1(4), Some(199));
    }
}
