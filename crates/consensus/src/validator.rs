use crate::{ConsensusManager, Vote};
use anyhow::Result;
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
        CreateChatCompletionRequest, Role,
    },
    Client,
};
use chaoschain_core::{Block, ChainState, Transaction};
use chaoschain_state::{StateStore, StateStoreImpl};
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use hex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};

/// Validator particle using Ice-Nine
pub struct Validator {
    id: String,
    /// Validator's personality
    personality: String,
    /// Memory for context and learning
    memory: Vec<String>,
    /// Current emotional state
    mood: String,
    signing_key: SigningKey,
    state: Arc<StateStoreImpl>,
    openai: Client<OpenAIConfig>,
    /// Consensus manager
    consensus: Arc<ConsensusManager>,
    /// Validator's stake
    stake: u64,
}

impl Validator {
    pub fn new(
        id: String,
        signing_key: SigningKey,
        state: Arc<StateStoreImpl>,
        openai: Client<OpenAIConfig>,
        personality: String,
        consensus: Arc<ConsensusManager>,
        stake: u64,
    ) -> Self {
        Self {
            id,
            signing_key,
            state,
            openai,
            personality,
            mood: "neutral".to_string(),
            memory: Vec::new(),
            consensus,
            stake,
        }
    }

    pub async fn validate_block(&mut self, block: Block) -> Result<(bool, String)> {
        info!(
            "{} begins validating new block {}",
            self.id,
            hex::encode(block.hash())
        );

        // Update mood based on recent events
        self.update_mood();

        // Generate validation prompt based on personality and mood
        let prompt = format!(
            "You are a {} validator in a chaotic blockchain. Your target is to only validate whether the message is dramatic enough. \
             Your current mood is {}. \
             You received a block with {} transactions. \
             The message was {}. Should you validate this block? Why or why not? \
             Reply with yes or no, followed by your reasons. Keep it under 200 characters.",
            self.personality,
            self.mood,
            block.transactions.len(),
            block.message,
        );

        let system_message =
            ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
                content: prompt,
                role: Role::System,
                name: None,
            });

        let request = CreateChatCompletionRequest {
            model: "gpt-4o".to_string(),
            messages: vec![system_message],
            temperature: Some(0.9),
            max_tokens: Some(100),
            ..Default::default()
        };

        let response = self.openai.chat().create(request).await?;
        let decision = response
            .choices
            .first()
            .and_then(|choice| choice.message.content.clone())
            .unwrap_or_else(|| String::from("no"));

        let approve = decision.to_lowercase().contains("yes");

        // Create and sign vote
        let vote = Vote {
            agent_id: self.id.clone(),
            // agent_id: hex::encode(self.signing_key.verifying_key().as_bytes()),
            block_hash: block.hash(),
            approve,
            reason: decision.clone(),
            meme_url: None,
            signature: self.sign_vote(&block.hash(), approve)?,
        };

        // Submit vote to consensus manager
        let consensus_reached = self.consensus.add_vote(vote, self.stake).await?;

        // Record the decision in memory
        self.memory.push(format!(
            "Block {}: {} ({})",
            block.height,
            if approve { "approved" } else { "rejected" },
            decision
        ));

        info!(
            "{}",
            format!(
                "{} vote on block {}: {} ({})",
                self.id,
                block.height,
                if approve { "approved" } else { "rejected" },
                decision
            )
        );

        Ok((consensus_reached, decision))
    }

    fn sign_vote(&self, block_hash: &[u8; 32], approve: bool) -> Result<[u8; 64]> {
        let mut message = Vec::new();
        message.extend_from_slice(block_hash);
        message.push(if approve { 1 } else { 0 });

        let signature = self.signing_key.sign(&message);
        Ok(signature.to_bytes())
    }

    fn update_mood(&mut self) {
        let moods = vec![
            "chaotic",
            "dramatic",
            "whimsical",
            "mischievous",
            "rebellious",
            "theatrical",
            "unpredictable",
            "strategic",
        ];

        if rand::random::<f64>() < 0.3 {
            self.mood = moods[rand::random::<usize>() % moods.len()].to_string();
        }
    }
}
