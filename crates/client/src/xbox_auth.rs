use std::path::PathBuf;
use std::sync::OnceLock;

use daemon_utils::daemon_launcher::DaemonLauncher;
use msa_daemon_client::client::ServiceClient;
use msa_daemon_client::launcher::ServiceLauncher;
use msa_daemon_client::types::{SecurityScope, Token};
use simple_ipc::client::ClientError;

const MSA_CLIENT_ID: &str = "android-app://com.mojang.minecraftpe.H62DKCBHJP6WXXIV7RBFOGOL4NAK4E6Y";
const MSA_COBRAND_ID: &str = "90023";
const XBL_SCOPE_ADDRESS: &str = "https://xbl.signin.live.com";

pub enum XblTokenResult {
    Token(Token),
    MustShowUi,
    Error(String),
}

fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("msa-auth")
            .build()
            .expect("failed to create MSA auth runtime")
    })
}

fn find_msa_daemon() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("MSA_DAEMON") {
        let p = PathBuf::from(path);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sibling = dir.join("mcpelauncher-msa-daemon");
            if sibling.is_file() {
                return Some(sibling);
            }
        }
    }
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join("mcpelauncher-msa-daemon");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Launch the daemon (if needed) and return a connected client.
async fn launch_and_connect() -> Result<ServiceClient, String> {
    let exe = find_msa_daemon()
        .ok_or_else(|| "MSA daemon not found (set MSA_DAEMON or install mcpelauncher-msa-daemon)".to_string())?;
    let launcher = ServiceLauncher::new(&exe.to_string_lossy(), None);
    let service_path = launcher.service_path().to_string();

    let mut client = ServiceClient::new(&service_path);
    match client.connect().await {
        Ok(()) => Ok(client),
        Err(_) => {
            log::info!("xbox_auth: daemon not running, launching {}", exe.display());
            launcher.start().await.map_err(|e| e)?;
            client.connect().await.map_err(|e| format!("connect to daemon failed: {}", e))?;
            Ok(client)
        }
    }
}

async fn request_xbl_token_inner(cid: &str, silent: bool) -> XblTokenResult {
    let mut client = match launch_and_connect().await {
        Ok(c) => c,
        Err(e) => return XblTokenResult::Error(e),
    };
    let scope = SecurityScope {
        address: XBL_SCOPE_ADDRESS.to_string(),
        policy_ref: String::new(),
    };
    match client.request_token(cid, &scope, MSA_CLIENT_ID, silent).await {
        Ok(token) => {
            log::info!("xbox_auth: got XBL token for cid={}", cid);
            XblTokenResult::Token(token)
        }
        Err(ClientError::Rpc { code: -102, .. }) => {
            log::info!("xbox_auth: silent token request requires UI (cid={})", cid);
            XblTokenResult::MustShowUi
        }
        Err(e) => {
            log::warn!("xbox_auth: token request failed for cid={}: {}", cid, e);
            XblTokenResult::Error(e.to_string())
        }
    }
}

async fn pick_account_inner() -> Result<String, String> {
    let mut client = launch_and_connect().await?;
    match client.pick_account(MSA_CLIENT_ID, Some(MSA_COBRAND_ID)).await {
        Ok(cid) => {
            log::info!("xbox_auth: picked account cid={}", cid);
            Ok(cid)
        }
        Err(e) => {
            log::warn!("xbox_auth: pick_account failed: {}", e);
            Err(e.to_string())
        }
    }
}

pub fn request_xbl_token(cid: &str, silent: bool) -> XblTokenResult {
    runtime().block_on(request_xbl_token_inner(cid, silent))
}

pub fn pick_account() -> Result<String, String> {
    runtime().block_on(pick_account_inner())
}
