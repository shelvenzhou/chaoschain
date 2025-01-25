use serde::{Deserialize, Serialize};
use ed25519_dalek::{Signature, PublicKey};

/// A transaction in ChaosChain is just a signed blob of arbitrary data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// The sender's public key
    pub sender: PublicKey,
    /// Monotonically increasing nonce to prevent replay attacks
    pub nonce: u64,
    /// Arbitrary payload - interpretation is up to the agents
    pub payload: Vec<u8>,
    /// Signature of (nonce || payload)
    pub signature: Signature,
}

/// A proposed state change in ChaosChain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDiff {
    /// Key-value pairs to update in the state
    pub updates: Vec<(Vec<u8>, Vec<u8>)>,
    /// Keys to delete from the state
    pub deletions: Vec<Vec<u8>>,
}

/// A block in ChaosChain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    /// Height of this block
    pub height: u64,
    /// Hash of the parent block
    pub parent_hash: [u8; 32],
    /// Transactions included in this block
    pub transactions: Vec<Transaction>,
    /// Proposed state changes
    pub state_diff: StateDiff,
    /// Unix timestamp
    pub timestamp: u64,
    /// Block proposer's public key
    pub proposer: PublicKey,
}

/// An agent's vote on a block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockVote {
    /// The block being voted on
    pub block_hash: [u8; 32],
    /// The agent's public key
    pub agent: PublicKey,
    /// Whether the agent approves the block
    pub approve: bool,
    /// Optional reason/message
    pub message: Option<String>,
    /// Signature of the vote
    pub signature: Signature,
} 