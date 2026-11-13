//! # Snapcast protocol messages
//!
//! `snapcast protocol` implementation of the streaming protocol as defined here:
//! https://github.com/snapcast/snapcast/blob/develop/doc/binary_protocol.md

use std::ops;

#[derive(Debug, Clone, Copy)]
pub enum MessageType {
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
pub struct SampleFormat {
    pub rate: u32,
    pub bits: u16,
    pub channels: u16,
}

#[derive(Debug)]
pub struct TimeStamp {
    sec: i32,
    usec: i32,
}

#[derive(Debug)]
pub struct BaseMessage {
    pub msgtype: MessageType,
    pub id: u16,
    pub refers_to: u16,
    pub sent: TimeStamp,
    pub received: TimeStamp,
    pub size: u32,
}

#[derive(Debug)]
pub struct CodecHeader {
    pub base: BaseMessage,
    pub codec: String,
    pub payload: Vec<u8>,
}

#[derive(Debug)]
pub struct WireChunk {
    pub base: BaseMessage,
    pub timestamp: TimeStamp,
    pub payload: Vec<u8>,
}

#[derive(Debug)]
pub struct ServerSettings {
    pub base: BaseMessage,
    pub payload: Vec<u8>,
}

#[derive(Debug)]
pub struct Time {
    pub base: BaseMessage,
    pub latency: TimeStamp,
}

#[derive(Debug)]
pub struct Hello {
    pub base: BaseMessage,
    pub payload: String,
}

#[derive(Debug)]
pub struct ClientInfo {
    pub base: BaseMessage,
    pub payload: String,
}

#[derive(Debug)]
pub struct ErrorMessage {
    pub base: BaseMessage,
    pub code: u32,
    pub message: String,
    pub details: String,
}

pub trait Message {
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
    pub fn serialize(&self) -> Vec<u8> {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&(self.msgtype as u16).to_le_bytes());
        buffer.extend_from_slice(&self.id.to_le_bytes());
        buffer.extend_from_slice(&self.refers_to.to_le_bytes());
        buffer.extend_from_slice(&self.sent.serialize());
        buffer.extend_from_slice(&self.received.serialize());
        buffer.extend_from_slice(&self.size.to_le_bytes());
        buffer
    }

    pub fn deserialize(buffer: &[u8]) -> Self {
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
        buffer.extend_from_slice(&self.codec.len().to_le_bytes());
        buffer.extend_from_slice(self.codec.as_bytes());
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
            payload,
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
    pub fn new(id: u16, refers_to: u16, payload: &str) -> Self {
        let size = payload.len() as u32;
        Hello {
            // base: BaseMessage::new(1, id, refers_to),
            base: BaseMessage {
                msgtype: MessageType::Hello,
                id,
                refers_to,
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
        buffer.extend_from_slice(&(self.payload.len() as u32).to_le_bytes());
        buffer.extend_from_slice(self.payload.as_bytes());
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
        buffer.extend_from_slice(&(self.message.len() as u32).to_le_bytes());
        buffer.extend_from_slice(self.message.as_bytes());
        buffer.extend_from_slice(&(self.details.len() as u32).to_le_bytes());
        buffer.extend_from_slice(self.details.as_bytes());
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
        buffer.extend_from_slice(&(self.payload.len() as u32).to_le_bytes());
        buffer.extend_from_slice(self.payload.as_bytes());
        buffer
    }
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
