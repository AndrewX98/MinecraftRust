use daemon_utils::client::LaunchableServiceClient;
use simple_ipc::client::ClientError;
use crate::types::{BaseAccountInfo, SecurityScope, Token};

pub struct ServiceClient {
    client: LaunchableServiceClient,
}

impl ServiceClient {
    pub fn new(path: &str) -> Self {
        ServiceClient {
            client: LaunchableServiceClient::new(path),
        }
    }

    pub async fn connect(&mut self) -> Result<(), ClientError> {
        self.client.connect().await
    }

    pub async fn get_accounts(&mut self) -> Result<Vec<BaseAccountInfo>, ClientError> {
        let result = self.client.call("msa/get_accounts", serde_json::Value::Null).await?;
        let accounts = result["accounts"]
            .as_array()
            .map(|arr| arr.iter().map(BaseAccountInfo::from_json).collect())
            .unwrap_or_default();
        Ok(accounts)
    }

    pub async fn add_account(
        &mut self,
        cid: &str,
        puid: &str,
        username: &str,
        token: &str,
    ) -> Result<(), ClientError> {
        let params = serde_json::json!({
            "cid": cid,
            "puid": puid,
            "username": username,
            "token": token,
        });
        self.client.call("msa/add_account", params).await?;
        Ok(())
    }

    pub async fn remove_account(&mut self, cid: &str) -> Result<(), ClientError> {
        let params = serde_json::json!({ "cid": cid });
        self.client.call("msa/remove_account", params).await?;
        Ok(())
    }

    pub async fn pick_account(
        &mut self,
        client_id: &str,
        cobrand_id: Option<&str>,
    ) -> Result<String, ClientError> {
        let mut params = serde_json::json!({ "client_id": client_id });
        if let Some(cobrand) = cobrand_id {
            params["cobrandid"] = serde_json::Value::String(cobrand.to_string());
        }
        let result = self.client.call("msa/pick_account", params).await?;
        Ok(result["cid"].as_str().unwrap_or("").to_string())
    }

    pub async fn request_token(
        &mut self,
        cid: &str,
        scope: &SecurityScope,
        client_id: &str,
        silent: bool,
    ) -> Result<Token, ClientError> {
        let params = serde_json::json!({
            "cid": cid,
            "scope": {
                "address": scope.address,
                "policy_ref": scope.policy_ref,
            },
            "client_id": client_id,
            "silent": silent,
        });
        let result = self.client.call("msa/request_token", params).await?;
        Ok(Token::from_json(&result))
    }
}
