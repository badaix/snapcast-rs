//! Snapcast client implementation

use std::error::Error;
use std::sync::{Arc, Mutex};

use clap::Parser;
use log::{debug, error, info, warn};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use snapcast::codec::{Codec, Flac};
use snapcast::messages::{self, Message};
use snapcast::player;
use snapcast::types;

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

    info!("Connecting to {}!", args.host);
    connect(&args.host).await?;
    Ok(())
}

/// Connects to the Snapcast server at the given host and port, sends a Hello message, and listens for incoming messages.
pub async fn connect(host: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut audiostream = Arc::new(Mutex::new(player::AudioStream::new(types::SampleFormat {
        rate: 4800,
        bits: 16,
        channels: 2,
    })));
    let mut player = player::Player::new();

    let mac = mac_address::get_mac_address()?.unwrap().to_string();
    let mut stream = TcpStream::connect(host).await?;
    let hello = messages::Hello::new(
        1,
        0,
        serde_json::json!({"Arch": "x86_64", "Auth": {"param": "YmFkYWl4OnBhc3N3ZA==", "scheme": "Basic"}, "ClientName": "Snapclient-rust", "HostName": hostname::get().unwrap().to_str(), "ID": mac, "Instance": 1, "MAC": mac, "OS": "Arch Linux", "SnapStreamProtocolVersion": 2, "Version": "0.32.0"}).to_string().as_str(),
    );
    debug!("Sending Hello message: {:?}", hello);
    let msg: Vec<u8> = hello.serialize();
    stream.write_all(&msg).await?;

    // Read incoming messages in a loop
    let mut base_buffer: [u8; 26] = [0; 26];
    let mut buffer = Vec::new();
    let mut codec: Option<Flac> = None;
    // let mut player = player::Player::new();
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
                match codec_header.codec.as_str() {
                    "pcm" => {
                        info!("pcm codec");
                        // codec = Wav::from_header(codec_header);
                        // info!("Sample format: {:?}", codec);
                        // audiostream = Arc::new(Mutex::new(player::AudioStream::new(
                        //     codec.as_ref().unwrap().sample_format,
                        // )));
                        // player.play(Arc::clone(&audiostream));

                        // let audiostream =
                        //     player::AudioStream::new(rx, codec.unwrap().sample_format);
                        // player.play(audiostream);
                    }
                    "ogg" => warn!("ogg not implemented"),
                    "flac" => {
                        info!("flac codec");
                        codec = Flac::from_header(codec_header);
                        // info!("Sample format: {:?}", codec);
                        audiostream = Arc::new(Mutex::new(player::AudioStream::new(
                            codec.as_ref().unwrap().sample_format,
                        )));
                        player.play(Arc::clone(&audiostream));
                        // let audiostream =
                        //     player::AudioStream::new(rx, codec.unwrap().sample_format);
                        // player.play(audiostream);
                    }
                    "opus" => warn!("opus not implemented"),
                    "null" => warn!("null not implemented"),
                    _ => error!("Unknown codec: {}", codec_header.codec),
                }
            }
            messages::MessageType::WireChunk => {
                let wire_chunk = messages::WireChunk::deserialize(base_message, &buffer);
                // debug!("Received WireChunk message: {:?}", wire_chunk.base);
                if let Some(ref mut c) = codec
                    && let Some(pcm_chunk) = c.decode(wire_chunk)
                {
                    let mut audiostream = audiostream.lock().unwrap();
                    audiostream.add(pcm_chunk);
                }
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
