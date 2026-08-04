//! Wire-parity golden tests against the C++ simpleipc encoding semantics,
//! plus in-process E2E client<->server round-trips over a unix socket.
//!
//! The expected byte sequences below were derived from the C++ encoders
//! (`encoding_json.cpp`, `encoding_json_cbor.cpp`, `varint.cpp`), which use
//! nlohmann::json compact dumps (JSON) and RFC 7049 CBOR with a base-128
//! LEB128 varint length prefix.

use bytes::{Buf, BytesMut};
use serde_json::json;
use tokio::net::UnixListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use simple_ipc::encoding::Encoding;
use simple_ipc::client::ClientError;
use simple_ipc::message::{
    ErrorBody, ErrorMessage, Message, ResponseMessage, RpcMessage,
};
use simple_ipc::{Client, Server};
use simple_ipc::varint;

// ---------------------------------------------------------------------------
// Error codes + protocol version parity with C++ `error_code.h` / `version.h`
// ---------------------------------------------------------------------------

#[test]
fn error_codes_match_cpp() {
    assert_eq!(simple_ipc::message::SUCCESS, 0);
    assert_eq!(simple_ipc::message::PARSE_ERROR, -32700);
    assert_eq!(simple_ipc::message::INVALID_REQUEST, -32600);
    assert_eq!(simple_ipc::message::METHOD_NOT_FOUND, -32601);
    assert_eq!(simple_ipc::message::INVALID_PARAMS, -32602);
    assert_eq!(simple_ipc::message::INTERNAL_ERROR, -32603);
    assert_eq!(simple_ipc::message::CONNECTION_CLOSED, -32000);
    assert_eq!(simple_ipc::message::NO_HELLO_REPLY, -32001);
    assert_eq!(simple_ipc::message::PROTOCOL_VERSION, 1);
}

// ---------------------------------------------------------------------------
// varint
// ---------------------------------------------------------------------------

#[test]
fn varint_encode_matches_cpp() {
    let mut out = [0u8; 10];
    let n = varint::encode_unsigned(0, &mut out);
    assert_eq!(&out[..n], &[0x00]);
    let n = varint::encode_unsigned(1, &mut out);
    assert_eq!(&out[..n], &[0x01]);
    let n = varint::encode_unsigned(127, &mut out);
    assert_eq!(&out[..n], &[0x7f]);
    let n = varint::encode_unsigned(128, &mut out);
    assert_eq!(&out[..n], &[0x80, 0x01]);
    let n = varint::encode_unsigned(300, &mut out);
    assert_eq!(&out[..n], &[0xac, 0x02]);
}

#[test]
fn varint_round_trip_large() {
    // 63-bit max = 9 bytes, the largest value the C++ decoder accepts.
    let mut out = [0u8; 10];
    let n = varint::encode_unsigned(0x7fff_ffff_ffff_ffff, &mut out);
    assert_eq!(n, 9);
    let (val, sz) = varint::decode_unsigned(&out[..n]).unwrap();
    assert_eq!(val, 0x7fff_ffff_ffff_ffff);
    assert_eq!(sz, n);
}

#[test]
fn varint_ten_byte_rejected_like_cpp() {
    // u64::MAX needs 10 bytes; the C++ encoder writes 10 bytes but the C++
    // decoder only inspects 9, so a 64-bit value never round-trips. The Rust
    // port rejects the same way.
    let mut out = [0u8; 10];
    let n = varint::encode_unsigned(u64::MAX, &mut out);
    assert_eq!(n, 10);
    assert!(varint::decode_unsigned(&out[..n]).is_err());
}

#[test]
fn varint_decode_handles_cpp_bytes() {
    // Decode byte sequences produced by the C++ encoder.
    assert_eq!(varint::decode_unsigned(&[0x00]).unwrap(), (0, 1));
    assert_eq!(varint::decode_unsigned(&[0x80, 0x01]).unwrap(), (128, 2));
    assert_eq!(varint::decode_unsigned(&[0xff, 0x7f]).unwrap(), (16383, 2));
}

