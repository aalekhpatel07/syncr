use crate::{ChecksumConfig, Checksums};




/// The weak rolling checksum is used in the 
/// [rsync algorithm] to quickly verify whether two
/// blocks of data are the same. 
/// 
/// Note: It is not guaranteed if the blocks are the same, 
/// but it is a good heuristic for a first-pass. Only those code blocks
/// that pass the weak checksum are then compared using the
/// strong checksum.
/// 
/// [Rsync Algorithm]: https://www.andrew.cmu.edu/course/15-749/READINGS/required/cas/tridgell96.pdf
#[derive(Debug, Copy, Clone)]
pub struct WeakCheckSum {
    /// The modulus to use for the checksum.
    /// This is typically 2^16.
    modulus: u32,
    /// The size of each chunk to calculate a running checksum for (i.e. window size).
    block_size: usize,
}

#[derive(Debug)]
pub struct WeakCheckSumBuilder {
    modulus: Option<u32>,
    block_size: Option<usize>,
}

impl WeakCheckSumBuilder {
    pub fn new() -> Self {
        Self {
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

    pub fn build(self) -> WeakCheckSum {
        WeakCheckSum {
            modulus: self.modulus.unwrap_or(1 << 16),
            block_size: self.block_size.unwrap_or(1000),
        }
    }
}

impl Default for WeakCheckSum {
    fn default() -> Self {
        Self {
            modulus: 1 << 16,
            block_size: 1000,
        }
    }
}

impl WeakCheckSum {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_config(config: &ChecksumConfig) -> Self {
        Self {
            block_size: config.block_size,
            modulus: config.modulus
        }
    }

    pub fn a_expanded(modulus: u32, left: usize, right: usize, buffer: &[u8]) -> u32 {
        if right >= buffer.len() {
            return 0;
        }
        let mut sum: u32 = 0;
        for i in left..=right {
            let summand = buffer[i] as u32;
            sum = (sum + summand) % modulus
        }
        sum % modulus
    }

    pub fn b_expanded(modulus: u32, left: usize, right: usize, buffer: &[u8]) -> u32 {
        if right >= buffer.len() {
            return 0;
        }
        let mut sum = 0;
        for i in left..=right {
            let summand = (buffer[i] as u32) * (right - i + 1) as u32;
            sum = (sum + summand) % modulus;
        }
        sum % modulus
    }

}

impl Checksums for WeakCheckSum {
    type Output = u32;

    fn checksums<'buf>(&self, buffer: &'buf [u8]) -> Box<dyn Iterator<Item=Self::Output> + 'buf > {
        let block_size = self.block_size;
        if buffer.len() == 0 {
            return Box::new(
                WeakCheckSumRollingIterator {
                    buffer,
                    modulus: self.modulus as isize,
                    previous_k: 0,
                    previous_l: 0,
                    a_k_l: 0,
                    b_k_l: 0,
                    ended: true,
                }
            );
        }
        if buffer.len() < block_size {
            return Box::new(
                WeakCheckSumRollingIterator {
                    buffer,
                    modulus: self.modulus as isize,
                    previous_k: 0,
                    previous_l: buffer.len() - 1,
                    a_k_l: WeakCheckSum::a_expanded(self.modulus, 0, buffer.len() - 1, buffer) as isize,
                    b_k_l: WeakCheckSum::b_expanded(self.modulus, 0, buffer.len() - 1, buffer) as isize,
                    ended: false,
                }
            );
        }
        Box::new(
            WeakCheckSumRollingIterator {
                buffer,
                modulus: self.modulus as isize,
                previous_k: 0,
                previous_l: block_size - 1,
                a_k_l: WeakCheckSum::a_expanded(self.modulus, 0, block_size - 1, buffer) as isize,
                b_k_l: WeakCheckSum::b_expanded(self.modulus, 0, block_size - 1, buffer) as isize,
                ended: false,
            }
        )
    }

    fn checksums_non_overlapping<'buf>(&self, data: &'buf [u8]) -> Box<dyn Iterator<Item=Self::Output> + 'buf> {
        let iterator = self.checksums(data).step_by(self.block_size);
        if data.len() % self.block_size != 0 {
            let last_chunk_start_index = data.len() - (data.len() % self.block_size);
            let last_chunk = data[last_chunk_start_index..].as_ref();
            return Box::new(
                iterator
                .chain(self.checksums(last_chunk))
            );
        }
        Box::new(
            iterator
        )
    }
}

