//! # Snapcast Protocol
//!
//! `snapcast protocol` implementation of the streaming protocol as defined here:
//! https://github.com/snapcast/snapcast/blob/develop/doc/binary_protocol.md

use std::ops;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[derive(Debug, Clone, Copy)]
enum MessageType {
    BaseMessage = 0,
    CodecHeader = 1,
    WireChunk = 2,
    ServerSettings = 3,
    Time = 4,
    Hello = 5,
    ClientInfo = 7,
    Error = 8,
}

#[derive(Debug)]
struct TimeStamp {
    sec: i32,
    usec: i32,
}

#[derive(Debug)]
struct BaseMessage {
    msgtype: MessageType,
    id: u16,
    refers_to: u16,
    sent: TimeStamp,
    received: TimeStamp,
    size: u32,
}

#[derive(Debug)]
struct CodecHeader {
    base: BaseMessage,
    codec: String,
    payload: Vec<u8>,
}

#[derive(Debug)]
struct WireChunk {
    base: BaseMessage,
    timestamp: TimeStamp,
    payload: Vec<u8>,
}

#[derive(Debug)]
struct ServerSettings {
    base: BaseMessage,
    payload: Vec<u8>,
}

#[derive(Debug)]
struct Time {
    base: BaseMessage,
    latency: TimeStamp,
}

#[derive(Debug)]
struct Hello {
    base: BaseMessage,
    payload: String,
}

#[derive(Debug)]
struct ClientInfo {
    base: BaseMessage,
    payload: String,
}

#[derive(Debug)]
struct ErrorMessage {
    base: BaseMessage,
    code: u32,
    message: String,
    details: String,
}

trait Message {
    fn deserialize(basemessage: BaseMessage, buffer: &[u8]) -> Self;
    fn serialize(&self) -> Vec<u8>;
}

impl From<u16> for MessageType {
    fn from(value: u16) -> Self {
        match value {
            0 => MessageType::BaseMessage,
            1 => MessageType::CodecHeader,
            2 => MessageType::WireChunk,
            3 => MessageType::ServerSettings,
            4 => MessageType::Time,
            5 => MessageType::Hello,
            7 => MessageType::ClientInfo,
            8 => MessageType::Error,
            _ => panic!("Unknown message type: {}", value),
        }
    }
}

impl TimeStamp {
    fn new(sec: i32, usec: i32) -> Self {
        TimeStamp { sec, usec }
    }

    fn now() -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap();
        TimeStamp {
            sec: now.as_secs() as i32,
            usec: now.subsec_micros() as i32,
        }
    }

    fn deserialize(buffer: &[u8]) -> Self {
        let sec = i32::from_le_bytes(buffer[0..4].try_into().unwrap());
        let usec = i32::from_le_bytes(buffer[4..8].try_into().unwrap());
        TimeStamp { sec, usec }
    }

    fn serialize(&self) -> Vec<u8> {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&self.sec.to_le_bytes());
        buffer.extend_from_slice(&self.usec.to_le_bytes());
        buffer
    }
}

impl ops::Sub<TimeStamp> for TimeStamp {
    type Output = TimeStamp;

    fn sub(self, other: TimeStamp) -> TimeStamp {
        let mut sec = self.sec - other.sec;
        let mut usec = self.usec - other.usec;
        if usec < 0 {
            sec -= 1;
            usec += 1_000_000;
        }
        TimeStamp { sec, usec }
    }
}

impl ops::Add<TimeStamp> for TimeStamp {
    type Output = TimeStamp;

    fn add(self, other: TimeStamp) -> TimeStamp {
        let mut sec = self.sec + other.sec;
        let mut usec = self.usec + other.usec;
        if usec >= 1_000_000 {
            sec += 1;
            usec -= 1_000_000;
        }
        TimeStamp { sec, usec }
    }
}

impl BaseMessage {
    fn serialize(&self) -> Vec<u8> {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&(self.msgtype as u16).to_le_bytes());
        buffer.extend_from_slice(&self.id.to_le_bytes());
        buffer.extend_from_slice(&self.refers_to.to_le_bytes());
        buffer.extend_from_slice(&self.sent.serialize());
        buffer.extend_from_slice(&self.received.serialize());
        buffer.extend_from_slice(&self.size.to_le_bytes());
        buffer
    }

    fn deserialize(buffer: &[u8]) -> Self {
        let msgtype = u16::from_le_bytes(buffer[0..2].try_into().unwrap());
        let id = u16::from_le_bytes(buffer[2..4].try_into().unwrap());
        let refers_to = u16::from_le_bytes(buffer[4..6].try_into().unwrap());
        let sent = TimeStamp::deserialize(&buffer[6..14]);
        let received = TimeStamp::now();
        let size = u32::from_le_bytes(buffer[22..26].try_into().unwrap());

        BaseMessage {
            msgtype: MessageType::from(msgtype),
            id,
            refers_to,
            sent,
            received,
            size,
        }
    }
}

