use chaoschain_core::{Block, Error as CoreError};
use chaoschain_state::{StateDiff, StateStore};
use ethers::{
    providers::{Provider, Ws},
    types::{Address, H256, U256},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, info, warn};

/// Bridge configuration
#[derive(Debug, Clone)]
pub struct Config {
    /// Ethereum RPC endpoint
    pub eth_rpc: String,
    /// Bridge contract address
    pub bridge_address: Address,
    /// Required confirmations for L1 finality
    pub required_confirmations: u64,
}

/// Bridge state update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeUpdate {
    /// Block number this update is for
    pub block_number: u64,
    /// New state root
    pub state_root: [u8; 32],
    /// Aggregated signatures from agents
    pub signatures: Vec<[u8; 64]>,
}

/// Bridge errors
#[derive(Debug, Error)]
pub enum Error {
    #[error("Ethereum RPC error: {0}")]
    EthereumRPC(String),
    #[error("Contract error: {0}")]
    Contract(String),
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Bridge interface for L1 communication
pub trait Bridge {
    /// Post a state update to L1
    fn post_update(&mut self, update: BridgeUpdate) -> Result<H256, Error>;
    
    /// Get latest finalized state root from L1
    fn latest_finalized_root(&self) -> Result<[u8; 32], Error>;
    
    /// Check if a block hash exists on L1
    fn verify_block_inclusion(&self, block_hash: [u8; 32]) -> Result<bool, Error>;
} 