#[derive(Debug, Clone, PartialEq)]
pub struct PackedBitArray {
    bits: usize,
    capacity: usize,
    data: Vec<u64>,
}

impl PackedBitArray {
    pub fn new(bits: usize, capacity: usize) -> Self {
        let bits = bits.max(1);
        let total_bits = bits * capacity;
        let u64_count = (total_bits + 63) / 64;
        Self {
            bits,
            capacity,
            data: vec![0u64; u64_count],
        }
    }

    pub fn from_raw(bits: usize, capacity: usize, data: Vec<u64>) -> Self {
        Self {
            bits: bits.max(1),
            capacity,
            data,
        }
    }

    pub fn bits(&self) -> usize {
        self.bits
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn raw_data(&self) -> &[u64] {
        &self.data
    }

    pub fn raw_data_mut(&mut self) -> &mut [u64] {
        &mut self.data
    }

    pub fn get(&self, index: usize) -> u32 {
        if index >= self.capacity {
            return 0;
        }

        let start_bit = index * self.bits;
        let start_idx = start_bit / 64;
        let offset = start_bit % 64;
        let mask = if self.bits >= 64 { u64::MAX } else { (1u64 << self.bits) - 1 };

        if start_idx >= self.data.len() {
            return 0;
        }

        if offset + self.bits <= 64 {
            ((self.data[start_idx] >> offset) & mask) as u32
        } else {
            let bits1 = 64 - offset;
            let bits2 = self.bits - bits1;
            let part1 = self.data[start_idx] >> offset;
            let part2 = if start_idx + 1 < self.data.len() {
                (self.data[start_idx + 1] & ((1u64 << bits2) - 1)) << bits1
            } else {
                0
            };
            ((part1 | part2) & mask) as u32
        }
    }

    pub fn set(&mut self, index: usize, value: u32) {
        if index >= self.capacity {
            return;
        }

        let start_bit = index * self.bits;
        let start_idx = start_bit / 64;
        let offset = start_bit % 64;
        let mask = if self.bits >= 64 { u64::MAX } else { (1u64 << self.bits) - 1 };
        let val = (value as u64) & mask;

        if start_idx >= self.data.len() {
            return;
        }

        if offset + self.bits <= 64 {
            self.data[start_idx] &= !(mask << offset);
            self.data[start_idx] |= val << offset;
        } else {
            let bits1 = 64 - offset;
            let mask1 = (1u64 << bits1) - 1;
            self.data[start_idx] &= !(mask1 << offset);
            self.data[start_idx] |= (val & mask1) << offset;

            if start_idx + 1 < self.data.len() {
                let bits2 = self.bits - bits1;
                let mask2 = (1u64 << bits2) - 1;
                self.data[start_idx + 1] &= !mask2;
                self.data[start_idx + 1] |= (val >> bits1) & mask2;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packed_bit_array() {
        for bits in 2..=16 {
            let count = 1000;
            let mut arr = PackedBitArray::new(bits, count);
            let max_val = ((1u64 << bits) - 1) as u32;

            for i in 0..count {
                let val = ((i * 37) as u32) & max_val;
                arr.set(i, val);
            }

            for i in 0..count {
                let expected = ((i * 37) as u32) & max_val;
                assert_eq!(arr.get(i), expected, "Failed at index {} for bits {}", i, bits);
            }
        }
    }
}
