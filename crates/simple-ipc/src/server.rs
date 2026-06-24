use std::collections::HashMap;
use std::sync::Arc;

use bytes::{Buf, BytesMut};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};


use crate::encoding::Encoding;
use crate::message::{
    HelloRequest, HelloResponse, Message, ResponseMessage, ErrorMessage, ErrorBody,
    PROTOCOL_VERSION, METHOD_NOT_FOUND,
};

pub type RpcResult = Result<serde_json::Value, (i32, String)>;
pub type RpcHandlerFn =
    Arc<dyn Fn(serde_json::Value) -> RpcResult + Send + Sync>;

pub struct RpcHandler {
    handlers: HashMap<String, RpcHandlerFn>,
}

impl RpcHandler {
    pub fn new() -> Self {
        RpcHandler {
            handlers: HashMap::new(),
        }
    }

    pub fn add_handler<F>(&mut self, method: &str, handler: F)
    where
        F: Fn(serde_json::Value) -> RpcResult + Send + Sync + 'static,
    {
        self.handlers.insert(method.to_string(), Arc::new(handler));
    }

    fn invoke(&self, method: &str, params: serde_json::Value) -> RpcResult {
        match self.handlers.get(method) {
            Some(handler) => handler(params),
            None => Err((METHOD_NOT_FOUND, format!("Method not found: {}", method))),
        }
    }
}

struct Connection {
    stream: UnixStream,
    encoding: Encoding,
    handler: Arc<RpcHandler>,
    read_buf: BytesMut,
}

impl Connection {
    async fn handle_hello(&mut self, params: serde_json::Value) -> Result<(), String> {
        let hello: HelloRequest = serde_json::from_value(params)
            .map_err(|e| format!("Invalid hello: {}", e))?;
        let encoding = Encoding::pick_from_preferred(&hello.encodings)
            .ok_or_else(|| "No common encoding".to_string())?;
        self.encoding = encoding;

        let resp = HelloResponse {
            version: PROTOCOL_VERSION,
            encoding: encoding.name().to_string(),
        };
        let result = serde_json::to_value(&resp)
            .map_err(|e| format!("Serialize hello response: {}", e))?;

        let msg = Message::Response(ResponseMessage {
            id: None,
            result,
        });
        self.send_message(&msg).await
    }

    async fn send_message(&mut self, msg: &Message) -> Result<(), String> {
        let mut buf = BytesMut::new();
        self.encoding.encode_message(msg, &mut buf);
        self.stream.write_all(&buf).await
            .map_err(|e| format!("Write error: {}", e))
    }

    async fn run(&mut self) {
        loop {
            let mut local_buf = [0u8; 4096];
            let n = match self.stream.read(&mut local_buf).await {
                Ok(n) => n,
                Err(_) => return,
            };
            if n == 0 {
                return;
            }
            self.read_buf.extend_from_slice(&local_buf[..n]);

            loop {
                match self.encoding.decode_message(&self.read_buf) {
                    Ok(Some((msg, consumed))) => {
                        self.read_buf.advance(consumed);
                        if let Err(_) = self.handle_message(msg).await {
                            return;
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        log::error!("simple-ipc: decode error: {}", e);
                        return;
                    }
                }
            }
        }
    }

    async fn handle_message(&mut self, msg: Message) -> Result<(), ()> {
        match msg {
            Message::Rpc(rpc) => {
                if rpc.method == ".hello" {
                    if let Err(e) = self.handle_hello(rpc.params).await {
                        log::error!("simple-ipc: hello error: {}", e);
                        return Err(());
                    }
                    return Ok(());
                }

                let result = self.handler.invoke(&rpc.method, rpc.params);
                if let Some(id) = rpc.id {
                    match result {
                        Ok(value) => {
                            let msg = Message::Response(ResponseMessage {
                                id: Some(id),
                                result: value,
                            });
                            let _ = self.send_message(&msg).await;
                        }
                        Err((code, message)) => {
                            let msg = Message::Error(ErrorMessage {
                                id: Some(id),
                                error: ErrorBody {
                                    code,
                                    message,
                                    data: serde_json::Value::Null,
                                },
                            });
                            let _ = self.send_message(&msg).await;
                        }
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

pub struct Server {
    listener: UnixListener,
    handler: Arc<RpcHandler>,
}

impl Server {
    pub async fn bind(path: &str) -> Result<Self, std::io::Error> {
        // Remove stale socket file
        let _ = tokio::fs::remove_file(path).await;
        let listener = UnixListener::bind(path)?;
        Ok(Server {
            listener,
            handler: Arc::new(RpcHandler::new()),
        })
    }

    pub fn add_handler<F>(&mut self, method: &str, handler: F)
    where
        F: Fn(serde_json::Value) -> RpcResult + Send + Sync + 'static,
    {
        Arc::get_mut(&mut self.handler)
            .unwrap()
            .add_handler(method, handler);
    }

    pub async fn run(&mut self) -> Result<(), std::io::Error> {
        loop {
            let (stream, _) = self.listener.accept().await?;
            let handler = self.handler.clone();
            tokio::spawn(async move {
                let mut conn = Connection {
                    stream,
                    encoding: Encoding::Json,
                    handler,
                    read_buf: BytesMut::with_capacity(4096),
                };
                conn.run().await;
            });
        }
    }
}
