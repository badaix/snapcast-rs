//! Snapcast client implementation

use std::error::Error;

use clap::Parser;
use log::{debug, info};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use snapcast::messages::{self, Message};
use snapcast::player;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Snapserver host
    #[arg(long)]
    host: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    let args = Args::parse();
    let (tx, rx) = mpsc::channel::<messages::WireChunk>(256);
    let mut player = player::Player::new();
    player.play(player::Stream::new(rx));
    info!("Connecting to {}!", args.host);
    connect(&args.host, tx).await?;
    Ok(())
}

/// Connects to the Snapcast server at the given host and port, sends a Hello message, and listens for incoming messages.
pub async fn connect(
    host: &str,
    channel: mpsc::Sender<messages::WireChunk>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect(host).await?;

    let hello = messages::Hello::new(
        1,
        0,
        r#"{"Arch": "x86_64", "Auth": {"param": "YmFkYWl4OnBhc3N3ZA==", "scheme": "Basic"}, "ClientName": "Snapclient-rust", "HostName": "my_hostname", "ID": "00:11:22:33:44:55", "Instance": 1, "MAC": "00:11:22:33:44:55", "OS": "Arch Linux", "SnapStreamProtocolVersion": 2, "Version": "0.32.0"}"#,
    );

    debug!("Sending Hello message: {:?}", hello);
    let msg: Vec<u8> = hello.serialize();
    stream.write_all(&msg).await?;

    // Read incoming messages in a loop
    let mut base_buffer: [u8; 26] = [0; 26];
    let mut buffer = Vec::new();
    loop {
        // Read the base message first
        stream.read_exact(&mut base_buffer).await?;
        let base_message = messages::BaseMessage::deserialize(&base_buffer);

        // Resize the buffer to the size of the incoming message and read the rest of the message
        buffer.resize(base_message.size as usize, 0);
        stream.read_exact(&mut buffer).await?;
        match base_message.msgtype {
            messages::MessageType::CodecHeader => {
                let codec_header = messages::CodecHeader::deserialize(base_message, &buffer);
                debug!("Received CodecHeader message: {:?}", codec_header);
            }
            messages::MessageType::WireChunk => {
                let wire_chunk = messages::WireChunk::deserialize(base_message, &buffer);
                debug!("Received WireChunk message: {:?}", wire_chunk.base);
                channel.send(wire_chunk).await?;
            }
            messages::MessageType::ServerSettings => {
                let server_settings = messages::ServerSettings::deserialize(base_message, &buffer);
                debug!("Received ServerSettings message: {:?}", server_settings);
            }
            messages::MessageType::Time => {
                let time_message = messages::Time::deserialize(base_message, &buffer);
                debug!("Received Time message: {:?}", time_message);
            }
            messages::MessageType::Hello => {
                let hello_message = messages::Hello::deserialize(base_message, &buffer);
                debug!("Received Hello message: {:?}", hello_message);
            }
            messages::MessageType::ClientInfo => {
                let client_info = messages::ClientInfo::deserialize(base_message, &buffer);
                debug!("Received ClientInfo message: {:?}", client_info);
            }
            messages::MessageType::Error => {
                let error_message = messages::ErrorMessage::deserialize(base_message, &buffer);
                debug!("Received Error message: {:?}", error_message);
            }
            _ => {
                debug!("Received other message type: {:?}", base_message.msgtype);
            }
        }
    }

    // Ok(())
}