// ---------------------------------------------------------------------------
// CBOR golden bytes (varint length prefix + nlohmann-compatible CBOR)
// ---------------------------------------------------------------------------

#[test]
fn cbor_rpc_golden_bytes() {
    let msg = Message::Rpc(RpcMessage {
        id: Some(1),
        method: "get".to_string(),
        params: json!({}),
    });
    let mut buf = BytesMut::new();
    Encoding::JsonCbor.encode_message(&msg, &mut buf);
    // [1, "get", {}] = 83 01 63 67 65 74 a0 (7 bytes), length varint = 07
    assert_eq!(&buf[..], &[0x07, 0x83, 0x01, 0x63, 0x67, 0x65, 0x74, 0xa0]);
}

#[test]
fn cbor_response_golden_bytes() {
    let msg = Message::Response(ResponseMessage {
        id: Some(2),
        result: json!(true),
    });
    let mut buf = BytesMut::new();
    Encoding::JsonCbor.encode_message(&msg, &mut buf);
    // [2, true] = 82 02 f5 (3 bytes), length varint = 03
    assert_eq!(&buf[..], &[0x03, 0x82, 0x02, 0xf5]);
}

#[test]
fn cbor_error_golden_bytes() {
    let msg = Message::Error(ErrorMessage {
        id: Some(3),
        error: ErrorBody {
            code: -32700,
            message: "Parse error".to_string(),
            data: json!(null),
        },
    });
    let mut buf = BytesMut::new();
    Encoding::JsonCbor.encode_message(&msg, &mut buf);
    // [3, -32700, "Parse error", null] =
    //   84 03 39 7f bb 6b 50 61 72 73 65 20 65 72 72 6f 72 f6 (18 bytes)
    let expected: &[u8] = &[
        0x12, 0x84, 0x03, 0x39, 0x7f, 0xbb, 0x6b, b'P', b'a', b'r', b's', b'e',
        b' ', b'e', b'r', b'r', b'o', b'r', 0xf6,
    ];
    assert_eq!(&buf[..], expected);
}

#[test]
fn cbor_rpc_without_id_golden_bytes() {
    let msg = Message::Rpc(RpcMessage {
        id: None,
        method: "get".to_string(),
        params: json!({}),
    });
    let mut buf = BytesMut::new();
    Encoding::JsonCbor.encode_message(&msg, &mut buf);
    // [null, "get", {}] = 83 f6 63 67 65 74 a0 (7 bytes), length varint = 07
    assert_eq!(&buf[..], &[0x07, 0x83, 0xf6, 0x63, 0x67, 0x65, 0x74, 0xa0]);
}

#[test]
fn cbor_decode_cpp_bytes() {
    // Bytes produced by the C++ encoder for [1, "get", {}].
    let bytes: &[u8] = &[0x07, 0x83, 0x01, 0x63, 0x67, 0x65, 0x74, 0xa0];
    let (msg, consumed) = Encoding::JsonCbor.decode_message(bytes).unwrap().unwrap();
    assert_eq!(consumed, bytes.len());
    match msg {
        Message::Rpc(rpc) => {
            assert_eq!(rpc.id, Some(1));
            assert_eq!(rpc.method, "get");
        }
        _ => panic!("expected RPC"),
    }
}

#[test]
fn cbor_decode_error_cpp_bytes() {
    let bytes: &[u8] = &[
        0x12, 0x84, 0x03, 0x39, 0x7f, 0xbb, 0x6b, b'P', b'a', b'r', b's', b'e',
        b' ', b'e', b'r', b'r', b'o', b'r', 0xf6,
    ];
    let (msg, consumed) = Encoding::JsonCbor.decode_message(bytes).unwrap().unwrap();
    assert_eq!(consumed, bytes.len());
    match msg {
        Message::Error(err) => {
            assert_eq!(err.id, Some(3));
            assert_eq!(err.error.code, -32700);
            assert_eq!(err.error.message, "Parse error");
        }
        _ => panic!("expected Error"),
    }
}

