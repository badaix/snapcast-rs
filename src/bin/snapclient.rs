//! Snapcast client implementation

use std::error::Error;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use clap::Parser;
use log::{debug, error, info, trace, warn};
use snapcast::timesync::TimeSync;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use anyhow::Result;
use snapcast::codec::{Codec, Flac, Null, Wav};
use snapcast::messages::{self, Message};
use snapcast::player;
use snapcast::stream::AudioStream;
use snapcast::types::{self, TimeStamp};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Snapserver host
    #[arg(long)]
    host: String,

    #[arg(long, default_value = "info")]
    loglevel: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    info!("Connecting to {}!", args.host);
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(args.loglevel))
        .init();
    connect(&args.host).await?;
    Ok(())
}

async fn send<T: Message>(write_stream: &mut OwnedWriteHalf, message: &mut T) {
    let msg: Vec<u8> = message.serialize();
    write_stream
        .write_all(&msg)
        .await
        .expect("Failed to write message");
}

async fn sync_time(mut write_stream: OwnedWriteHalf) -> Result<()> {
    let mut id = 0;
    loop {
        let mut time = messages::Time::new();
        time.base.id = id;
        id += 1;
        debug!("Sending Time message: {:?}", time);
        send(&mut write_stream, &mut time).await;
        tokio::time::sleep(Duration::from_millis(1000)).await;
    }
}

async fn reader(mut read_stream: OwnedReadHalf) -> Result<()> {
    let mut audiostream = Arc::new(Mutex::new(AudioStream::new(types::SampleFormat {
        rate: 4800,
        bits: 16,
        channels: 2,
    })));
    let mut player = player::Player::new();

    // Read incoming messages in a loop
    let mut base_buffer: [u8; 26] = [0; 26];
    let mut buffer = Vec::new();
    let mut codec: Box<dyn Codec> = Box::new(Null);
    let mut timesync = TimeSync::new();
    // let mut player = player::Player::new();
    loop {
        // Read the base message first
        read_stream.read_exact(&mut base_buffer).await?;
        let mut base_message = messages::BaseMessage::deserialize(&base_buffer);
        base_message.received = TimeStamp::now();

        // Resize the buffer to the size of the incoming message and read the rest of the message
        buffer.resize(base_message.size as usize, 0);
        read_stream.read_exact(&mut buffer).await?;
        match base_message.msgtype {
            messages::MessageType::CodecHeader => {
                let codec_header = messages::CodecHeader::deserialize(base_message, &buffer);
                // debug!("Received CodecHeader message: {:?}", codec_header);
                match codec_header.codec.as_str() {
                    "pcm" => {
                        codec = Box::new(Wav::from_header(codec_header).unwrap());
                        info!("{}", codec);
                        audiostream = Arc::new(Mutex::new(AudioStream::new(
                            codec.as_ref().get_sampleformat(),
                        )));
                        player.play(Arc::clone(&audiostream));
                    }
                    "flac" => {
                        codec = Box::new(Flac::from_header(codec_header).unwrap());
                        info!("{}", codec);
                        audiostream = Arc::new(Mutex::new(AudioStream::new(
                            codec.as_ref().get_sampleformat(),
                        )));
                        player.play(Arc::clone(&audiostream));
                    }
                    "null" => {
                        codec = Box::new(Null::new());
                        info!("{}", codec);
                        audiostream = Arc::new(Mutex::new(AudioStream::new(
                            codec.as_ref().get_sampleformat(),
                        )));
                        player.play(Arc::clone(&audiostream));
                    }
                    "ogg" => warn!("ogg not implemented"),
                    "opus" => warn!("opus not implemented"),
                    _ => error!("Unknown codec: {}", codec_header.codec),
                }
            }
            messages::MessageType::WireChunk => {
                let wire_chunk = messages::WireChunk::deserialize(base_message, &buffer);
                // debug!("Received WireChunk message: {:?}", wire_chunk.base);
                let diff = timesync.get_diff();
                let now = TimeStamp::now();
                let local = wire_chunk.timestamp - diff;
                let play = local - now + TimeStamp::new(1, 0);
                trace!(
                    "wirechunk ts: {:?}, diff: {:?}, now: {:?}, play: {:?}",
                    wire_chunk.timestamp, local, now, play
                );
                if let Some(pcm_chunk) = codec.decode(wire_chunk) {
                    let mut audiostream = audiostream.lock().unwrap();
                    audiostream.add(pcm_chunk);
                }
            }
            messages::MessageType::ServerSettings => {
                let server_settings = messages::ServerSettings::deserialize(base_message, &buffer);
                debug!("Received ServerSettings message: {:?}", server_settings);
                let vol = server_settings.volume as i16;
                player
                    .volume
                    .store(if server_settings.muted { -vol } else { vol }, Relaxed);
            }
            messages::MessageType::Time => {
                let time_message = messages::Time::deserialize(base_message, &buffer);
                let latency_c2s = time_message.latency;
                let latency_s2c = time_message.base.received - time_message.base.sent;
                debug!("Received Time message: {:?}", time_message);
                debug!(
                    "Latency c2s: {:?}, s2c: {:?}, diff: {:?}",
                    latency_c2s,
                    latency_s2c,
                    latency_c2s - latency_s2c
                );
                timesync.add(&latency_c2s, &latency_s2c);
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
}

/// Connects to the Snapcast server at the given host and port, sends a Hello message, and listens for incoming messages.
pub async fn connect(host: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mac = mac_address::get_mac_address()?.unwrap().to_string();
    let stream = TcpStream::connect(host).await?;

    let (read_stream, mut write_stream) = stream.into_split();
    let mut hello = messages::Hello::new(
        serde_json::json!({"Arch": "x86_64", "Auth": {"param": "YmFkYWl4OnBhc3N3ZA==", "scheme": "Basic"}, "ClientName": "Snapclient-rust", "HostName": hostname::get().unwrap().to_str(), "ID": mac, "Instance": 1, "MAC": mac, "OS": "Arch Linux", "SnapStreamProtocolVersion": 2, "Version": "0.32.0"}).to_string().as_str(),
    );
    debug!("Sending Hello message: {:?}", hello);
    send(&mut write_stream, &mut hello).await;

    // for _i in 0..20 {
    //     let mut time = messages::Time::new();
    //     debug!("Sending Time message: {:?}", time);
    //     send(&mut write_stream, &mut time).await;
    //     tokio::time::sleep(Duration::from_millis(10)).await;
    // }

    tokio::spawn(async move {
        sync_time(write_stream).await.expect("Sync time failed");
    });

    reader(read_stream).await?;
    Ok(())
}
