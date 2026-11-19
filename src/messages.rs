//! # Snapcast protocol messages
//!
//! `snapcast protocol` implementation of the streaming protocol as defined here:
//! https://github.com/snapcast/snapcast/blob/develop/doc/binary_protocol.md

use crate::types::TimeStamp;
use serde_json;

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
    // pub payload: String,
    pub buffer_ms: u16,
    pub latency: i16,
    pub muted: bool,
    pub volume: u16,
    // {
    //     "bufferMs": 1000,
    //     "latency": 0,
    //     "muted": false,
    //     "volume": 100
    // }
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
    // pub arch: String,
    // pub client_name: String,
    // pub host_name: String,
    // pub id: String,
    // pub instance: u16,
    // pub mac: String,
    // pub os: String,
    // pub protocol_version: u16,
    // pub client_version: String,
    // {
    //     "Arch": "x86_64",
    //     "Auth": {
    //         "param": "YmFkYWl4OnBhc3N3ZA==",
    //         "scheme": "Basic"
    //     },
    //     "ClientName": "Snapclient",
    //     "HostName": "my_hostname",
    //     "ID": "00:11:22:33:44:55",
    //     "Instance": 1,
    //     "MAC": "00:11:22:33:44:55",
    //     "OS": "Arch Linux",
    //     "SnapStreamProtocolVersion": 2,
    //     "Version": "0.32.0"
    // }
}

#[derive(Debug)]
pub struct ClientInfo {
    pub base: BaseMessage,
    pub volume: u16,
    pub muted: bool,
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
    fn serialize(&mut self) -> Vec<u8>;
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

impl BaseMessage {
    pub fn new(msgtype: MessageType) -> Self {
        BaseMessage {
            msgtype,
            id: 0,
            refers_to: 0,
            sent: TimeStamp::now(),
            received: TimeStamp::now(),
            size: 0,
        }
    }

    pub fn serialize(&mut self) -> Vec<u8> {
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

    fn serialize(&mut self) -> Vec<u8> {
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

    fn serialize(&mut self) -> Vec<u8> {
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
        let json: serde_json::Value =
            serde_json::from_slice(&buffer[4..(4 + payload_size as usize)]).unwrap();
        ServerSettings {
            base: basemessage,
            buffer_ms: json["bufferMs"].as_i64().unwrap() as u16,
            latency: json["latency"].as_i64().unwrap() as i16,
            muted: json["muted"].as_bool().unwrap(),
            volume: json["volume"].as_i64().unwrap() as u16,
        }
    }

    fn serialize(&mut self) -> Vec<u8> {
        let mut buffer = self.base.serialize();
        let json = serde_json::json!({"bufferMs": &self.buffer_ms, "latency": &self.latency, "muted": self.muted, "volume": &self.volume});
        let json_str = json.to_string();
        buffer.extend_from_slice(&(json_str.len() as u32).to_le_bytes());
        buffer.extend_from_slice(json_str.as_bytes());
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

    fn serialize(&mut self) -> Vec<u8> {
        self.base.sent = TimeStamp::now();
        let payload = self.latency.serialize();
        self.base.size = payload.len() as u32;
        let mut buffer = self.base.serialize();
        buffer.extend_from_slice(&payload);
        buffer
    }
}

impl Hello {
    pub fn new(payload: &str) -> Self {
        Hello {
            // base: BaseMessage::new(1, id, refers_to),
            base: BaseMessage::new(MessageType::Hello),
            payload: payload.to_string(),
            // arch: String::new(),
            // client_name: String::from_str("").unwrap(),
            // host_name: String::from_str("").unwrap(),
            // id: String::from_str("").unwrap(),
            // instance: 1,
            // mac: String::from_str("").unwrap(),
            // os: String::from_str("").unwrap(),
            // protocol_version: 2,
            // client_version: String::from_str("").unwrap(),
        }
    }
}

impl Time {
    pub fn new() -> Self {
        Time {
            base: BaseMessage::new(MessageType::Time),
            latency: TimeStamp::now(),
        }
    }
}

impl Default for Time {
    fn default() -> Self {
        Self::new()
    }
}

impl Message for ClientInfo {
    fn deserialize(basemessage: BaseMessage, buffer: &[u8]) -> Self {
        let payload_size = u32::from_le_bytes(buffer[0..4].try_into().unwrap());
        let json: serde_json::Value =
            serde_json::from_slice(&buffer[4..(4 + payload_size as usize)]).unwrap();
        ClientInfo {
            base: basemessage,
            volume: json["volume"].as_i64().unwrap() as u16,
            muted: json["muted"].as_bool().unwrap(),
        }
    }

    fn serialize(&mut self) -> Vec<u8> {
        let mut buffer = self.base.serialize();
        let json = serde_json::json!({"volume": self.volume, "muted": self.muted});
        let json_str = json.to_string();
        buffer.extend_from_slice(&(json_str.len() as u32).to_le_bytes());
        buffer.extend_from_slice(json_str.as_bytes());
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

    fn serialize(&mut self) -> Vec<u8> {
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

        // let json: serde_json::Value =
        //     serde_json::from_slice(&buffer[4..(4 + payload_size as usize)]).unwrap();
        // Hello {
        //     base: basemessage,
        //     arch: String::from_str(json["arch"].as_str().unwrap()).unwrap(),
        //     client_name: String::from_str(json["ClientName"].as_str().unwrap()).unwrap(),
        //     host_name: String::from_str(json["HostName"].as_str().unwrap()).unwrap(),
        //     id: String::from_str(json["ID"].as_str().unwrap()).unwrap(),
        //     instance: json["Instance"].as_i64().unwrap() as u16,
        //     mac: String::from_str(json["MAC"].as_str().unwrap()).unwrap(),
        //     os: String::from_str(json["OS"].as_str().unwrap()).unwrap(),
        //     protocol_version: json["SnapStreamProtocolVersion"].as_i64().unwrap() as u16,
        //     client_version: String::from_str(json["Version"].as_str().unwrap()).unwrap(),
        // }
    }

    fn serialize(&mut self) -> Vec<u8> {
        self.base.size = self.payload.len() as u32 + 4;
        let mut buffer = self.base.serialize();

        // let json = serde_json::json!(
        // {
        //     "Arch": &self.arch,
        //     "ClientName": &self.client_name,
        //     "HostName": &self.host_name,
        //     "ID": &self.id,
        //     "Instance": &self.instance,
        //     "MAC": &self.mac,
        //     "OS": &self.os,
        //     "SnapStreamProtocolVersion": &self.protocol_version,
        //     "Version": &self.client_version
        // });

        // let json_str = json.to_string();
        // buffer.extend_from_slice(&(json_str.len() as u32).to_le_bytes());
        // buffer.extend_from_slice(json_str.as_bytes());

        buffer.extend_from_slice(&(self.payload.len() as u32).to_le_bytes());
        buffer.extend_from_slice(self.payload.as_bytes());
        buffer
    }
}
