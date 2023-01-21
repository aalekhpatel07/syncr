
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
#[derive(Debug)]
pub struct RollingCheckSum {
    /// The modulus to use for the checksum.
    /// This is typically 2^16.
    modulus: u32,
    /// The size of each chunk to calculate a running checksum for (i.e. window size).
    block_size: usize,
}

#[derive(Debug)]
pub struct RollingCheckSumBuilder {
    modulus: Option<u32>,
    block_size: Option<usize>,
}

impl RollingCheckSumBuilder {
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

    pub fn build(self) -> RollingCheckSum {
        RollingCheckSum {
            modulus: self.modulus.unwrap_or(1 << 16),
            block_size: self.block_size.unwrap_or(1000),
        }
    }
}

impl RollingCheckSum {
    pub fn new() -> Self {
        Self {
            modulus: 1 << 16,
            block_size: 1000,
        }
    }

    fn a_expanded(&self, left: usize, right: usize, buffer: &[u8]) -> u32 {
        if right >= buffer.len() {
            return 0;
        }
        let mut sum: u32 = 0;
        for i in left..=right {
            let summand = buffer[i] as u32;
            sum = (sum + summand) % self.modulus
        }
        sum % self.modulus
    }

    fn b_expanded(&self, left: usize, right: usize, buffer: &[u8]) -> u32 {
        if right >= buffer.len() {
            return 0;
        }
        let mut sum = 0;
        for i in left..=right {
            let summand = (buffer[i] as u32) * (right - i + 1) as u32;
            sum = (sum + summand) % self.modulus;
        }
        sum % self.modulus
    }

    pub fn rolling_checksums<'buf>(&self, buffer: &'buf [u8]) -> RollingCheckSumIterator<'buf>
    {
        let block_size = self.block_size;
        RollingCheckSumIterator {
            buffer,
            modulus: self.modulus as isize,
            previous_k: 0,
            previous_l: block_size - 1,
            a_k_l: self.a_expanded(0, block_size - 1, buffer) as isize,
            b_k_l: self.b_expanded(0, block_size - 1, buffer) as isize,
            ended: false,
        }
    }
}

#[derive(Debug)]
pub struct RollingCheckSumIterator<'buf> {
    buffer: &'buf [u8],
    modulus: isize,
    previous_k: usize,
    previous_l: usize,
    a_k_l: isize,
    b_k_l: isize,
    ended: bool,
}


impl<'buf> Iterator for RollingCheckSumIterator<'buf> {
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
pub fn rolling_checksum(buffer: &[u8]) -> Vec<u32> {
    let rolling_checksum = RollingCheckSum::new();
    rolling_checksum.rolling_checksums(buffer).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn rolling_checksum_of_buffer() {
        let buffer = [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09];
        let rolling_checksum = RollingCheckSum::new();
        assert_eq!(rolling_checksum.a_expanded(0, 1, &buffer), 1);
        assert_eq!(rolling_checksum.a_expanded(0, 2, &buffer), 3);
    }

    proptest! {

        #[test]
        fn rolling_checksum_of_buffer_is_an_iterator(buffer in prop::collection::vec(0u8..=255, 0..=10000)) {
            let rolling_checksum = RollingCheckSum::new();
            rolling_checksum
            .rolling_checksums(&buffer)
            .for_each(drop);
        }

        #[test]
        fn rolling_checksum_both_implementation_give_same_result(buffer in prop::collection::vec(0u8..=255, 0..=10000)) {
            let rolling_checksum = RollingCheckSum::new();
            let block_size = rolling_checksum.block_size;
            let mut rolling_checksum_iterator_forward = rolling_checksum.rolling_checksums(&buffer);
            for idx in 0..buffer.len() {
                if idx as isize > buffer.len() as isize - block_size as isize {
                    break;
                }
                let expected_value = rolling_checksum.a_expanded(idx, idx + block_size - 1, &buffer) + (rolling_checksum.b_expanded(idx, idx + block_size - 1, &buffer) << 16);
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
