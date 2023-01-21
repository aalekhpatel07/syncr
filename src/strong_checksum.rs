
#[derive(Debug)]
pub struct StrongCheckSum {
    block_size: usize,
}

impl Default for StrongCheckSum {
    fn default() -> Self {
        Self {
            block_size: 1000,
        }
    }
}

impl StrongCheckSum {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(config: &ChecksumConfig) -> Self {
        Self {
            block_size: config.block_size,
        }
    }
}

impl Checksums for StrongCheckSum {
    type Output = u128;
    fn checksums<'buf>(&self, data: &'buf [u8]) -> Box<dyn Iterator<Item=Self::Output> + 'buf> {
        if data.len() < self.block_size {
            return Box::new(
                StrongCheckSumIterator {
                    data,
                    left_index: 0,
                    right_index: data.len(),
                    shift: 1
                }
            );
        }
        Box::new(
            StrongCheckSumIterator {
                data,
                left_index: 0,
                right_index: self.block_size,
                shift: 1
            }
        )
    }

    fn checksums_non_overlapping<'buf>(&self, data: &'buf [u8]) -> Box<dyn Iterator<Item=Self::Output> + 'buf> {
        Box::new(
            StrongCheckSumIterator {
                data,
                left_index: 0,
                right_index: data.len().min(self.block_size),
                shift: data.len().min(self.block_size)
            }
        )
    }
}


#[derive(Debug)]
pub struct StrongCheckSumIterator<'buf> {
    data: &'buf [u8],
    left_index: usize,
    right_index: usize,
    shift: usize
}

impl<'buf> Iterator for StrongCheckSumIterator<'buf> {
    type Item = u128;

    fn next(&mut self) -> Option<Self::Item> {
        if self.right_index >= self.data.len() {
            return None;
        }
        let result = hash(&self.data[self.left_index..self.right_index]);
        self.left_index += self.shift;
        self.right_index += self.shift;
        Some(result)
    }
}


#[cfg(feature = "md4")]
mod _md4 {
    use md4::{Md4, Digest};

    pub fn hash(data: &[u8]) -> u128 {
        let mut hasher = Md4::new();
        hasher.update(data);
        let result: [u8; 16] = hasher.finalize().into();
        u128::from_le_bytes(result)
    }
}

#[cfg(feature = "md4")]
pub use _md4::*;

use crate::{ChecksumConfig, Checksums};


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strong_checksum() {
        let data = b"";
        let checksum = StrongCheckSum::new();
        let checksums: Vec<u128> = checksum.checksums_non_overlapping(data).collect();
        assert_eq!(checksums.len(), 0);
        // assert_eq!(checksums[0], 0x6d9f9b5a_0d5f9b5a_0d5f9b5a_0d5f9b5a);
    }

}