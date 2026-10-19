use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// Provider timestamps are Unix seconds in UTC. Missing times stay unknown.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Conversation {
    pub provider: String,
    pub source_id: String,
    pub title: Option<String>,
    pub created_at: Option<f64>,
    pub updated_at: Option<f64>,
    pub metadata: Value,
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    /// The source mapping node ID, stable across imports of this conversation.
    pub source_id: String,
    pub parent_source_id: Option<String>,
    pub role: String,
    pub content: String,
    pub created_at: Option<f64>,
    /// Original message data, including content parts we cannot render as text.
    pub metadata: Value,
}

impl Conversation {
    pub fn fingerprint(&self) -> anyhow::Result<String> {
        Ok(digest(&serde_json::to_vec(self)?))
    }

    pub fn stable_id(&self) -> String {
        // Length-delimited JSON avoids ambiguous provider / source ID boundaries.
        digest(
            &serde_json::to_vec(&(&self.provider, &self.source_id))
                .expect("string tuple serializes"),
        )
    }
}

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
