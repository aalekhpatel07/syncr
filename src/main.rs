use std::path::{PathBuf, Path};
use std::sync::{Mutex, Arc};
use std::time::Duration;

use tokio::net::TcpStream;
use syncr::network::*;
use tracing::{info, debug};
use tracing_subscriber;
use syncr::{
    Checksums,
    CheckSum
};
use clap::Parser;


// #[async_trait]
// pub trait Client {
//     async fn process(&mut self, connection: &mut Connection) -> crate::Result<()>;
// }

#[derive(Debug, Parser)]
pub struct Cli {
    #[clap(short, long, default_value = "test.txt")]
    pub file: String,
    #[clap(short, long, default_value = "test-remote.txt")]
    pub remote_file: String,
    #[clap(short, long, default_value_t = 8000)]
    pub port: u16,
}


// #[derive(Debug)]
// pub struct State {
//     pub file: PathBuf,
//     pub checksum: CheckSum,
//     pub weak_checksums: Vec<u32>,
//     pub remote_file: PathBuf,
// }


// impl State {
//     pub fn new<P>(file: P, remote_file: &str) -> Self 
//     where
//         P: AsRef<Path>,
//     {
//         let checksum = CheckSum::default();

//         Self {
//             file: PathBuf::from(file.as_ref()), 
//             weak_checksums: vec![],
//             checksum,
//             remote_file: PathBuf::from(remote_file),
//         }
//     }

//     pub fn read_weak_checksums(&mut self) -> crate::Result<()> {
//         let data = std::fs::read(&self.file)?;
//         self.weak_checksums = self.checksum.weak.checksums(&data).collect();
//         Ok(())
//     }
// }

// pub type SharedState = std::sync::Arc<std::sync::Mutex<State>>;

// #[async_trait]
// impl Client for State {
//     async fn process(&mut self, connection: &mut Connection) -> crate::Result<()> {
//         // Send the file name
//         // First send the file name and then a BOM: (0xefbbff)
//         let file_name = self.remote_file.to_str().unwrap().as_bytes().to_vec();
        
//         connection.writer.write_all(&file_name).await?;
//         debug!("Sending file name: (size: {}) {:?}", file_name.len(), file_name);

//         connection.writer.write_all(vec![0xef, 0xbb, 0xbf].as_slice()).await?;
//         debug!("Sending BOM");

//         let mut buffer = vec![];
//         self.read_weak_checksums()?;
//         for num in self.weak_checksums.iter() {
//             buffer.extend_from_slice(&num.to_le_bytes());
//         }
//         connection.writer.write_all(&buffer).await?;
//         debug!("Sending weak checksums: (size: {}) {:?}", buffer.len(), buffer);

//         buffer = vec![0xef, 0xbb, 0xbf];
//         debug!("Sending BOM");
//         connection.writer.write_all(&buffer).await?;

//         Ok(())
//     }
// }


#[derive(Debug)]
pub struct ConnectionState {
    pub checksum: CheckSum,
    pub weak_checksums: Vec<u32>,
    pub file_path: String,
    pub remote_file_path: String,
    pub own_data: Vec<u8>,
}


#[derive(Debug)]
pub struct SingleConnection {
    pub outbound_msg_tx: tokio::sync::mpsc::Sender<Message>,
    pub inbound_msg_rx: tokio::sync::mpsc::Receiver<Message>,
    pub state: Arc<Mutex<ConnectionState>>,
}

impl SingleConnection {

    pub fn build_filename_msg(&self) -> Message {
        let state = self.state.lock().unwrap();
        Message::FileName(state.remote_file_path.clone())
    }
    pub async fn send_filename_msg(&mut self) -> syncr::Result<()> {
        self.outbound_msg_tx.send(self.build_filename_msg()).await?;
        Ok(())
    }

    pub fn compute_our_checksums(&mut self) -> syncr::Result<()> {
        let mut state = self.state.lock().unwrap();
        state.own_data = std::fs::read(&state.file_path)?;
        state.weak_checksums = state.checksum.weak.checksums(&state.own_data).collect();
        Ok(())
    }
    pub async fn send_weak_checksums_msg(&mut self) -> syncr::Result<()> {
        self.compute_our_checksums()?;
        let state = self.state.lock().unwrap();
        let msg = Message::WeakChecksums(state.weak_checksums.clone());
        drop(state);
        self.outbound_msg_tx.send(msg).await?;
        Ok(())
    }

    // TODO: Determine the block and in the order they should be recreated.
    pub fn given_indices_issue_list_of_instructions(&self, indices: &[(usize, usize)]) -> Vec<Instruction>{
        // The first index is the byte offset in the recipient that matches to the byte offset (the second index) in our file (the sender).

        vec![]
    }

    pub async fn run(mut self) -> syncr::Result<()> {
        let _ = self.send_filename_msg().await;
        tokio::time::sleep(Duration::from_micros(10)).await;
        let _ = self.send_weak_checksums_msg().await;

        while let Some(msg) = self.inbound_msg_rx.recv().await {
            match msg {
                Message::StrongChecksumRequest(strong_checksum_indices) => {
                    let state = self.state.lock().unwrap();
                    let mut result = vec![];
                    for idx in strong_checksum_indices {
                        result.push((idx, state.checksum.strong.checksum_for_block(idx, &state.own_data)));
                    }
                    self.outbound_msg_tx.send(Message::StrongChecksums(result)).await?;
                },
                Message::Matches(matches) => {
                    debug!("Recipient match info: {:?}", matches);
                    // Once the recipient has sent us the blocks that it already
                    // has, we can determine the blocks that we need to send.
                    
                    // We can then send the blocks in the order that they should be
                    // recreated by the recipient.

                    let instructions = self.given_indices_issue_list_of_instructions(&matches);
                    self.outbound_msg_tx.send(Message::Instructions(instructions)).await?;
                },
                _ => {

                }
            }
        }

        Ok(())
    }
}


#[tokio::main]
pub async fn main() -> syncr::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();
    let stream = TcpStream::connect(format!("0.0.0.0:{}", cli.port)).await?;

    let (inbound_msg_tx, inbound_msg_rx) = tokio::sync::mpsc::channel(100);
    let (outbound_msg_tx, outbound_msg_rx) = tokio::sync::mpsc::channel(100);

    let connection = Connection::new(stream, inbound_msg_tx);
    let state: ConnectionState = ConnectionState { 
        checksum: CheckSum::default(), 
        weak_checksums: vec![],
        file_path: cli.file,
        remote_file_path: cli.remote_file,
        own_data: vec![]
    };

    let single_connection = SingleConnection {
        outbound_msg_tx,
        inbound_msg_rx,
        state: Arc::new(Mutex::new(state)),
    };

    let _ = tokio::join!(connection.run(outbound_msg_rx), single_connection.run());

    Ok(())
}
