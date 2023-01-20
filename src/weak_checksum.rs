use fxhash::FxHashMap as HashMap;

pub type CacheKey = (u8, usize, usize);

pub struct RollingCheckSum<'buf> {
    buffer: &'buf [u8],
    modulus: u32,
    block_size: usize,
    cache: HashMap<CacheKey, u32>,
}

#[derive(Debug)]
pub struct RollingCheckSumBuilder<'buf> {
    buffer: &'buf [u8],
    modulus: Option<u32>,
    block_size: Option<usize>,
}

impl<'buf> RollingCheckSumBuilder<'buf> {
    pub fn new(buffer: &'buf [u8]) -> Self {
        Self {
            buffer,
            modulus: None,
            block_size: None,
        }
    }

    pub fn modulus(mut self, modulus: u32) -> Self {
        self.modulus = Some(modulus);
        self
    }

    pub fn block_size(mut self, block_size: usize) -> Self {
        self.block_size = Some(block_size);
        self
    }

    pub fn build(self) -> RollingCheckSum<'buf> {
        RollingCheckSum {
            buffer: self.buffer,
            modulus: self.modulus.unwrap_or(1 << 16),
            block_size: self.block_size.unwrap_or(1000),
            cache: HashMap::default(),
        }
    }
}

fn build_key(is_function_a: bool, left: usize, right: usize) -> CacheKey {
    (is_function_a as u8, left, right)
}

impl<'buf> RollingCheckSum<'buf> {
    pub fn new(buffer: &'buf [u8]) -> Self {
        Self {
            buffer,
            modulus: 1 << 16,
            block_size: 1000,
            cache: HashMap::default(),
        }
    }

    #[inline(always)]
    fn a_expanded(&mut self, left: usize, right: usize) -> u32 {
        if self.cache.contains_key(&build_key(true, left, right)) {
            return *self.cache.get(&build_key(true, left, right)).unwrap();
        }
        let mut sum: u32 = 0;
        for i in left..=right {
            let summand = self.buffer[i] as u32;
            sum = (sum + summand) % self.modulus
        }
        let result = sum % self.modulus;
        self.cache
            .entry(build_key(true, left, right))
            .or_insert(result);
        result
    }

    #[inline(always)]
    fn b_expanded(&mut self, left: usize, right: usize) -> u32 {
        if self.cache.contains_key(&build_key(false, left, right)) {
            return *self.cache.get(&build_key(false, left, right)).unwrap();
        }
        let mut sum = 0;
        for i in left..=right {
            let summand = (self.buffer[i] as u32) * (right - i + 1) as u32;
            sum = (sum + summand) % self.modulus;
        }
        let result = sum % self.modulus;
        self.cache
            .entry(build_key(false, left, right))
            .or_insert(result);
        result
    }

    fn a_recurrence(&mut self, left: usize, right: usize) -> u32 {
        if self.cache.contains_key(&build_key(true, left, right)) {
            return *self.cache.get(&build_key(true, left, right)).unwrap();
        }
        if left == 0 {
            self.a_expanded(left, right)
        } else {
            let result = (self.a_recurrence(left - 1, right - 1) + self.buffer[right] as u32
                - self.buffer[left - 1] as u32)
                % self.modulus;
            self.cache
                .entry(build_key(true, left, right))
                .or_insert(result);
            result
        }
    }

    fn b_recurrence(&mut self, left: usize, right: usize) -> u32 {
        if self.cache.contains_key(&build_key(false, left, right)) {
            return *self.cache.get(&build_key(false, left, right)).unwrap();
        }

        if left == 0 {
            self.b_expanded(left, right) % self.modulus
        } else {
            let first_term = self.b_recurrence(left - 1, right - 1) % self.modulus;
            let middle_term =
                (self.buffer[left - 1] as u32 * (right - left + 1) as u32) % self.modulus;
            let last_term = self.a_recurrence(left, right) % self.modulus;

            let res = ((first_term as i64 - middle_term as i64 + last_term as i64)
                % (self.modulus as i64)) as u32;
            let result = res % self.modulus;
            self.cache
                .entry(build_key(false, left, right))
                .or_insert(result);

            result
        }
    }

