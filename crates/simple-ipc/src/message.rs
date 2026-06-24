use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type MessageId = i64;
pub type ErrorCode = i32;

pub const SUCCESS: ErrorCode = 0;
pub const PARSE_ERROR: ErrorCode = -32700;
pub const INVALID_REQUEST: ErrorCode = -32600;
pub const METHOD_NOT_FOUND: ErrorCode = -32601;
pub const INVALID_PARAMS: ErrorCode = -32602;
pub const INTERNAL_ERROR: ErrorCode = -32603;
pub const CONNECTION_CLOSED: ErrorCode = -32000;
pub const NO_HELLO_REPLY: ErrorCode = -32001;

pub const PROTOCOL_VERSION: i32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcMessage {
    pub id: Option<MessageId>,
    pub method: String,
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMessage {
    pub id: Option<MessageId>,
    pub result: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMessage {
    pub id: Option<MessageId>,
    pub error: ErrorBody,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: ErrorCode,
    pub message: String,
    #[serde(default)]
    pub data: Value,
}

#[derive(Debug, Clone)]
pub enum Message {
    Rpc(RpcMessage),
    Response(ResponseMessage),
    Error(ErrorMessage),
}

/// Hello handshake: client proposes encodings, server picks one
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloRequest {
    pub version: i32,
    pub encodings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloResponse {
    pub version: i32,
    pub encoding: String,
}
