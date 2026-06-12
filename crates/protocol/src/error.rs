use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("missing 4-byte length prefix")]
    MissingLengthPrefix,
    #[error("header too large: {size} bytes")]
    HeaderTooLarge { size: usize },
    #[error("incomplete header: expected {expected} bytes, got {actual}")]
    IncompleteHeader { expected: usize, actual: usize },
    #[error("json encode failed: {0}")]
    JsonEncode(#[from] serde_json::Error),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WireErrorCode {
    AuthFailed,
    SessionExpired,
    ServiceNotFound,
    ServiceDialFailed,
    P2pTimeout,
    RelayRequired,
    QuicHandshakeFailed,
    StreamOpenFailed,
    RateLimited,
    TrafficQuotaExceeded,
    MaxStreamsExceeded,
    SessionClosed,
}
