use crate::{Error, Vote};
use chaoschain_core::Block;
use hex;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};
use tracing::{debug, error, info, warn};

/// Represents the current state of voting
#[derive(Debug, Clone, PartialEq)]
enum VotingState {
    Inactive,
    Active,
    Completed,
}

/// Internal state maintained by the consensus manager
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

/// Messages that can be sent to the consensus manager
#[derive(Debug)]
enum ConsensusMessage {
    /// Start a new voting round for a block
    StartVoting(Block),
    /// Submit a vote with associated stake
    Vote(Vote, u64, oneshot::Sender<Result<bool, Error>>),
    /// Get the current block being voted on
    GetCurrentBlock(oneshot::Sender<Option<Block>>),
    /// Get all current votes
    GetVotes(oneshot::Sender<HashMap<String, Vote>>),
    /// Get and clear feedback for a producer
    GetAndClearFeedback(String, oneshot::Sender<Vec<String>>),
    /// Store feedback for a producer
    StoreFeedback(String, String),
}

/// Manages the consensus process through message passing
pub struct ConsensusManager {
    /// Channel for sending consensus messages
    tx: mpsc::Sender<ConsensusMessage>,
    /// Shared consensus state
    state: Arc<RwLock<ConsensusState>>,
    /// Total stake in the system
    total_stake: u64,
    /// Required stake percentage for consensus (e.g. 0.67 for 2/3)
    finality_threshold: f64,
}

impl ConsensusManager {
    /// Creates a new consensus manager with the specified parameters
    pub fn new(total_stake: u64, finality_threshold: f64) -> Self {
        let (tx, mut rx) = mpsc::channel(100);
        let state = Arc::new(RwLock::new(ConsensusState::new()));
        let state_clone = state.clone();

        // Spawn background task to handle consensus messages
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                match msg {
                    ConsensusMessage::StartVoting(block) => {
                        let mut state = state_clone.write().await;
                        debug!("Starting new voting round for block {}", block.height);
                        state.current_block = Some(block);
                        state.votes.clear();
                        state.voting_state = VotingState::Active;
                    }
                    ConsensusMessage::Vote(vote, stake, resp) => {
                        let mut state = state_clone.write().await;

                        // Process vote and check for consensus
                        let result = Self::process_vote(
                            &mut state,
                            vote,
                            stake,
                            total_stake,
                            finality_threshold,
                        );
                        let _ = resp.send(result);
                    }
                    ConsensusMessage::GetCurrentBlock(resp) => {
                        let state = state_clone.read().await;
                        let _ = resp.send(state.current_block.clone());
                    }
                    ConsensusMessage::GetVotes(resp) => {
                        let state = state_clone.read().await;
                        let _ = resp.send(state.votes.clone());
                    }
                    ConsensusMessage::StoreFeedback(producer_id, feedback) => {
                        let mut state = state_clone.write().await;
                        state
                            .validator_feedback
                            .entry(producer_id)
                            .or_insert_with(Vec::new)
                            .push(feedback);
                    }
                    ConsensusMessage::GetAndClearFeedback(producer_id, resp) => {
                        let mut state = state_clone.write().await;
                        let feedback = state
                            .validator_feedback
                            .remove(&producer_id)
                            .unwrap_or_default();
                        let _ = resp.send(feedback);
                    }
                }
            }
        });

        Self {
            tx,
            state,
            total_stake,
            finality_threshold,
        }
    }

    /// Starts a new voting round for the given block
    pub async fn start_voting_round(&self, block: Block) -> Result<(), Error> {
        debug!(
            "Requesting to start voting round for block {}",
            block.height
        );

        // Check current voting state before proceeding
        let current_state = {
            let state = self.state.read().await;
            state.voting_state.clone()
        };

        // If there's an active voting round, return an error
        match current_state {
            VotingState::Active => {
                return Err(Error::Internal(
                    "Cannot start new voting round while previous round is active".to_string(),
                ));
            }
            VotingState::Completed => {
                debug!("Previous voting round was completed, starting new round");
            }
            VotingState::Inactive => {
                debug!("No active voting round, starting new round");
            }
        }

        // Proceed with starting the new voting round
        self.tx
            .send(ConsensusMessage::StartVoting(block))
            .await
            .map_err(|_| Error::Internal("Failed to start voting round".to_string()))
    }

    /// Adds a vote from a validator with the specified stake
    pub async fn add_vote(&self, vote: Vote, stake: u64) -> Result<bool, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(ConsensusMessage::Vote(vote, stake, tx))
            .await
            .map_err(|_| Error::Internal("Failed to submit vote".to_string()))?;

        rx.await
            .map_err(|_| Error::Internal("Failed to get vote result".to_string()))?
    }

    /// Gets the current block being voted on
    pub async fn get_current_block(&self) -> Option<Block> {
        let (tx, rx) = oneshot::channel();
        if let Ok(_) = self.tx.send(ConsensusMessage::GetCurrentBlock(tx)).await {
            rx.await.unwrap_or(None)
        } else {
            None
        }
    }

    /// Gets all current votes
    pub async fn get_votes(&self) -> HashMap<String, Vote> {
        let (tx, rx) = oneshot::channel();
        if let Ok(_) = self.tx.send(ConsensusMessage::GetVotes(tx)).await {
            rx.await.unwrap_or_default()
        } else {
            HashMap::new()
        }
    }

    /// Stores feedback for a block producer
    pub async fn store_feedback(&self, producer_id: String, feedback: String) {
        let _ = self
            .tx
            .send(ConsensusMessage::StoreFeedback(producer_id, feedback))
            .await;
    }

    /// Gets and clears feedback for a producer
    pub async fn get_and_clear_feedback(&self, producer_id: &str) -> Vec<String> {
        let (tx, rx) = oneshot::channel();
        if let Ok(_) = self
            .tx
            .send(ConsensusMessage::GetAndClearFeedback(
                producer_id.to_string(),
                tx,
            ))
            .await
        {
            rx.await.unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    /// Internal helper to process a vote and check for consensus
    fn process_vote(
        state: &mut ConsensusState,
        vote: Vote,
        stake: u64,
        total_stake: u64,
        finality_threshold: f64,
    ) -> Result<bool, Error> {
        // Verify voting state
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

        // Add the vote
        state.votes.insert(vote.agent_id.clone(), vote);

        // Check consensus
        let mut approve_stake = 0u64;
        let mut reject_stake = 0u64;

        for vote in state.votes.values() {
            if vote.approve {
                approve_stake = approve_stake.saturating_add(stake);
            } else {
                reject_stake = reject_stake.saturating_add(stake);
            }
        }

        let threshold_stake = (total_stake as f64 * finality_threshold) as u64;

        let consensus_reached = if approve_stake >= threshold_stake {
            state.voting_state = VotingState::Completed;
            true
        } else if reject_stake >= threshold_stake {
            state.voting_state = VotingState::Completed;
            false
        } else {
            return Err(Error::InsufficientStake);
        };

        Ok(consensus_reached)
    }
}