    #[inline(always)]
    pub fn checksum(&mut self, left: usize, right: usize) -> u32 {
        (self.a_recurrence(left, right) << 16) + self.b_recurrence(left, right)
    }

    pub fn rolling_checksums<'roll>(&'roll mut self) -> RollingCheckSumIterator<'roll, 'buf>
    where
        'buf: 'roll,
    {
        let block_size = self.block_size;
        RollingCheckSumIterator {
            rolling_checksum: self,
            left: 0,
            right: block_size,
        }
    }
}

pub struct RollingCheckSumIterator<'roll, 'buf> {
    rolling_checksum: &'roll mut RollingCheckSum<'buf>,
    left: usize,
    right: usize,
}

impl<'roll, 'buff> Iterator for RollingCheckSumIterator<'roll, 'buff>
where
    'buff: 'roll,
{
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.right < self.rolling_checksum.buffer.len() {
            let checksum = self.rolling_checksum.checksum(self.left, self.right);
            self.left += 1;
            self.right += 1;
            Some(checksum)
        } else {
            None
        }
    }
}

/// Utility function to compute the rolling weak checksum of a buffer
/// meant for a direct public API.
pub fn rolling_checksum(buffer: &[u8]) -> Vec<u32> {
    let mut rolling_checksum = RollingCheckSum::new(buffer);
    rolling_checksum.rolling_checksums().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn rolling_checksum_of_buffer() {
        let buffer = [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09];
        let mut rolling_checksum = RollingCheckSum::new(&buffer);
        assert_eq!(rolling_checksum.a_expanded(0, 1), 1);
        assert_eq!(rolling_checksum.a_expanded(0, 2), 3);
    }

    proptest! {
        #[test]
        fn rolling_checksum_a_of_buffer_is_same_as_expanded_checksum_a(buffer in prop::collection::vec(0u8..=255, 0..=100)) {
            let mut rolling_checksum = RollingCheckSumBuilder::new(&buffer).block_size(10).build();
            for i in 0..buffer.len() {
                for j in i..buffer.len() {
                    prop_assert_eq!(rolling_checksum.a_expanded(i, j), rolling_checksum.a_recurrence(i, j));
                }
            }
        }

        #[test]
        fn rolling_checksum_b_of_buffer_is_same_as_expanded_checksum_b(buffer in prop::collection::vec(0u8..=255, 0..=100)) {
            let mut rolling_checksum = RollingCheckSumBuilder::new(&buffer).block_size(10).build();
            for i in 0..buffer.len() {
                for j in i..buffer.len() {
                    prop_assert_eq!(rolling_checksum.b_expanded(i, j), rolling_checksum.b_recurrence(i, j));
                }
            }
        }

        #[test]
        fn rolling_checksum_of_buffer_is_same_as_expanded_checksum(buffer in prop::collection::vec(0u8..=255, 0..=100)) {
            let mut rolling_checksum = RollingCheckSumBuilder::new(&buffer).block_size(10).build();
            for i in 0..buffer.len() {
                for j in i..buffer.len() {
                    prop_assert_eq!(rolling_checksum.checksum(i, j), (rolling_checksum.a_expanded(i, j) << 16) + rolling_checksum.b_expanded(i, j));
                }
            }
        }

        #[test]
        fn rolling_checksum_of_buffer_is_an_iterator(buffer in prop::collection::vec(0u8..=255, 0..=100)) {
            let mut rolling_checksum = RollingCheckSum::new(&buffer);
            rolling_checksum
            .rolling_checksums()
            .for_each(|v| {
                assert!(v > 0);
                assert!(v < 1 << 16);
            });
        }
    }
}
