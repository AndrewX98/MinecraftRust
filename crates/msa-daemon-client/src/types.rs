use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct BaseAccountInfo {
    pub username: String,
    pub cid: String,
}

impl BaseAccountInfo {
    pub fn from_json(j: &serde_json::Value) -> Self {
        BaseAccountInfo {
            username: j["username"].as_str().unwrap_or("").to_string(),
            cid: j["cid"].as_str().unwrap_or("").to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SecurityScope {
    pub address: String,
    pub policy_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenType {
    Legacy,
    Compact,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub token_type: TokenType,
    pub scope: SecurityScope,
    pub created_time: DateTime<Utc>,
    pub expires_time: DateTime<Utc>,
    // Type-specific data
    pub xml_data: Option<String>,
    pub binary_secret: Option<String>,
    pub binary_token: Option<String>,
}

impl Token {
    pub fn from_json(j: &serde_json::Value) -> Self {
        let token_type = match j["type"].as_str() {
            Some("urn:passport:legacy") => TokenType::Legacy,
            _ => TokenType::Compact,
        };
        let scope = SecurityScope {
            address: j["scope"]["address"].as_str().unwrap_or("").to_string(),
            policy_ref: j["scope"]["policy_ref"].as_str().unwrap_or("").to_string(),
        };
        let created = parse_time(j["created"].as_str().unwrap_or(""));
        let expires = parse_time(j["expires"].as_str().unwrap_or(""));

        Token {
            token_type,
            scope,
            created_time: created,
            expires_time: expires,
            xml_data: j["xml_data"].as_str().map(|s| s.to_string()),
            binary_secret: j["binary_secret"].as_str().map(|s| s.to_string()),
            binary_token: j["binary_token"].as_str().map(|s| s.to_string()),
        }
    }
}

fn parse_time(s: &str) -> DateTime<Utc> {
    // Try parsing ISO 8601 format (with Z or +00:00)
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return dt.with_timezone(&Utc);
    }
    // Fallback
    Utc::now()
}

pub struct LegacyToken(pub Token);
pub struct CompactToken(pub Token);
