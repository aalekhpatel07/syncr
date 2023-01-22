//! Sender sends all the weak rolling checksums to the receiver.
//! Receiver computes weak non-overlapping checksums and if they match
//! it requests the sender to compute the strong checksums for the matching
//! blocks.
//! Sender computes the strong checksums and sends them to the receiver.
//! Receiver compares the strong checksums and if they match, the receiver
//! knows that it has blocks that are identical to the sender. It also knows
//! the matching offsets of the blocks from the sender's file. A run of matching
//! blocks can be coalesced into a single matching block.
//! Then the receiver requests the sender to send everything 
//! but the matching blocks.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::net::{
    TcpListener,
    TcpStream,
};
use syncr::{network::*, CheckSum, Checksums, strong_checksum::hash as strong_hash, multisearch::weak_hash};
use tracing::{info, error};
use tracing_subscriber;


pub async fn handle_stream(
    stream: TcpStream,
    peer: SocketAddr,
    inbound_msg_tx: tokio::sync::mpsc::Sender<Message>,
    outbound_msg_rx: tokio::sync::mpsc::Receiver<Message>
) -> syncr::Result<()> {

    info!("Accepted connection from syncr client: {}", peer);
    let connection = Connection::new(stream, inbound_msg_tx);
    connection.run(outbound_msg_rx).await
}


#[derive(Debug, Default)]
pub struct ConnectionState {
    pub checksum: CheckSum,
    pub weak_checksums: Vec<u32>,
    pub file_path: String,
    pub remote_weak_checksums: Vec<u32>,
    pub hash_table: HashMap<u16, HashMap<u32, Vec<usize>>>,
    pub own_data: Vec<u8>,
}


#[derive(Debug)]
pub struct SingleConnection {
    pub outbound_message_tx: tokio::sync::mpsc::Sender<Message>,
    pub inbound_msg_rx: tokio::sync::mpsc::Receiver<Message>,
    pub state: Arc<Mutex<ConnectionState>>,
}

impl SingleConnection {

    pub fn set_file_path(&mut self, path: String) {
        let mut state = self.state.lock().unwrap();
        state.file_path = path;
    }
    pub fn set_remote_weak_checksums(&mut self, weak_checksums: Vec<u32>) {
        let mut state = self.state.lock().unwrap();
        state.remote_weak_checksums = weak_checksums;
    }

    pub fn compute_our_checksums(&mut self) -> syncr::Result<()> {
        let mut state = self.state.lock().unwrap();
        state.own_data = std::fs::read(&state.file_path)?;
        state.weak_checksums = state.checksum.weak.checksums_non_overlapping(&state.own_data).collect();

        Ok(())
    }

    pub fn compile_hash_table(&self) {

        let mut hash_table = HashMap::new();
        let mut state = self.state.lock().unwrap();

        for (offset, &checksum) in state.weak_checksums.iter().enumerate() {
            let checksum_hash: u16 = weak_hash(checksum);

            hash_table
            .entry(checksum_hash)
            .and_modify(|m: &mut HashMap<u32, Vec<usize>>| {
                m
                .entry(checksum)
                .and_modify(|strong_hashes| {
                    strong_hashes.push(offset * state.checksum.strong.block_size);
                })
                .or_insert(vec![offset * state.checksum.strong.block_size]);
                
            })
            .or_insert_with(|| HashMap::from_iter([(checksum, vec![offset * state.checksum.strong.block_size])]));
        }
        state.hash_table = hash_table;
    }

    pub fn build_strong_hash_request(&self) -> syncr::Result<Message> {
        let mut matches = Vec::new();
    
        let state = self.state.lock().unwrap();

        for (byte_offset, weak) in state.remote_weak_checksums.iter().enumerate() {
            let weak_16_bit_hash = weak_hash(*weak);
            
            // First, check the 16-bit hash.
            if !state.hash_table.contains_key(&weak_16_bit_hash) {
                continue;
            }

            // Now, the 16-bit hash matches, so we need to check the 32-bit hash.
            let rolling_hash_map = state.hash_table.get(&weak_16_bit_hash).unwrap();
            if !rolling_hash_map.contains_key(&weak) {
                continue;
            }

            // // Now, the 32-bit hash matches, so we need to check the 128-bit hash.
            // Add this to the list of indices we want a strong checksum for.
            matches.push(byte_offset);
        }

        Ok(Message::StrongChecksumRequest(matches))

    }

    pub fn process_strong_hash_response(&self, strong_checksums: Vec<(usize, u128)>) -> Vec<(usize, usize)> {
        let state = self.state.lock().unwrap();
        let mut matches = Vec::new();
        for (remote_offset, strong) in strong_checksums {
            for &our_offset in state.hash_table.get(&weak_hash(state.remote_weak_checksums[remote_offset])).unwrap().get(&state.remote_weak_checksums[remote_offset]).unwrap().iter() {
                let our_strong = strong_hash(&state.own_data[our_offset..our_offset + state.checksum.strong.block_size]);
                if our_strong == strong {
                    // Our non-overlapping block matches with some block of the sender.
                    // So sender may ne able to use a reference to this block
                    // instead of sending it.
                    matches.push((our_offset, remote_offset));
                }
            }
        }
        matches
    }

    pub async fn run(mut self) -> syncr::Result<()> {
        while let Some(msg) = self.inbound_msg_rx.recv().await {
            match msg {
                Message::FileName(path) => {
                    self.set_file_path(path);
                },
                Message::WeakChecksums(weak_checksums) => {
                    self.set_remote_weak_checksums(weak_checksums);
                    self.compute_our_checksums()?;
                    self.compile_hash_table();
                    let strong_hash_request = self.build_strong_hash_request()?;
                    self.outbound_message_tx.send(strong_hash_request).await?;
                },
                Message::StrongChecksums(strong_checksums) => {
                    let matches = self.process_strong_hash_response(strong_checksums);
                    self.outbound_message_tx.send(Message::Matches(matches)).await?;
                },
                _ => {}
            }
        }
        Ok(())
    }
}

#[tokio::main]
pub async fn main() -> syncr::Result<()> {
    tracing_subscriber::fmt::init();
    let listener = TcpListener::bind("0.0.0.0:8000").await?;

    loop {
        let (stream, peer) = listener.accept().await?;
        
        let (outbound_message_tx, outbound_msg_rx) = tokio::sync::mpsc::channel(100);
        let (inbound_msg_tx, inbound_msg_rx) = tokio::sync::mpsc::channel(100);
        let handle1 = tokio::spawn(async move {
            match handle_stream(stream, peer, inbound_msg_tx, outbound_msg_rx).await {
                Ok(_) => info!("Stream handled successfully."),
                Err(e) => error!("Connection error: {}", e),
            };
        });

        let state: ConnectionState = ConnectionState::default();
        
        let single_connection = SingleConnection {
            outbound_message_tx,
            inbound_msg_rx,
            state: Arc::new(Mutex::new(state)),
        };

        let handle2 = tokio::spawn(async move {
            match single_connection.run().await {
                Ok(_) => info!("Single connection ran successfully."),
                Err(e) => error!("Connection error: {}", e),
            };
        });

        let _ = tokio::join!(handle1, handle2);
    }
}