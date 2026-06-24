use crate::message::{Message, RpcMessage, ResponseMessage, ErrorMessage, ErrorBody};
use crate::varint;
use bytes::{BufMut, BytesMut};
use serde_json::Value;

pub const JSON_NAME: &str = "json";
pub const JSON_CBOR_NAME: &str = "json_cbor";

pub const PREFERRED_ENCODINGS: &[&str] = &[JSON_CBOR_NAME, JSON_NAME];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Json,
    JsonCbor,
}

impl Encoding {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            JSON_NAME => Some(Encoding::Json),
            JSON_CBOR_NAME => Some(Encoding::JsonCbor),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Encoding::Json => JSON_NAME,
            Encoding::JsonCbor => JSON_CBOR_NAME,
        }
    }

    pub fn pick_from_preferred(preferred: &[String]) -> Option<Encoding> {
        for name in PREFERRED_ENCODINGS {
            if preferred.iter().any(|s| s == *name) {
                return Encoding::from_name(name);
            }
        }
        None
    }

    pub fn encode_message(&self, msg: &Message, buf: &mut BytesMut) {
        match self {
            Encoding::Json => self.encode_json(msg, buf),
            Encoding::JsonCbor => self.encode_cbor(msg, buf),
        }
    }

    pub fn decode_message(&self, buf: &[u8]) -> Result<Option<(Message, usize)>, String> {
        match self {
            Encoding::Json => self.decode_json(buf),
            Encoding::JsonCbor => self.decode_cbor(buf),
        }
    }

    fn encode_json(&self, msg: &Message, buf: &mut BytesMut) {
        let json = match msg {
            Message::Rpc(m) => {
                let mut map = serde_json::Map::new();
                if let Some(id) = m.id {
                    map.insert("id".into(), Value::from(id));
                }
                map.insert("method".into(), Value::from(m.method.as_str()));
                map.insert("params".into(), m.params.clone());
                serde_json::to_string(&map)
            }
            Message::Response(m) => {
                let mut map = serde_json::Map::new();
                if let Some(id) = m.id {
                    map.insert("id".into(), Value::from(id));
                }
                map.insert("result".into(), m.result.clone());
                serde_json::to_string(&map)
            }
            Message::Error(m) => {
                let mut map = serde_json::Map::new();
                if let Some(id) = m.id {
                    map.insert("id".into(), Value::from(id));
                }
                map.insert("error".into(), serde_json::to_value(&m.error).unwrap());
                serde_json::to_string(&map)
            }
        };
        if let Ok(s) = json {
            buf.put_slice(s.as_bytes());
            buf.put_u8(b'\n');
        }
    }

    fn decode_json(&self, buf: &[u8]) -> Result<Option<(Message, usize)>, String> {
        let end = buf.iter().position(|&b| b == b'\n' || b == b'\0');
        let end = match end {
            Some(pos) => pos,
            None => return Ok(None),
        };
        let line = &buf[..end];
        if line.is_empty() {
            return Ok(Some((Message::Response(ResponseMessage {
                id: None,
                result: Value::Null,
            }), end + 1)));
        }
        let value: serde_json::Value =
            serde_json::from_slice(line).map_err(|e| format!("JSON parse error: {}", e))?;
        let obj = value.as_object().ok_or("Expected JSON object")?;
        let id = obj.get("id").and_then(|v| v.as_i64());

        if let Some(error_val) = obj.get("error") {
            let error: ErrorBody =
                serde_json::from_value(error_val.clone())
                    .map_err(|e| format!("Error parse: {}", e))?;
            return Ok(Some((Message::Error(ErrorMessage { id, error }), end + 1)));
        }

        if let Some(result) = obj.get("result") {
            return Ok(Some((Message::Response(ResponseMessage { id, result: result.clone() }), end + 1)));
        }

        if let Some(method) = obj.get("method").and_then(|v| v.as_str()) {
            let params = obj.get("params").cloned().unwrap_or(Value::Null);
            return Ok(Some((Message::Rpc(RpcMessage {
                id,
                method: method.to_string(),
                params,
            }), end + 1)));
        }

        Err("Unknown message format".into())
    }

    fn encode_cbor(&self, msg: &Message, buf: &mut BytesMut) {
        let cbor = match msg {
            Message::Rpc(m) => {
                let id_val = m.id.map(|id| Value::from(id));
                let arr = vec![
                    id_val.map_or(Value::Null, |v| v),
                    Value::from(m.method.as_str()),
                    m.params.clone(),
                ];
                serde_cbor::to_vec(&arr)
            }
            Message::Response(m) => {
                let id_val = m.id.map(|id| Value::from(id));
                let arr = vec![
                    id_val.map_or(Value::Null, |v| v),
                    m.result.clone(),
                ];
                serde_cbor::to_vec(&arr)
            }
            Message::Error(m) => {
                let id_val = m.id.map(|id| Value::from(id));
                let arr = vec![
                    id_val.map_or(Value::Null, |v| v),
                    Value::from(m.error.code),
                    Value::from(m.error.message.as_str()),
                    m.error.data.clone(),
                ];
                serde_cbor::to_vec(&arr)
            }
        };
        if let Ok(cbor_bytes) = cbor {
            let mut varint_buf = [0u8; 10];
            let n = varint::encode_unsigned(cbor_bytes.len() as u64, &mut varint_buf);
            buf.put_slice(&varint_buf[..n]);
            buf.put_slice(&cbor_bytes);
        }
    }

    fn decode_cbor(&self, buf: &[u8]) -> Result<Option<(Message, usize)>, String> {
        let (len, varint_size) = varint::decode_unsigned(buf)?;
        let len = len as usize;
        let total = varint_size + len;
        if buf.len() < total {
            return Ok(None);
        }
        let cbor_data = &buf[varint_size..total];
        let value: Vec<Value> = serde_cbor::from_slice(cbor_data)
            .map_err(|e| format!("CBOR parse error: {}", e))?;

        if value.len() == 4 {
            let id = value[0].as_i64();
            let code = value[1].as_i64().unwrap_or(0) as i32;
            let message = value[2].as_str().unwrap_or("").to_string();
            let data = value[3].clone();
            Ok(Some((Message::Error(ErrorMessage {
                id,
                error: ErrorBody { code, message, data },
            }), total)))
        } else if value.len() == 3 {
            let id = value[0].as_i64();
            let method = value[1].as_str().unwrap_or("").to_string();
            let params = value[2].clone();
            Ok(Some((Message::Rpc(RpcMessage {
                id,
                method,
                params,
            }), total)))
        } else if value.len() == 2 {
            let id = value[0].as_i64();
            let result = value[1].clone();
            Ok(Some((Message::Response(ResponseMessage {
                id,
                result,
            }), total)))
        } else {
            Err(format!("Unexpected CBOR array length: {}", value.len()))
        }
    }
}
