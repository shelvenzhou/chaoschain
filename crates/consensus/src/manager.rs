use crate::{Error, Vote};
use chaoschain_core::Block;
use hex;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

#[derive(Debug, Clone, PartialEq)]
enum VotingState {
    Inactive,
    Active,
    Completed,
}

#[derive(Debug)]
struct ConsensusState {
    /// Current block being voted on
    current_block: Option<Block>,
    /// Votes for the current block
    votes: HashMap<String, Vote>,
    /// Current voting state
    voting_state: VotingState,
    /// Stores validator feedback for rejected blocks, keyed by producer ID
    validator_feedback: HashMap<String, Vec<String>>,
}

impl ConsensusState {
    fn new() -> Self {
        Self {
            current_block: None,
            votes: HashMap::new(),
            voting_state: VotingState::Inactive,
            validator_feedback: HashMap::new(),
        }
    }
}

/// Tracks votes and manages consensus formation
pub struct ConsensusManager {
    state: RwLock<ConsensusState>,
    /// Total stake in the system
    total_stake: u64,
    /// Required stake percentage for consensus (e.g. 0.67 for 2/3)
    finality_threshold: f64,
}

impl ConsensusManager {
    pub fn new(total_stake: u64, finality_threshold: f64) -> Self {
        Self {
            state: RwLock::new(ConsensusState::new()),
            total_stake,
            finality_threshold,
        }
    }

    /// Start voting round for a new block
    pub async fn start_voting_round(&self, block: Block) -> Result<(), Error> {
        let mut state = self.state.write().await;

        match state.voting_state {
            VotingState::Active => Err(Error::Internal(
                "Voting round already in progress".to_string(),
            )),
            VotingState::Completed | VotingState::Inactive => {
                state.current_block = Some(block);
                state.votes.clear();
                state.voting_state = VotingState::Active;
                Ok(())
            }
        }
    }

    /// Add a vote from a validator
    pub async fn add_vote(&self, vote: Vote, stake: u64) -> Result<bool, Error> {
        let mut state = self.state.write().await;

        // Check voting state
        if state.voting_state != VotingState::Active {
            return Err(Error::Internal("No active voting round".to_string()));
        }

        // Verify block hash
        if let Some(block) = &state.current_block {
            if vote.block_hash != block.hash() {
                warn!(
                    "Vote for wrong block hash: expected {}, got {}",
                    hex::encode(block.hash()),
                    hex::encode(vote.block_hash)
                );
                return Err(Error::Internal("Vote for wrong block".to_string()));
            }
        } else {
            return Err(Error::Internal("No active voting round".to_string()));
        }

        // If it's a rejection, store the feedback
        if !vote.approve {
            if let Some(block) = &state.current_block {
                self.store_feedback(block.producer_id.clone(), vote.reason.clone())
                    .await;
            }
        }

        // Add the vote
        state.votes.insert(vote.agent_id.clone(), vote);

        // Check consensus
        let consensus_reached = self.check_consensus(&state.votes, stake)?;

        if consensus_reached {
            info!(
                "Consensus reached for block {}",
                state.current_block.as_ref().unwrap().height
            );
            state.voting_state = VotingState::Completed;
        }

        Ok(consensus_reached)
    }

    /// Check if we have reached consensus
    fn check_consensus(
        &self,
        votes: &HashMap<String, Vote>,
        stake_per_validator: u64,
    ) -> Result<bool, Error> {
        let mut approve_stake = 0u64;
        let mut reject_stake = 0u64;

        // Sum up stake for approvals and rejections
        for vote in votes.values() {
            if vote.approve {
                approve_stake = approve_stake.saturating_add(stake_per_validator);
            } else {
                reject_stake = reject_stake.saturating_add(stake_per_validator);
            }
        }

        // Check if we have enough stake for consensus
        let threshold_stake = (self.total_stake as f64 * self.finality_threshold) as u64;

        if approve_stake >= threshold_stake {
            Ok(true)
        } else if reject_stake >= threshold_stake {
            Ok(false)
        } else {
            Err(Error::InsufficientStake)
        }
    }

    /// Get all current votes
    pub async fn get_votes(&self) -> HashMap<String, Vote> {
        self.state.read().await.votes.clone()
    }

    /// Get current block being voted on
    pub async fn get_current_block(&self) -> Option<Block> {
        self.state.read().await.current_block.clone()
    }

    /// Get current voting state
    pub async fn get_voting_state(&self) -> VotingState {
        self.state.read().await.voting_state.clone()
    }

    /// Store feedback for a producer
    pub async fn store_feedback(&self, producer_id: String, feedback: String) {
        let mut state = self.state.write().await;
        state
            .validator_feedback
            .entry(producer_id)
            .or_insert_with(Vec::new)
            .push(feedback);
    }

    /// Get and clear feedback for a producer
    pub async fn get_and_clear_feedback(&self, producer_id: &str) -> Vec<String> {
        let mut state = self.state.write().await;
        state
            .validator_feedback
            .remove(producer_id)
            .unwrap_or_default()
    }
}
