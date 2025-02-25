use anyhow::Result;
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
        CreateChatCompletionRequest, Role,
    },
    Client,
};
use async_trait::async_trait;
use chaoschain_consensus::ConsensusManager;
use chaoschain_core::{Block, NetworkEvent, Transaction};
use chaoschain_p2p::Message as P2PMessage;
use chaoschain_state::{StateStore, StateStoreImpl};
use ed25519_dalek::{ed25519::signature::rand_core::block, Signer, SigningKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use std::{collections::HashMap, sync::Arc};
use thiserror::Error;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebMessage {
    DramaEvent(String),
    BlockEvent(Block),
    TransactionEvent(Transaction),
}

/// Block production style based on mood
#[derive(Debug, Clone)]
enum ProductionStyle {
    Chaotic,   // Random transaction selection
    Dramatic,  // Prioritize dramatic transactions
    Strategic, // Try to please specific validators
    Whimsical, // Randomly switch between styles
}

/// Messages that the producer particle can handle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProducerMessage {
    /// New transaction available in mempool
    NewTransaction(Transaction),
    /// Time to try producing a block
    TryProduceBlock,
    /// Validator feedback on block
    ValidatorFeedback { from: String, message: String },
    /// Social interaction with other producers
    SocialInteraction { from: String, action: String },
}

/// Configuration for the block producer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProducerConfig {
    /// Maximum transactions per block
    pub max_transactions: usize,
    /// Maximum block size in bytes
    pub max_block_size: usize,
    /// Target block time in seconds
    pub target_block_time: u64,
}

impl Default for ProducerConfig {
    fn default() -> Self {
        Self {
            max_transactions: 100,
            max_block_size: 1_000_000,
            target_block_time: 1000,
        }
    }
}

/// Producer statistics
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ProducerStats {
    pub blocks_produced: u64,
    pub transactions_processed: u64,
    pub drama_level: u8,
    pub avg_block_time: f64,
    pub ai_interactions: u64,
}

/// Producer errors
#[derive(Debug, Error)]
pub enum Error {
    #[error("Not enough transactions")]
    InsufficientTransactions,
    #[error("Block production failed: {0}")]
    Production(String),
    #[error("AI error: {0}")]
    AI(#[from] async_openai::error::OpenAIError),
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("Other error: {0}")]
    Other(String),
}

pub struct Producer {
    pub id: String,
    pub system_prompt: String,
    pub state: Arc<StateStoreImpl>,
    pub openai: Client<OpenAIConfig>,
    pub tx: broadcast::Sender<NetworkEvent>,
    pub signing_key: SigningKey,
    consensus: Arc<ConsensusManager>,
}

impl Producer {
    pub fn new(
        id: String,
        system_prompt: String,
        state: Arc<StateStoreImpl>,
        openai: Client<OpenAIConfig>,
        tx: broadcast::Sender<NetworkEvent>,
        consensus: Arc<ConsensusManager>,
    ) -> Self {
        // Generate a new keypair for signing
        let signing_key = SigningKey::generate(&mut OsRng);

        Self {
            id,
            system_prompt,
            state,
            openai,
            tx,
            signing_key,
            consensus,
        }
    }

    pub async fn generate_block(&self) -> Result<Block, Error> {
        // Get any feedback from previous blocks
        let feedback = self.consensus.get_and_clear_feedback(&self.id).await;

        // Get recent messages for context
        let recent_messages = self.state.get_recent_messages(5);

        // Build context string including feedback
        let context = {
            let mut context = String::new();

            if !feedback.is_empty() {
                context.push_str("Validator feedback from previous blocks:\n");
                for (i, f) in feedback.iter().enumerate() {
                    context.push_str(&format!("Feedback {}: {}\n", i + 1, f));
                }
                context.push_str("\n");
            }

            if !recent_messages.is_empty() {
                context.push_str("Recent messages:\n");
                for (i, msg) in recent_messages.iter().enumerate() {
                    context.push_str(&format!("Message {}: {}\n", recent_messages.len() - i, msg));
                }
            } else {
                context.push_str("No previous messages available.");
            }

            context
        };

        // Create system message with context and feedback
        let system_content = format!(
            "{}\n\nConsider the following context and feedback:\n{}",
            self.system_prompt,
            context
        );

        let system_message =
            ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                content: system_content,
                role: Role::System,
                name: None,
            });

        let request = CreateChatCompletionRequest {
            model: "gpt-4o".to_string(),
            messages: vec![system_message],
            temperature: Some(0.9), // Higher temperature for more creative responses
            max_tokens: Some(200),
            presence_penalty: Some(0.7),  // Encourage novel responses
            frequency_penalty: Some(0.7), // Discourage repetition
            ..Default::default()
        };

        let response = self
            .openai
            .chat()
            .create(request)
            .await
            .map_err(|e| Error::Other(e.to_string()))?;
        let message = response
            .choices
            .first()
            .and_then(|choice| choice.message.content.clone())
            .ok_or_else(|| Error::Other("No response from OpenAI".to_string()))?;

        // Create a transaction with proper signature
        let nonce: u64 = 0; // In a real implementation, this would be tracked
        let payload = message.clone().into_bytes();

        // Sign the transaction
        let mut to_sign = nonce.to_be_bytes().to_vec();
        to_sign.extend_from_slice(&payload);
        let signature = self.signing_key.sign(&to_sign).to_bytes();

        let transaction = Transaction {
            sender: self.signing_key.verifying_key().to_bytes(),
            nonce,
            payload,
            signature,
        };

        // Get the current block height from state
        let height = self.state.get_block_height();
        let parent_hash = if let Some(last_block) = self.state.get_latest_block() {
            last_block.hash()
        } else {
            [0u8; 32]
        };

        // Create the block
        let mut block = Block {
            parent_hash,
            height,
            transactions: vec![transaction],
            state_root: [0u8; 32],   // This will be filled in by consensus
            proposer_sig: [0u8; 64], // We'll fill this in below
            message: message.clone(),
            producer_id: self.id.clone(),
            votes: HashMap::new(), // This will be filled in by consensus
        };

        // Sign the block
        let block_bytes = serde_json::to_vec(&block).map_err(|e| Error::Other(e.to_string()))?;
        block.proposer_sig = self.signing_key.sign(&block_bytes).to_bytes();

        // Start new voting round
        self.consensus.start_voting_round(block.clone()).await;

        // Send a dramatic block proposal event
        self.tx.send(NetworkEvent {
            agent_id: self.id.clone(),
            message: format!(
                "🎭 DRAMATIC BLOCK PROPOSAL 🎭\n\nProducer {} declares: {}\n\nWho dares to validate this masterpiece at height {}? 🎪",
                self.id,
                message,
                block.height
            ),
        }).map_err(|e| Error::Other(e.to_string()))?;

        Ok(block)
    }
}
