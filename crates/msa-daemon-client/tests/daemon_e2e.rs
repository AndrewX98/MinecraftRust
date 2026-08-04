//! End-to-end tests against a real `mcpelauncher-msa-daemon` process.
//!
//! Ignored by default — they need the daemon binary. Each test launches its
//! own daemon in a private data dir via `ServiceLauncher`/`DaemonLauncher` (the
//! same path `crates/client/src/xbox_auth.rs` uses), then drives it through the
//! Rust `simple-ipc` RPC stack.
//!
//! ```sh
//! MSA_DAEMON_BIN=/path/to/mcpelauncher-msa-daemon \
//!   cargo test -p msa-daemon-client --test daemon_e2e -- --ignored --nocapture
//! ```
//!
//! (Set `MSA_DAEMON_BIN` to any daemon binary; e.g. the one shipped by the
//! mcpelauncher Flatpak bundle.)

use daemon_utils::daemon_launcher::DaemonLauncher;
use msa_daemon_client::client::ServiceClient;
use msa_daemon_client::launcher::ServiceLauncher;
use msa_daemon_client::types::SecurityScope;

const MSA_CLIENT_ID: &str = "android-app://com.mojang.minecraftpe.H62DKCBHJP6WXXIV7RBFOGOL4NAK4E6Y";

/// Launch a fresh daemon in a unique temp data dir and connect a client to it.
async fn fresh_client() -> ServiceClient {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let exe = std::env::var("MSA_DAEMON_BIN").expect("set MSA_DAEMON_BIN to the daemon binary");
    let data = std::env::temp_dir()
        .join(format!("msad-test-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    let launcher = ServiceLauncher::new(&exe, Some(data.to_str().unwrap()));
    let path = launcher.service_path().to_string();
    launcher.start().await.expect("spawn + wait for socket");
    let mut client = ServiceClient::new(&path);
    client.connect().await.expect("connect + .hello handshake");
    client
}

#[tokio::test]
#[ignore]
async fn hello_and_get_accounts() {
    let mut client = fresh_client().await;
    let accounts = client.get_accounts().await.expect("msa/get_accounts");
    println!("accounts: {accounts:?}");
    assert!(accounts.is_empty(), "fresh data dir should have no accounts");
}

#[tokio::test]
#[ignore]
async fn request_token_for_unknown_cid_fails() {
    let mut client = fresh_client().await;
    let scope = SecurityScope {
        address: "https://xbl.signin.live.com".to_string(),
        policy_ref: String::new(),
    };
    let err = client
        .request_token("00000000-0000-0000-0000-000000000000", &scope, MSA_CLIENT_ID, true)
        .await
        .expect_err("unknown cid must not yield a token");
    println!("expected error: {err}");
    assert!(err.to_string().contains("No such account"), "unexpected: {err}");
}