impl Message for CodecHeader {
    fn deserialize(basemessage: BaseMessage, buffer: &[u8]) -> Self {
        let codec_size = u32::from_le_bytes(buffer[0..4].try_into().unwrap());
        let codec = String::from_utf8_lossy(&buffer[4..(4 + codec_size as usize)]).into_owned();
        let payload = buffer[(8 + codec_size as usize)..].to_vec();
        CodecHeader {
            base: basemessage,
            codec,
            payload,
        }
    }

    fn serialize(&self) -> Vec<u8> {
        let mut buffer = self.base.serialize();
        buffer.extend_from_slice(&self.codec.as_bytes().len().to_le_bytes());
        buffer.extend_from_slice(&self.codec.as_bytes());
        buffer.extend_from_slice(&self.payload.len().to_le_bytes());
        buffer.extend_from_slice(&self.payload);
        buffer
    }
}

impl Message for WireChunk {
    fn deserialize(basemessage: BaseMessage, buffer: &[u8]) -> Self {
        let timestamp = TimeStamp::deserialize(&buffer[0..8]);
        let payload_size = u32::from_le_bytes(buffer[8..12].try_into().unwrap());
        let payload = buffer[12..(12 + payload_size as usize)].to_vec();
        WireChunk {
            base: basemessage,
            timestamp,
            payload,
        }
    }

    fn serialize(&self) -> Vec<u8> {
        let mut buffer = self.base.serialize();
        buffer.extend_from_slice(&self.timestamp.serialize());
        buffer.extend_from_slice(&(self.payload.len() as u32).to_le_bytes());
        buffer.extend_from_slice(&self.payload);
        buffer
    }
}

impl Message for ServerSettings {
    fn deserialize(basemessage: BaseMessage, buffer: &[u8]) -> Self {
        let payload_size = u32::from_le_bytes(buffer[0..4].try_into().unwrap());
        let payload = buffer[4..(4 + payload_size as usize)].to_vec();
        ServerSettings {
            base: basemessage,
            payload: payload,
        }
    }

    fn serialize(&self) -> Vec<u8> {
        let mut buffer = self.base.serialize();
        buffer.extend_from_slice(&self.payload.len().to_le_bytes());
        buffer.extend_from_slice(&self.payload);
        buffer
    }
}

impl Message for Time {
    fn deserialize(basemessage: BaseMessage, buffer: &[u8]) -> Self {
        let latency = TimeStamp::deserialize(&buffer[0..8]);
        Time {
            base: basemessage,
            latency,
        }
    }

    fn serialize(&self) -> Vec<u8> {
        let mut buffer = self.base.serialize();
        buffer.extend_from_slice(&self.latency.serialize());
        buffer
    }
}

impl Hello {
    fn new(id: u16, refers_to: u16, payload: &str) -> Self {
        let size = payload.len() as u32;
        Hello {
            // base: BaseMessage::new(1, id, refers_to),
            base: BaseMessage {
                msgtype: MessageType::Hello,
                id: id,
                refers_to: refers_to,
                sent: TimeStamp::new(0, 0),
                received: TimeStamp::new(0, 0),
                size: size + 4, // size of the payload plus the size field itself
            },
            payload: payload.to_string(),
        }
    }
}

impl Message for ClientInfo {
    fn deserialize(basemessage: BaseMessage, buffer: &[u8]) -> Self {
        let payload_size = u32::from_le_bytes(buffer[0..4].try_into().unwrap());
        let payload = String::from_utf8_lossy(&buffer[4..(4 + payload_size as usize)]).into_owned();
        ClientInfo {
            base: basemessage,
            payload,
        }
    }

    fn serialize(&self) -> Vec<u8> {
        let mut buffer = self.base.serialize();
        buffer.extend_from_slice(&(self.payload.as_bytes().len() as u32).to_le_bytes());
        buffer.extend_from_slice(&self.payload.as_bytes());
        buffer
    }
}

impl Message for ErrorMessage {
    fn deserialize(basemessage: BaseMessage, buffer: &[u8]) -> Self {
        let code = u32::from_le_bytes(buffer[0..4].try_into().unwrap());
        let message_size = u32::from_le_bytes(buffer[4..8].try_into().unwrap());
        let message = String::from_utf8_lossy(&buffer[8..(8 + message_size as usize)]).into_owned();
        let details_size = u32::from_le_bytes(
            buffer[(8 + message_size as usize)..(12 + message_size as usize)]
                .try_into()
                .unwrap(),
        );
        let details = String::from_utf8_lossy(
            &buffer[(12 + message_size as usize)
                ..(12 + message_size as usize + details_size as usize)],
        )
        .into_owned();
        ErrorMessage {
            base: basemessage,
            code,
            message,
            details,
        }
    }

