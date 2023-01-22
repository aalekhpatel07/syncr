use itertools::Itertools;
use network::Message;
use strong_checksum::StrongCheckSum;
use weak_checksum::WeakCheckSum;

pub mod weak_checksum;
pub mod multisearch;
pub mod strong_checksum;
pub mod network;
use thiserror::Error;


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

#[derive(Debug, Default, Copy, Clone)]
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


pub type Result<T> = std::result::Result<T, SyncrError>;


#[derive(Error, Debug)]
pub enum SyncrError {
    #[error("Client disconnected unexpectedly while sending weak checksums.")]
    UnexpectedEndOfFileWhileReadingWeakChecksums,
    #[error("Client disconnected unexpectedly while sending file name.")]
    UnexpectedEndOfFileWhileReadingFileName,
    #[error("Underlying IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Error while parsing file name: {0}")]
    InvalidFileName(#[from] std::str::Utf8Error),
    #[error("Serialization Error: {0}")]
    SerializationError(#[from] rmp_serde::encode::Error),
    #[error("Deserialization Error: {0}")]
    DeserializationError(#[from] rmp_serde::decode::Error),
    #[error("Connection reset by peer.")]
    ConnectionResetByPeer,
    #[error("SendError: {0}")]
    SyncMpScError(#[from] tokio::sync::mpsc::error::SendError<Message>),
}