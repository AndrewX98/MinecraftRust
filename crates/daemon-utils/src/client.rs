use simple_ipc::client::{Client, ClientError};

pub struct LaunchableServiceClient {
    path: String,
    client: Option<Client>,
}

impl LaunchableServiceClient {
    pub fn new(path: &str) -> Self {
        LaunchableServiceClient {
            path: path.to_string(),
            client: None,
        }
    }

    pub async fn connect(&mut self) -> Result<(), ClientError> {
        self.client = Some(Client::connect(&self.path).await?);
        Ok(())
    }

    pub async fn call(&mut self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, ClientError> {
        if self.client.is_none() {
            self.connect().await?;
        }
        self.client.as_mut().unwrap().call(method, params).await
    }

    pub async fn notify(&mut self, method: &str, params: serde_json::Value) -> Result<(), ClientError> {
        if self.client.is_none() {
            self.connect().await?;
        }
        self.client.as_mut().unwrap().notify(method, params).await
    }

    pub async fn close(&mut self) -> Result<(), ClientError> {
        if let Some(client) = self.client.as_mut() {
            client.close().await
        } else {
            Ok(())
        }
    }
}
