use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use bytes::{Buf, BytesMut};
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::UnixStream;
use tokio::sync::{Mutex, RwLock, oneshot};
use tokio::task::JoinHandle;

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

type PendingMap = HashMap<MessageId, oneshot::Sender<Result<serde_json::Value, ClientError>>>;

/// Async RPC client over a unix socket.
///
/// Mirrors the C++ `simpleipc::client::service_client`: a background reader
/// task owns the socket read half and dispatches each `response`/`error` to
/// the pending one-shot channel registered under its message id. When the peer
/// closes the connection, every pending call fails with `connection_closed`.
/// The read and write halves are independent, so a blocked read never stalls
/// a concurrent write.
pub struct Client {
    writer: WriteHalf<UnixStream>,
    encoding: Arc<RwLock<Encoding>>,
    next_id: AtomicI64,
    pending: Arc<Mutex<PendingMap>>,
    reader: JoinHandle<()>,
}

fn spawn_reader(
    mut reader: ReadHalf<UnixStream>,
    encoding: Arc<RwLock<Encoding>>,
    pending: Arc<Mutex<PendingMap>>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut buf = BytesMut::with_capacity(4096);
        loop {
            let mut local = [0u8; 4096];
            let n = match reader.read(&mut local).await {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => break,
            };
            buf.extend_from_slice(&local[..n]);
            let enc = *encoding.read().await;

            loop {
                match enc.decode_message(&buf) {
                    Ok(Some((msg, consumed))) => {
                        buf.advance(consumed);
                        match msg {
                            Message::Response(resp) => {
                                if let Some(id) = resp.id {
                                    let mut p = pending.lock().await;
                                    if let Some(tx) = p.remove(&id) {
                                        let _ = tx.send(Ok(resp.result));
                                    }
                                }
                            }
                            Message::Error(err) => {
                                if let Some(id) = err.id {
                                    let mut p = pending.lock().await;
                                    if let Some(tx) = p.remove(&id) {
                                        let _ = tx.send(Err(ClientError::Rpc {
                                            code: err.error.code,
                                            message: err.error.message,
                                            data: err.error.data,
                                        }));
                                    }
                                }
                            }
                            Message::Rpc(_) => {}
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        log::warn!("simple-ipc: client decode error: {}", e);
                        let mut p = pending.lock().await;
                        for (_, tx) in p.drain() {
                            let _ = tx.send(Err(ClientError::Protocol(e.clone())));
                        }
                        break;
                    }
                }
            }
        }

        // Connection closed: fail all pending calls, like C++ `connection_closed`.
        let mut p = pending.lock().await;
        for (_, tx) in p.drain() {
            let _ = tx.send(Err(ClientError::ConnectionClosed));
        }
    })
}

impl Client {
    pub async fn connect(path: &str) -> Result<Self, ClientError> {
        Self::connect_with_preferred(path, crate::encoding::PREFERRED_ENCODINGS).await
    }

    /// Connect and negotiate an encoding, proposing the given preferred
    /// encodings (in order) during the `.hello` handshake.
    pub async fn connect_with_preferred(
        path: &str,
        preferred: &[&str],
    ) -> Result<Self, ClientError> {
        let stream = UnixStream::connect(path).await?;
        let (reader_half, writer_half) = tokio::io::split(stream);
        let encoding = Arc::new(RwLock::new(Encoding::Json));
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let reader = spawn_reader(reader_half, encoding.clone(), pending.clone());
        let mut client = Client {
            writer: writer_half,
            encoding,
            next_id: AtomicI64::new(1),
            pending,
            reader,
        };

        let hello = HelloRequest {
            version: PROTOCOL_VERSION,
            encodings: preferred.iter().map(|s| s.to_string()).collect(),
        };
        let hello_params = serde_json::to_value(&hello)
            .map_err(|e| ClientError::Protocol(format!("Serialize hello: {}", e)))?;

        let response = client.call_raw(".hello", hello_params).await?;

        let hello_resp: HelloResponse = serde_json::from_value(response)
            .map_err(|e| ClientError::Protocol(format!("Parse hello response: {}", e)))?;
        let negotiated = Encoding::from_name(&hello_resp.encoding)
            .ok_or_else(|| ClientError::Protocol(format!("Unknown encoding: {}", hello_resp.encoding)))?;
        *client.encoding.write().await = negotiated;

        Ok(client)
    }

    pub async fn call(&mut self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, ClientError> {
        self.call_raw(method, params).await
    }

    async fn call_raw(&mut self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, ClientError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();

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
        self.encoding.read().await.encode_message(&msg, &mut buf);
        self.writer.write_all(&buf).await?;

        match rx.await {
            Ok(result) => result,
            Err(_) => Err(ClientError::ConnectionClosed),
        }
    }

    pub async fn notify(&mut self, method: &str, params: serde_json::Value) -> Result<(), ClientError> {
        let msg = Message::Rpc(crate::message::RpcMessage {
            id: None,
            method: method.to_string(),
            params,
        });
        let mut buf = BytesMut::new();
        self.encoding.read().await.encode_message(&msg, &mut buf);
        self.writer.write_all(&buf).await?;
        Ok(())
    }

    pub async fn close(&mut self) -> Result<(), ClientError> {
        self.writer.shutdown().await?;
        Ok(())
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        // Stop the reader task. Dropping both halves closes the socket, and
        // dropping the pending map fails every in-flight call with
        // `ConnectionClosed` when its receiver observes the channel close.
        self.reader.abort();
    }
}
