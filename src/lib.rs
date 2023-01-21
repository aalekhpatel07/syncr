use itertools::Itertools;
use strong_checksum::StrongCheckSum;
use weak_checksum::WeakCheckSum;

pub mod weak_checksum;
pub mod multisearch;
pub mod strong_checksum;

#[derive(Debug)]
pub struct ChecksumConfig {
    pub block_size: usize,
    pub modulus: u32,
}

impl Default for ChecksumConfig {
    fn default() -> Self {
        Self {
            block_size: 1000,
            modulus: 1 << 16,
        }
    }
}

#[derive(Debug, Default)]
pub struct CheckSum {
    pub weak: WeakCheckSum,
    pub strong: StrongCheckSum
}


impl CheckSum {
    pub fn new() -> Self {
        Self {
            weak: WeakCheckSum::new(),
            strong: StrongCheckSum::new(),
        }
    }

    pub fn with_config(config: &ChecksumConfig) -> Self {
        Self {
            weak: WeakCheckSum::with_config(config),
            strong: StrongCheckSum::with_config(config),
        }
    }

    // pub fn checksums (
    //     &self,
    //     data: &'buf [u8]
    // ) -> impl Iterator<Item=(u32, u128)> + '_
    // {
    //     let weak_iter = self.weak.checksums(data);
    //     let strong_iter = self.strong.checksums(data);
    //     weak_iter.zip(strong_iter)
    // }

}

impl Checksums for CheckSum {
    type Output = (u32, u128);
    fn checksums<'buf>(&self, data: &'buf [u8]) -> Box<dyn Iterator<Item=Self::Output> + 'buf> {
        let weak_iter = self.weak.checksums(data);
        let strong_iter = self.strong.checksums(data);
        Box::new(weak_iter.zip(strong_iter))
    }

    fn checksums_non_overlapping<'buf>(&self, data: &'buf [u8]) -> Box<dyn Iterator<Item=Self::Output> + 'buf> {
        let weak_iter = self.weak.checksums_non_overlapping(data);
        let strong_iter = self.strong.checksums_non_overlapping(data);
        Box::new(weak_iter.zip(strong_iter))
    }
}


pub trait Checksums {
    /// The output type for each item in the stream of checksums.
    type Output;
    /// Returns a rolling iterator over the checksums of the data.
    fn checksums<'buf>(&self, data: &'buf [u8]) -> Box<dyn Iterator<Item=Self::Output> + 'buf>;
    /// Returns a non-overlapping iterator over the checksums of the data.
    fn checksums_non_overlapping<'buf>(&self, data: &'buf [u8]) -> Box<dyn Iterator<Item=Self::Output> + 'buf>;
}