#[derive(Debug)]
pub struct WeakCheckSumRollingIterator<'buf> {
    buffer: &'buf [u8],
    modulus: isize,
    previous_k: usize,
    previous_l: usize,
    a_k_l: isize,
    b_k_l: isize,
    ended: bool,
}

#[derive(Debug)]
pub struct WeakChecksumNonOverlappingIterator<'buf> {
    buffer: &'buf [u8],
    modulus: isize,
    left: usize,
    right: usize,
    window_size: usize,
}

impl<'buf> Iterator for WeakChecksumNonOverlappingIterator<'buf> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.right > self.buffer.len() {
            return None;
        }
        let checksum = 
            WeakCheckSum::a_expanded(self.modulus as u32, self.left, self.right, self.buffer) as u32    
            + (WeakCheckSum::b_expanded(self.modulus as u32, self.left, self.right, self.buffer) as u32) << 16;
        
        self.left += self.window_size;
        self.right += self.window_size;
        Some(checksum)
    }
}


impl<'buf> Iterator for WeakCheckSumRollingIterator<'buf> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ended {
            return None;
        }
        if self.previous_l + 1 >= self.buffer.len() || self.previous_k >= self.buffer.len() {
            let checksum = self.a_k_l + (self.b_k_l << 16);
            self.ended = true;
            return Some(checksum as u32);
        }
        let new_a_k_l = (
            self.a_k_l 
            + (self.buffer[self.previous_l + 1] as isize)
            - (self.buffer[self.previous_k] as isize)
        ).rem_euclid(self.modulus);

        let new_b_k_l = (
            self.b_k_l 
            + new_a_k_l 
            - (
                (self.previous_l as isize - self.previous_k as isize + 1) * (self.buffer[self.previous_k] as isize)
            )
        ).rem_euclid(self.modulus);

        self.previous_k += 1;
        self.previous_l += 1;

        let checksum = self.a_k_l + (self.b_k_l << 16);

        self.a_k_l = new_a_k_l.rem_euclid(self.modulus);
        self.b_k_l = new_b_k_l.rem_euclid(self.modulus);

        Some(checksum as u32)
    }
}


/// Utility function to compute the rolling weak checksum of a buffer
/// meant for a direct public API.
pub fn rolling_checksum(buffer: &'static [u8]) -> Vec<u32> {
    let rolling_checksum = WeakCheckSum::new();
    rolling_checksum.checksums(buffer).collect()
}



#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn rolling_checksum_of_buffer() {
        let buffer = [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09];
        let rolling_checksum = WeakCheckSum::new();
        assert_eq!(WeakCheckSum::a_expanded(rolling_checksum.modulus, 0, 1, &buffer), 1);
        assert_eq!(WeakCheckSum::a_expanded(rolling_checksum.modulus, 0, 2, &buffer), 3);
    }

    #[test]
    fn rolling_checksum_of_empty_buffer_does_not_exist() {
        let buffer: [u8; 0] = [];
        let rolling_checksum = WeakCheckSum::new();
        assert_eq!(rolling_checksum.checksums(&buffer).count(), 0);
    }

    proptest! {

        #[test]
        fn rolling_checksum_of_buffer_is_an_iterator(buffer in prop::collection::vec(0u8..=255, 0..=10000)) {
            let rolling_checksum = WeakCheckSum::new();
            rolling_checksum
            .checksums(&buffer)
            .for_each(drop);
        }

        #[test]
        fn rolling_checksum_has_one_entry_for_smol_block(buffer in prop::collection::vec(0u8..=255, 1..=900)) {
            let rolling_checksum = WeakCheckSum::new();
            prop_assert_eq!(rolling_checksum.checksums(&buffer).count(), 1);
        }

        #[test]
        fn rolling_checksum_both_implementation_give_same_result(buffer in prop::collection::vec(0u8..=255, 0..=10000)) {
            let rolling_checksum = WeakCheckSum::new();
            let block_size = rolling_checksum.block_size;
            let mut rolling_checksum_iterator_forward = rolling_checksum.checksums(&buffer);
            for idx in 0..buffer.len() {
                if idx as isize > buffer.len() as isize - block_size as isize {
                    break;
                }
                let expected_value = WeakCheckSum::a_expanded(rolling_checksum.modulus, idx, idx + block_size - 1, &buffer) + (WeakCheckSum::b_expanded(rolling_checksum.modulus, idx, idx + block_size - 1, &buffer) << 16);
                let iterator_forward_next = rolling_checksum_iterator_forward.next();

                prop_assert_eq!(
                    Some(expected_value), 
                    iterator_forward_next,
                    "idx: {}, buffer_len: {}", 
                    idx,
                    buffer.len()
                );
            }
        }
    }
}
