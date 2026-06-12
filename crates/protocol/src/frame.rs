use serde::{Deserialize, Serialize};

use crate::{
    error::{ProtocolError, WireErrorCode},
    ids::{ClientId, DeviceId, ServiceId, SessionId, StreamId},
    model::TrafficStats,
};

const HEADER_LEN_SIZE: usize = 4;
const MAX_HEADER_LEN: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataStreamHeader {
    pub stream_id: StreamId,
    pub session_id: SessionId,
    pub service_id: ServiceId,
}

impl DataStreamHeader {
    pub fn encode_with_len_prefix(&self) -> Result<Vec<u8>, ProtocolError> {
        let header = serde_json::to_vec(self)?;
        if header.len() > MAX_HEADER_LEN {
            return Err(ProtocolError::HeaderTooLarge { size: header.len() });
        }

        let mut bytes = Vec::with_capacity(HEADER_LEN_SIZE + header.len());
        bytes.extend_from_slice(&(header.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&header);
        Ok(bytes)
    }

    pub fn decode_with_len_prefix(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() < HEADER_LEN_SIZE {
            return Err(ProtocolError::MissingLengthPrefix);
        }

        let header_len =
            u32::from_be_bytes(bytes[..HEADER_LEN_SIZE].try_into().expect("slice len")) as usize;
        if header_len > MAX_HEADER_LEN {
            return Err(ProtocolError::HeaderTooLarge { size: header_len });
        }

        let expected = HEADER_LEN_SIZE + header_len;
        if bytes.len() < expected {
            return Err(ProtocolError::IncompleteHeader {
                expected,
                actual: bytes.len(),
            });
        }

        Ok(serde_json::from_slice(
            &bytes[HEADER_LEN_SIZE..HEADER_LEN_SIZE + header_len],
        )?)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ControlFrame {
    Hello(HelloFrame),
    Auth(AuthFrame),
    Ping,
    Pong,
    Error(ErrorFrame),
    TrafficReport(TrafficStats),
    RelayBind(RelayBindFrame),
    SessionReady { session_id: SessionId },
    SessionClosed { session_id: SessionId },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelloFrame {
    pub role: PeerRole,
    pub client_id: Option<ClientId>,
    pub device_id: Option<DeviceId>,
    pub session_id: SessionId,
    pub protocol_version: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthFrame {
    pub session_id: SessionId,
    pub token: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorFrame {
    pub code: WireErrorCode,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayBindFrame {
    pub role: PeerRole,
    pub session_id: SessionId,
    pub token: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeerRole {
    Mobile,
    Agent,
}
