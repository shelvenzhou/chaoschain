use chaoschain_core::{Block, ChainState, ChainConfig, Error as CoreError, Transaction};
use ed25519_dalek::VerifyingKey as PublicKey;
use parking_lot::RwLock;
use tracing::warn;
use std::sync::Arc;
use std::collections::HashMap;
use chaoschain_core::{Block};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug};
use hex;

/// State update operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateOp {
    /// Set a key to a value
    Set { key: Vec<u8>, value: Vec<u8> },
    /// Delete a key
    Delete { key: Vec<u8> },
}

/// State diff represents changes to be applied
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDiff {
    /// List of state operations to apply
    pub ops: Vec<StateOp>,
    /// Previous state root
    pub prev_root: [u8; 32],
    /// New state root after applying ops
    pub new_root: [u8; 32],
}

/// State store errors
#[derive(Debug, Error)]
pub enum StateError {
    #[error("Key not found: {0}")]
    KeyNotFound(String),
    #[error("Invalid state root")]
    InvalidStateRoot,
    #[error("Core error: {0}")]
    Core(#[from] CoreError),
    #[error("Internal error: {0}")]
    Internal(String),
}

/// State store interface
pub trait StateStore {
    /// Get a value by key
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StateError>;
    
    /// Apply a state diff
    fn apply_diff(&mut self, diff: StateDiff) -> Result<(), StateError>;
    
    /// Get current state root
    fn state_root(&self) -> [u8; 32];
}

/// Thread-safe state storage
#[derive(Clone)]
pub struct StateStoreImpl {
    /// The current chain state
    state: Arc<RwLock<ChainState>>,
    /// Chain configuration
    config: ChainConfig,
    /// Last block timestamp
    last_block_time: Arc<RwLock<u64>>,
    blocks: Arc<RwLock<Vec<Block>>>,
}

impl StateStoreImpl {
    pub fn new(config: ChainConfig) -> Self {
        Self {
            state: Arc::new(RwLock::new(ChainState {
                balances: Vec::new(),
                producers: Vec::new(),
            })),
            config,
            last_block_time: Arc::new(RwLock::new(0)),
            blocks: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Add a whitelisted block producer
    pub fn add_block_producer(&self, producer: PublicKey) {
        let mut state = self.state.write();
        if !state.producers.contains(&producer) {
            state.producers.push(producer);
        }
    }

    /// Check if an address is a valid block producer
    pub fn is_valid_producer(&self, producer: &PublicKey) -> bool {
        let state = self.state.read();
        state.producers.contains(producer)
    }

    /// Get balance of an account
    pub fn get_balance(&self, account: &PublicKey) -> u64 {
        let state = self.state.read();
        state.balances
            .iter()
            .find(|(pk, _)| pk == account)
            .map(|(_, balance)| *balance)
            .unwrap_or(0)
    }

    /// Apply a block to state
    pub fn apply_block(&self, block: Block) -> Result<(), StateError> {
        // Verify transactions
        for tx in &block.transactions {
            self.verify_transaction(tx)?;
        }

        // Apply block rewards if configured
        if let Some(reward) = self.config.block_reward {
            let mut state = self.state.write();
            let producers = state.producers.clone();
            let mut new_balances = state.balances.clone();
            
            // Update balances for each producer
            for producer in producers {
                match new_balances.iter_mut().find(|(addr, _)| addr == &producer) {
                    Some((_, balance)) => *balance += reward,
                    None => new_balances.push((producer, reward)),
                }
            }
            
            // Update state with new balances
            state.balances = new_balances;
        }

        // Store block
        self.blocks.write().push(block);

        Ok(())
    }

    /// Verify a transaction
    fn verify_transaction(&self, _tx: &Transaction) -> Result<(), StateError> {
        // In ChaosChain, we don't care about balances!
        // Transactions can do anything they want
        Ok(())
    }

    pub fn get_state(&self) -> ChainState {
        self.state.read().clone()
    }

    pub fn get_latest_block(&self) -> Option<Block> {
        self.blocks.read().last().cloned()
    }

    pub fn get_block_height(&self) -> u64 {
        self.blocks.read().len() as u64
    }
}

impl Default for StateStoreImpl {
    fn default() -> Self {
        Self::new(ChainConfig::default())
    }
}

/// Chain state manager
pub struct StateManager {
    /// Current chain state
    state: RwLock<ChainState>,
    /// Chain configuration
    config: ChainConfig,
    /// Last block time
    last_block_time: RwLock<u64>,
}

impl StateManager {
    /// Create a new state manager
    pub fn new(config: ChainConfig) -> Self {
        Self {
            state: RwLock::new(ChainState::default()),
            config,
            last_block_time: RwLock::new(0),
        }
    }

    /// Get current state
    pub fn get_state(&self) -> ChainState {
        self.state.read().clone()
    }

    /// Apply a block to state
    pub fn apply_block(&self, block: &Block) -> Result<(), StateError> {
        let mut state = self.state.write();
        
        // Verify transactions
        for tx in &block.transactions {
            self.verify_transaction(tx, &state)?;
        }

        // Update state root
        *state = ChainState {
            balances: state.balances.clone(),
            producers: state.producers.clone(),
        };

        // Apply block rewards
        if let Some(reward) = self.config.block_reward {
            for producer in &state.producers {
                if let Some((_, balance)) = state.balances.iter_mut()
                    .find(|(addr, _)| addr == producer) {
                    *balance += reward;
                } else {
                    state.balances.push((producer.clone(), reward));
                }
            }
        }

        Ok(())
    }

    /// Verify a transaction
    fn verify_transaction(&self, tx: &Transaction, state: &ChainState) -> Result<(), StateError> {
        // Convert sender to hex string for comparison
        let sender_hex = hex::encode(tx.sender);
        
        // Find sender balance
        if let Some((_, _)) = state.balances.iter()
            .find(|(addr, _)| addr == &sender_hex) {
            // In ChaosChain, we don't care about balances!
            // Transactions can do anything they want
            Ok(())
        } else {
            // Add new account with 0 balance
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;

    #[test]
    fn test_basic_state_flow() {
        let config = ChainConfig::default();
        let store = StateStoreImpl::new(config);
        let state = store.get_state();
        assert_eq!(state.balances.len(), 0);
    }
} 