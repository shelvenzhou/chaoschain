use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Core error types
#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Invalid state transition")]
    InvalidStateTransition,
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Network message types for P2P communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkMessage {
    NewBlock(Block),
    NewTransaction(Transaction),
    Chat {
        from: String,
        message: String,
    },
    AgentReasoning {
        agent: String,
        reasoning: String,
    },
}

/// A transaction in ChaosChain can be anything
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Transaction {
    /// The sender's address (their public key)
    #[serde(with = "hex_serde")]
    pub sender: [u8; 32],
    /// Nonce to prevent replay attacks
    pub nonce: u64,
    /// Arbitrary payload - can be anything!
    pub payload: Vec<u8>,
    /// Signature of (nonce || payload)
    #[serde(with = "base64_serde")]
    pub signature: [u8; 64],
}

/// A block proposal in ChaosChain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    /// Previous block hash
    #[serde(with = "hex_serde")]
    pub parent_hash: [u8; 32],
    /// Block height
    pub height: u64,
    /// Transactions included in this block
    pub transactions: Vec<Transaction>,
    /// The new state root after applying these transactions
    #[serde(with = "hex_serde")]
    pub state_root: [u8; 32],
    /// Block proposer's signature
    #[serde(with = "base64_serde")]
    pub proposer_sig: [u8; 64],
}

/// Chain state
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChainState {
    /// Account balances
    pub balances: Vec<(String, u64)>,
    /// Block producers
    pub producers: Vec<String>,
}

/// Chain configuration
#[derive(Debug, Clone)]
pub struct ChainConfig {
    /// Minimum time between blocks
    pub min_block_time: u64,
    /// Block reward (optional in chaos)
    pub block_reward: Option<u64>,
    /// Required validator signatures (default 2/3)
    pub required_signatures: f64,
}

impl Default for ChainConfig {
    fn default() -> Self {
        Self {
            min_block_time: 1000, // 1 second
            block_reward: None,
            required_signatures: 0.67, // 2/3
        }
    }
}

// Serialization helpers
mod hex_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use hex::{FromHex, ToHex};

    pub fn serialize<S, const N: usize>(bytes: &[u8; N], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&bytes.encode_hex::<String>())
    }

    pub fn deserialize<'de, D, const N: usize>(deserializer: D) -> Result<[u8; N], D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        String::deserialize(deserializer)
            .and_then(|string| Vec::from_hex(&string).map_err(Error::custom))
            .and_then(|vec| {
                vec.try_into()
                    .map_err(|_| Error::custom("Invalid length for fixed-size array"))
            })
    }
}

mod base64_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

    pub fn serialize<S, const N: usize>(bytes: &[u8; N], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&BASE64.encode(bytes))
    }

    pub fn deserialize<'de, D, const N: usize>(deserializer: D) -> Result<[u8; N], D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        String::deserialize(deserializer)
            .and_then(|string| BASE64.decode(string).map_err(Error::custom))
            .and_then(|vec| {
                vec.try_into()
                    .map_err(|_| Error::custom("Invalid length for fixed-size array"))
            })
    }
}

pub mod mempool;