#[test]
fn cbor_partial_message_returns_none() {
    let bytes: &[u8] = &[0x07, 0x83, 0x01];
    assert!(Encoding::JsonCbor.decode_message(bytes).unwrap().is_none());
}

// ---------------------------------------------------------------------------
// JSON golden bytes (nlohmann key order: id, then method/result/error)
// ---------------------------------------------------------------------------

#[test]
fn json_rpc_golden_bytes() {
    let msg = Message::Rpc(RpcMessage {
        id: Some(1),
        method: "echo".to_string(),
        params: json!({"a": 1}),
    });
    let mut buf = BytesMut::new();
    Encoding::Json.encode_message(&msg, &mut buf);
    assert_eq!(
        String::from_utf8_lossy(&buf),
        "{\"id\":1,\"method\":\"echo\",\"params\":{\"a\":1}}\n"
    );
}

#[test]
fn json_error_golden_bytes() {
    let msg = Message::Error(ErrorMessage {
        id: Some(2),
        error: ErrorBody {
            code: -32000,
            message: "Connection was closed unexpectedly".to_string(),
            data: json!(null),
        },
    });
    let mut buf = BytesMut::new();
    Encoding::Json.encode_message(&msg, &mut buf);
    assert_eq!(
        String::from_utf8_lossy(&buf),
        "{\"id\":2,\"error\":{\"code\":-32000,\"message\":\"Connection was closed unexpectedly\",\"data\":null}}\n"
    );
}

#[test]
fn json_decode_cpp_bytes() {
    // C++-produced bytes (nlohmann order: id first).
    let bytes = b"{\"id\":1,\"method\":\"echo\",\"params\":{\"a\":1}}\n";
    let (msg, consumed) = Encoding::Json.decode_message(bytes).unwrap().unwrap();
    assert_eq!(consumed, bytes.len());
    match msg {
        Message::Rpc(rpc) => {
            assert_eq!(rpc.id, Some(1));
            assert_eq!(rpc.method, "echo");
            assert_eq!(rpc.params, json!({"a": 1}));
        }
        _ => panic!("expected RPC"),
    }
}

#[test]
fn json_decode_error_cpp_bytes() {
    let bytes = b"{\"id\":2,\"error\":{\"code\":-32000,\"message\":\"boom\",\"data\":null}}\n";
    let (msg, consumed) = Encoding::Json.decode_message(bytes).unwrap().unwrap();
    assert_eq!(consumed, bytes.len());
    match msg {
        Message::Error(err) => {
            assert_eq!(err.id, Some(2));
            assert_eq!(err.error.code, -32000);
            assert_eq!(err.error.message, "boom");
        }
        _ => panic!("expected Error"),
    }
}

#[test]
fn json_empty_line_is_parse_error() {
    assert!(Encoding::Json.decode_message(b"\n").is_err());
}

// ---------------------------------------------------------------------------
// Encoding negotiation (C++ default_rpc_handler::handle_hello semantics)
// ---------------------------------------------------------------------------

#[test]
fn pick_encoding_iterates_client_order() {
    // Client proposes json_cbor first -> json_cbor wins (default for both).
    assert_eq!(
        Encoding::pick_from_preferred(&["json_cbor".to_string(), "json".to_string()]),
        Some(Encoding::JsonCbor)
    );
    // Client prefers json -> C++ server picks json (its order, not the server's).
    assert_eq!(
        Encoding::pick_from_preferred(&["json".to_string(), "json_cbor".to_string()]),
        Some(Encoding::Json)
    );
    // Client only supports json.
    assert_eq!(
        Encoding::pick_from_preferred(&["json".to_string()]),
        Some(Encoding::Json)
    );
    // No common encoding.
    assert_eq!(
        Encoding::pick_from_preferred(&["xml".to_string()]),
        None
    );
}

// ---------------------------------------------------------------------------
// E2E client<->server over a unix socket
// ---------------------------------------------------------------------------

fn tmp_socket(name: &str) -> String {
    let pid = std::process::id();
    std::env::temp_dir()
        .join(format!("simpleipc-test-{}-{}.sock", pid, name))
        .to_string_lossy()
        .into_owned()
}

