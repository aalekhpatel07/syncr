use tokio::{net::{tcp::{OwnedReadHalf, OwnedWriteHalf}, TcpStream}, io::{AsyncReadExt, AsyncWriteExt}};
use tokio::io::{BufWriter, BufReader};
use serde::{
    Serialize,
    Deserialize
};
use bytes::BytesMut;
use tracing::trace;

#[derive(Debug)]
pub struct Connection {
    // The `TcpStream`. It is decorated with a `BufWriter`, which provides write
    // level buffering. The `BufWriter` implementation provided by Tokio is
    // sufficient for our needs.
    pub inbound_message_tx: tokio::sync::mpsc::Sender<Message>,
    pub read_half: BufReader<OwnedReadHalf>,
    pub write_half: BufWriter<OwnedWriteHalf>,

    // // The buffer for reading frames.
    pub buffer: BytesMut,
}

impl Connection {
    pub fn new(
        stream: TcpStream,
        inbound_message_tx: tokio::sync::mpsc::Sender<Message>,
    ) -> Self {
        let (read_half, write_half) = stream.into_split();
        Self {
            inbound_message_tx,
            read_half: BufReader::new(read_half),
            write_half: BufWriter::new(write_half),
            buffer: BytesMut::with_capacity(4 * 1024),
        }
    }

    pub async fn send(&mut self, message: Message) -> crate::Result<()> {
        let serialized = rmp_serde::to_vec(&message)?;
        self.write_half.write_all(&serialized).await?;
        self.write_half.flush().await?;
        trace!("Sent message: (size: {}): {:#?}", serialized.len(), message);
        Ok(())
    }

    pub async fn run(mut self, mut outbound_message_rx: tokio::sync::mpsc::Receiver<Message>) -> crate::Result<()> {

        loop {
            tokio::select! {
                Some(message) = outbound_message_rx.recv() => {
                    self.send(message).await?;
                },
                maybe_message = self.read_message() => {
                    match maybe_message {
                        Ok(None) => {
                            trace!("Connection closed");
                            return Ok(());
                        }
                        Err(e) => {
                            trace!("Error reading message: {}", e);
                            return Err(e);
                        }
                        Ok(Some(message)) => {
                            let msg_debug = format!("{:#?}", message);
                            self.inbound_message_tx.send(message).await?;
                            trace!("Received message: ({})", msg_debug);
                            self.buffer.clear();
                        }
                    }
                },
            }
        }
    }

    pub async fn read_message(&mut self) -> crate::Result<Option<Message>> {
        loop {
            if let Ok(message) = rmp_serde::from_slice(&self.buffer) {
                return Ok(Some(message));
            }

            if 0 == self.read_half.read_buf(&mut self.buffer).await? {
                // The remote closed the connection. For this to be a clean
                // shutdown, there should be no data in the read buffer. If
                // there is, this means that the peer closed the socket while
                // sending a frame.
                if self.buffer.is_empty() {
                    return Ok(None);
                } else {
                    return Err(crate::SyncrError::ConnectionResetByPeer);
                }
            }
        }
    }
}


#[derive(Debug, Serialize, Deserialize)]
// #[serde(tag = "type")]
pub enum Message {
    FileName(String),
    WeakChecksums(Vec<u32>),
    StrongChecksumRequest(Vec<usize>),
    StrongChecksums(Vec<(usize, u128)>),
    Matches(Vec<(usize, usize)>),
    Instructions(Vec<Instruction>),
}

impl Message {
    pub fn kind(&self) -> &str {
        match self {
            Message::FileName { .. } => "FileName",
            Message::WeakChecksums { .. } => "WeakChecksums",
            Message::StrongChecksumRequest { .. } => "StrongChecksumRequest",
            Message::StrongChecksums { .. } => "StrongChecksums",
            Message::Matches { .. } => "Matches",
            Message::Instructions { .. } => "Instructions",
        }
    }
}


#[derive(Debug, Serialize, Deserialize)]
pub enum Instruction {
    NewData {
        offset: usize,
        length: usize,
        bytes: Vec<u8>,
    },
    Replicate {
        from_offset: usize,
        length: usize,
        new_offset: usize,
    }
}