    fn serialize(&self) -> Vec<u8> {
        let mut buffer = self.base.serialize();
        buffer.extend_from_slice(&self.code.to_le_bytes());
        buffer.extend_from_slice(&(self.message.as_bytes().len() as u32).to_le_bytes());
        buffer.extend_from_slice(&self.message.as_bytes());
        buffer.extend_from_slice(&(self.details.as_bytes().len() as u32).to_le_bytes());
        buffer.extend_from_slice(&self.details.as_bytes());
        buffer
    }
}

impl Message for Hello {
    fn deserialize(basemessage: BaseMessage, buffer: &[u8]) -> Self {
        let payload_size = u32::from_le_bytes(buffer[0..4].try_into().unwrap());
        let payload = String::from_utf8_lossy(&buffer[4..(4 + payload_size as usize)]).into_owned();
        Hello {
            base: basemessage,
            payload,
        }
    }

    fn serialize(&self) -> Vec<u8> {
        let mut buffer = self.base.serialize();
        buffer.extend_from_slice(&(self.payload.as_bytes().len() as u32).to_le_bytes());
        buffer.extend_from_slice(&self.payload.as_bytes());
        buffer
    }
}

/// Connects to the Snapcast server at the given host and port, sends a Hello message, and listens for incoming messages.
pub async fn connect(host: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect(host).await?;

    let hello = Hello::new(
        1,
        0,
        r#"{"Arch": "x86_64", "Auth": {"param": "YmFkYWl4OnBhc3N3ZA==", "scheme": "Basic"}, "ClientName": "Snapclient-rust", "HostName": "my_hostname", "ID": "00:11:22:33:44:55", "Instance": 1, "MAC": "00:11:22:33:44:55", "OS": "Arch Linux", "SnapStreamProtocolVersion": 2, "Version": "0.32.0"}"#,
    );

    println!("Sending Hello message: {:?}", hello);
    let msg: Vec<u8> = hello.serialize();
    stream.write_all(&msg).await?;

    // Read incoming messages in a loop
    let mut base_buffer: [u8; 26] = [0; 26];
    let mut buffer = Vec::new();
    loop {
        // Read the base message first
        stream.read_exact(&mut base_buffer).await?;
        let base_message = BaseMessage::deserialize(&base_buffer);

        // Resize the buffer to the size of the incoming message and read the rest of the message
        buffer.resize(base_message.size as usize, 0);
        stream.read_exact(&mut buffer).await?;
        match base_message.msgtype {
            MessageType::CodecHeader => {
                let codec_header = CodecHeader::deserialize(base_message, &buffer);
                println!("Received CodecHeader message: {:?}", codec_header);
            }
            MessageType::WireChunk => {
                let wire_chunk = WireChunk::deserialize(base_message, &buffer);
                println!("Received WireChunk message: {:?}", wire_chunk.base);
            }
            MessageType::ServerSettings => {
                let server_settings = ServerSettings::deserialize(base_message, &buffer);
                println!("Received ServerSettings message: {:?}", server_settings);
            }
            MessageType::Time => {
                let time_message = Time::deserialize(base_message, &buffer);
                println!("Received Time message: {:?}", time_message);
            }
            MessageType::Hello => {
                let hello_message = Hello::deserialize(base_message, &buffer);
                println!("Received Hello message: {:?}", hello_message);
            }
            MessageType::ClientInfo => {
                let client_info = ClientInfo::deserialize(base_message, &buffer);
                println!("Received ClientInfo message: {:?}", client_info);
            }
            MessageType::Error => {
                let error_message = ErrorMessage::deserialize(base_message, &buffer);
                println!("Received Error message: {:?}", error_message);
            }
            _ => {
                println!("Received other message type: {:?}", base_message.msgtype);
            }
        }
    }

    // Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_subtraction() {
        let t1 = TimeStamp::new(5, 500_000);
        let t2 = TimeStamp::new(3, 800_000);
        let result = t1 - t2;
        assert_eq!(result.sec, 1);
        assert_eq!(result.usec, 700_000);
    }

    #[test]
    fn test_timestamp_addition() {
        let t1 = TimeStamp::new(5, 500_000);
        let t2 = TimeStamp::new(3, 800_000);
        let result = t1 + t2;
        assert_eq!(result.sec, 9);
        assert_eq!(result.usec, 300_000);
    }
}
