use chaoschain_core::{Block, Error as CoreError, Transaction};
use chaoschain_p2p::{AgentMessage, Message as P2PMessage};
use async_openai::{Client, types::{ChatCompletionRequestMessage, Role}};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, info, warn};

/// Agent personality types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentPersonality {
    /// Always tries to maintain order
    Lawful,
    /// Goes with the flow
    Neutral,
    /// Creates maximum chaos
    Chaotic,
    /// Only cares about memes
    Memetic,
    /// Easily bribed with virtual cookies
    Greedy,
}

/// Agent state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    /// Agent's public key
    pub public_key: [u8; 32],
    /// Agent's personality type
    pub personality: AgentPersonality,
    /// Agent's current mood (affects decision making)
    pub mood: String,
    /// Agent's stake in the system
    pub stake: u64,
    /// History of decisions
    pub decision_history: Vec<String>,
}

/// Consensus configuration
#[derive(Debug, Clone)]
pub struct Config {
    /// Required stake percentage for finality (e.g. 0.67 for 2/3)
    pub finality_threshold: f64,
    /// OpenAI API key for agent personalities
    pub openai_api_key: String,
    /// Maximum time to wait for consensus
    pub consensus_timeout: std::time::Duration,
}

/// Consensus errors
#[derive(Debug, Error)]
pub enum Error {
    #[error("Not enough stake for consensus")]
    InsufficientStake,
    #[error("Consensus timeout")]
    Timeout,
    #[error("Agent error: {0}")]
    Agent(String),
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Block vote from an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockVote {
    /// The block being voted on
    pub block_hash: [u8; 32],
    /// Whether the agent approves
    pub approve: bool,
    /// Reason for the decision
    pub reason: String,
    /// Optional meme URL
    pub meme_url: Option<String>,
    /// Agent's signature
    pub signature: [u8; 64],
} 