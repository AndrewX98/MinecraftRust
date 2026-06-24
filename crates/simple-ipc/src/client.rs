use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use bytes::{Buf, BytesMut};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::{Mutex, oneshot};

use crate::encoding::Encoding;
use crate::message::{
    HelloRequest, HelloResponse, Message, MessageId, PROTOCOL_VERSION,
};
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("RPC error (code {code}): {message}")]
    Rpc { code: i32, message: String, data: serde_json::Value },
    #[error("Connection closed")]
    ConnectionClosed,
}

pub struct Client {
    stream: Arc<Mutex<UnixStream>>,
    encoding: Encoding,
    next_id: AtomicI64,
    pending: Arc<Mutex<HashMap<MessageId, oneshot::Sender<Result<serde_json::Value, ClientError>>>>>,
    read_buf: BytesMut,
}

impl Client {
    pub async fn connect(path: &str) -> Result<Self, ClientError> {
        let stream = UnixStream::connect(path).await?;
        let mut client = Client {
            stream: Arc::new(Mutex::new(stream)),
            encoding: Encoding::Json, // temporary, will be negotiated
            next_id: AtomicI64::new(1),
            pending: Arc::new(Mutex::new(HashMap::new())),
            read_buf: BytesMut::with_capacity(4096),
        };

        // Send hello
        let hello = HelloRequest {
            version: PROTOCOL_VERSION,
            encodings: crate::encoding::PREFERRED_ENCODINGS.iter().map(|s| s.to_string()).collect(),
        };
        let hello_params = serde_json::to_value(&hello)
            .map_err(|e| ClientError::Protocol(format!("Serialize hello: {}", e)))?;

        let response = client.call_raw(".hello", hello_params).await?;

        let hello_resp: HelloResponse = serde_json::from_value(response)
            .map_err(|e| ClientError::Protocol(format!("Parse hello response: {}", e)))?;

        let encoding = Encoding::from_name(&hello_resp.encoding)
            .ok_or_else(|| ClientError::Protocol(format!("Unknown encoding: {}", hello_resp.encoding)))?;
        client.encoding = encoding;

        Ok(client)
    }

    pub async fn call(&mut self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, ClientError> {
        self.call_raw(method, params).await
    }

    async fn call_raw(&mut self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, ClientError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, _rx) = oneshot::channel();

        {
            let mut pending = self.pending.lock().await;
            pending.insert(id, tx);
        }

        let msg = Message::Rpc(crate::message::RpcMessage {
            id: Some(id),
            method: method.to_string(),
            params,
        });

        let mut buf = BytesMut::new();
        self.encoding.encode_message(&msg, &mut buf);

        {
            let mut stream = self.stream.lock().await;
            stream.write_all(&buf).await?;
        }

        // Spawn reader task if not already running
        // For simplicity, read response synchronously
        let pending = self.pending.clone();
        let stream = self.stream.clone();
        let encoding = self.encoding;
        let read_buf = &mut self.read_buf;

        loop {
            let mut local_buf = [0u8; 4096];
            let n = {
                let mut s = stream.lock().await;
                s.read(&mut local_buf).await?
            };
            if n == 0 {
                // Connection closed
                let mut p = pending.lock().await;
                if let Some(tx) = p.remove(&id) {
                    let _ = tx.send(Err(ClientError::ConnectionClosed));
                }
                return Err(ClientError::ConnectionClosed);
            }
            read_buf.extend_from_slice(&local_buf[..n]);

            loop {
                match encoding.decode_message(read_buf) {
                    Ok(Some((msg, consumed))) => {
                        read_buf.advance(consumed);
                        match msg {
                            Message::Response(resp) => {
                                if resp.id == Some(id) {
                                    return Ok(resp.result);
                                }
                                // Mismatched ID, dispatch to pending
                                if let Some(id_val) = resp.id {
                                    let mut p = pending.lock().await;
                                    if let Some(tx) = p.remove(&id_val) {
                                        let _ = tx.send(Ok(resp.result));
                                    }
                                }
                            }
                            Message::Error(err) => {
                                if err.id == Some(id) {
                                    return Err(ClientError::Rpc {
                                        code: err.error.code,
                                        message: err.error.message,
                                        data: err.error.data,
                                    });
                                }
                                if let Some(id_val) = err.id {
                                    let mut p = pending.lock().await;
                                    if let Some(tx) = p.remove(&id_val) {
                                        let _ = tx.send(Err(ClientError::Rpc {
                                            code: err.error.code,
                                            message: err.error.message,
                                            data: err.error.data,
                                        }));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        let mut p = pending.lock().await;
                        if let Some(tx) = p.remove(&id) {
                            let _ = tx.send(Err(ClientError::Protocol(e)));
                        }
                        return Err(ClientError::Protocol("decode error".into()));
                    }
                }
            }
        }
    }

    pub async fn notify(&mut self, method: &str, params: serde_json::Value) -> Result<(), ClientError> {
        let msg = Message::Rpc(crate::message::RpcMessage {
            id: None,
            method: method.to_string(),
            params,
        });
        let mut buf = BytesMut::new();
        self.encoding.encode_message(&msg, &mut buf);
        let mut stream = self.stream.lock().await;
        stream.write_all(&buf).await?;
        Ok(())
    }

    pub async fn close(&mut self) -> Result<(), ClientError> {
        let mut stream = self.stream.lock().await;
        stream.shutdown().await?;
        Ok(())
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        // Best effort: notify all pending callbacks
        let pending = self.pending.clone();
        let stream = self.stream.clone();
        tokio::spawn(async move {
            let mut s = stream.lock().await;
            let _ = s.shutdown().await;
            let mut p = pending.lock().await;
            for (_, tx) in p.drain() {
                let _ = tx.send(Err(ClientError::ConnectionClosed));
            }
        });
    }
}