#[tokio::test]
async fn e2e_json_cbor() {
    let path = tmp_socket("cbor");
    let mut server = Server::bind(&path).await.unwrap();
    server.add_handler("echo", |params| Ok(params));
    tokio::spawn(async move {
        server.run().await.unwrap();
    });

    let mut client = Client::connect(&path).await.unwrap();
    let result = client.call("echo", json!({"hello": "world"})).await.unwrap();
    assert_eq!(result, json!({"hello": "world"}));

    let result = client.call("echo", json!([1, 2, 3])).await.unwrap();
    assert_eq!(result, json!([1, 2, 3]));

    client.close().await.unwrap();
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn e2e_json() {
    let path = tmp_socket("json");
    let mut server = Server::bind(&path).await.unwrap();
    server.add_handler("echo", |params| Ok(params));
    tokio::spawn(async move {
        server.run().await.unwrap();
    });

    let mut client = Client::connect_with_preferred(&path, &["json"]).await.unwrap();
    let result = client.call("echo", json!({"hello": "json"})).await.unwrap();
    assert_eq!(result, json!({"hello": "json"}));

    client.close().await.unwrap();
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn e2e_rpc_error_dispatch() {
    let path = tmp_socket("err");
    let mut server = Server::bind(&path).await.unwrap();
    server.add_handler("echo", |params| Ok(params));
    server.add_handler("boom", |_| Err((-32601, "Method not found".to_string())));
    tokio::spawn(async move {
        server.run().await.unwrap();
    });

    let mut client = Client::connect(&path).await.unwrap();
    let err = client.call("boom", json!(null)).await.unwrap_err();
    match err {
        ClientError::Rpc { code, message, .. } => {
            assert_eq!(code, -32601);
            assert_eq!(message, "Method not found");
        }
        other => panic!("expected Rpc error, got {:?}", other),
    }

    client.close().await.unwrap();
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn e2e_notify_is_ignored_by_server() {
    let path = tmp_socket("notify");
    let mut server = Server::bind(&path).await.unwrap();
    server.add_handler("nop", |params| Ok(params));
    tokio::spawn(async move {
        server.run().await.unwrap();
    });

    let mut client = Client::connect_with_preferred(&path, &["json"]).await.unwrap();
    client.notify("nop", json!({"a": 1})).await.unwrap();
    // A real call still works after a notification.
    let result = client.call("nop", json!({"b": 2})).await.unwrap();
    assert_eq!(result, json!({"b": 2}));

    client.close().await.unwrap();
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn connection_closed_fails_pending_calls() {
    let path = tmp_socket("close");
    let listener = UnixListener::bind(&path).unwrap();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = BytesMut::new();
        let mut tmp = [0u8; 4096];
        let mut saw_second_rpc = false;
        while !saw_second_rpc {
            if let Ok(Some((msg, consumed))) = Encoding::Json.decode_message(&buf) {
                buf.advance(consumed);
                match msg {
                    Message::Rpc(rpc) if rpc.method == ".hello" => {
                        let resp = Message::Response(ResponseMessage {
                            id: rpc.id,
                            result: json!({"version": 1, "encoding": "json"}),
                        });
                        let mut out = BytesMut::new();
                        Encoding::Json.encode_message(&resp, &mut out);
                        stream.write_all(&out).await.unwrap();
                    }
                    Message::Rpc(_) => {
                        // Second RPC seen: drop the connection without replying.
                        saw_second_rpc = true;
                    }
                    _ => {}
                }
            }
            if saw_second_rpc {
                break;
            }
            let n = stream.read(&mut tmp).await.unwrap();
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
        }
    });

    let mut client = Client::connect_with_preferred(&path, &["json"]).await.unwrap();
    // Server never replies; it closes the socket while this call is pending.
    let err = client.call("echo", json!(null)).await.unwrap_err();
    assert!(matches!(err, ClientError::ConnectionClosed), "got {:?}", err);
    let _ = std::fs::remove_file(&path);
}
