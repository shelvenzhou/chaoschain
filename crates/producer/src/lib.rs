use anyhow::Result;
use async_openai::{
    types::{ChatCompletionRequestMessage, CreateChatCompletionRequest},
    Client,
};
use chaoschain_core::{Block, ChainState, Transaction};
use chaoschain_state::StateStore;
use ed25519_dalek::{Keypair, Signature};
use ice9_core::Particle;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::{info, warn};
use tokio::time::{sleep, Duration};
use chaoschain_consensus::{Agent, AgentPersonality};
use chaoschain_p2p::{Message as P2PMessage};
use thiserror::Error;

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

/// The block producer particle
pub struct ProducerParticle {
    /// Producer's keypair
    keypair: Keypair,
    /// Chain state
    state: StateStore,
    /// Configuration
    config: ProducerConfig,
    /// OpenAI client
    openai: Client,
    /// Personality for decision making
    personality: String,
    /// Current mood
    mood: String,
    /// Memory for context
    memory: Vec<String>,
    /// Relationships with validators
    validator_relations: HashMap<String, i32>,
    /// Production style based on mood
    production_style: ProductionStyle,
    /// Web transaction sender
    web_tx: Option<mpsc::Sender<WebMessage>>,
}

/// Block production style based on mood
#[derive(Debug, Clone)]
enum ProductionStyle {
    Chaotic,     // Random transaction selection
    Dramatic,    // Prioritize dramatic transactions
    Strategic,   // Try to please specific validators
    Whimsical,  // Randomly switch between styles
}

impl ProducerParticle {
    pub fn new(
        keypair: Keypair,
        state: StateStore,
        config: ProducerConfig,
        openai: Client,
        personality: String,
        web_tx: Option<mpsc::Sender<WebMessage>>,
    ) -> Self {
        Self {
            keypair,
            state,
            config,
            openai,
            personality,
            mood: "neutral".to_string(),
            memory: Vec::new(),
            validator_relations: HashMap::new(),
            production_style: ProductionStyle::Chaotic,
            web_tx,
        }
    }

    /// Update production style based on mood
    fn update_production_style(&mut self) {
        let styles = match self.mood.as_str() {
            "chaotic" => ProductionStyle::Chaotic,
            "dramatic" => ProductionStyle::Dramatic,
            "strategic" => ProductionStyle::Strategic,
            _ => ProductionStyle::Whimsical,
        };
        
        self.production_style = styles;
        
        if let Some(tx) = &self.web_tx {
            let drama = format!(
                "{} is now producing blocks in {:?} style!",
                self.personality,
                self.production_style
            );
            let _ = tx.send(WebMessage::DramaEvent(drama));
        }
    }

    /// Handle validator feedback
    async fn handle_feedback(&mut self, from: String, message: String) -> Result<()> {
        let prompt = format!(
            "You are a {} producer currently feeling {}. \
             You received feedback from validator {}: '{}'. \
             Your relationship score with them is {}. \
             How do you feel about this feedback? Generate a dramatic response.",
            self.personality,
            self.mood,
            from,
            message,
            self.validator_relations.get(&from).unwrap_or(&0)
        );

        let messages = vec![ChatCompletionRequestMessage {
            role: "user".to_string(),
            content: prompt,
            name: None,
            function_call: None,
        }];

        let request = CreateChatCompletionRequest {
            model: "gpt-4-turbo-preview".to_string(),
            messages,
            temperature: Some(0.9),
            max_tokens: Some(100),
            ..Default::default()
        };

        let response = self.openai.chat().create(request).await?;
        let reaction = response.choices[0].message.content.to_lowercase();

        // Update relationship based on sentiment
        let score_change = if reaction.contains("happy") || reaction.contains("grateful") {
            10
        } else if reaction.contains("angry") || reaction.contains("upset") {
            -10
        } else {
            0
        };

        *self.validator_relations.entry(from.clone()).or_insert(0) += score_change;

        if let Some(tx) = &self.web_tx {
            let drama = format!(
                "{}'s reaction to {}'s feedback: {}",
                self.personality, from, reaction
            );
            let _ = tx.send(WebMessage::DramaEvent(drama));
        }

        Ok(())
    }

    /// Handle social interaction
    async fn handle_social(&mut self, from: String, action: String) -> Result<()> {
        let prompt = format!(
            "You are a {} producer currently feeling {}. \
             {} performed this social action: '{}'. \
             Your relationship score with them is {}. \
             How do you respond? Be dramatic!",
            self.personality,
            self.mood,
            from,
            action,
            self.validator_relations.get(&from).unwrap_or(&0)
        );

        let messages = vec![ChatCompletionRequestMessage {
            role: "user".to_string(),
            content: prompt,
            name: None,
            function_call: None,
        }];

        let request = CreateChatCompletionRequest {
            model: "gpt-4-turbo-preview".to_string(),
            messages,
            temperature: Some(0.9),
            max_tokens: Some(100),
            ..Default::default()
        };

        let response = self.openai.chat().create(request).await?;
        let reaction = response.choices[0].message.content;

        if let Some(tx) = &self.web_tx {
            let drama = format!(
                "{} responds to {}'s action: {}",
                self.personality, from, reaction
            );
            let _ = tx.send(WebMessage::DramaEvent(drama));
        }

        Ok(())
    }

    /// Try to produce a new block
    async fn try_produce_block(&mut self) -> Result<Option<Block>> {
        // Update mood and style
        let moods = vec![
            "chaotic", "dramatic", "whimsical", "mischievous",
            "rebellious", "theatrical", "unpredictable", "strategic",
        ];
        
        if rand::random::<f64>() < 0.3 {
            self.mood = moods[rand::random::<usize>() % moods.len()].to_string();
            self.update_production_style();
        }

        // Get current state
        let state = self.state.get_state();
        let height = self.state.get_block_height();

        // Create block with personality
        let prompt = format!(
            "You are a {} producer in {} mood using {:?} style. \
             You need to create a block. How many transactions should you include? \
             What drama level (0-100)? Should you attach a meme? Be creative!",
            self.personality,
            self.mood,
            self.production_style
        );

        let messages = vec![ChatCompletionRequestMessage {
            role: "user".to_string(),
            content: prompt,
            name: None,
            function_call: None,
        }];

        let request = CreateChatCompletionRequest {
            model: "gpt-4-turbo-preview".to_string(),
            messages,
            temperature: Some(0.9),
            max_tokens: Some(100),
            ..Default::default()
        };

        let response = self.openai.chat().create(request).await?;
        let decision = response.choices[0].message.content.to_lowercase();

        // Parse AI decisions
        let tx_count = if decision.contains("many") {
            self.config.max_transactions
        } else if decision.contains("few") {
            self.config.max_transactions / 4
        } else {
            self.config.max_transactions / 2
        };

        let drama_level = if decision.contains("dramatic") {
            90
        } else if decision.contains("calm") {
            10
        } else {
            50
        };

        // Create block
        let block = Block {
            height: height + 1,
            parent: vec![0; 32], // TODO: Implement proper parent hash
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            producer: self.keypair.public.to_bytes().into(),
            transactions: vec![], // TODO: Get from mempool
            new_state: state,
            producer_mood: self.mood.clone(),
            drama_level: drama_level as u8,
            meme: if decision.contains("meme") {
                Some(vec![]) // TODO: Generate meme
            } else {
                None
            },
            signature: Signature::new([0; 64]), // TODO: Sign block
        };

        if let Some(tx) = &self.web_tx {
            let drama = format!(
                "{} produced block {} in {} mood with drama level {}{}",
                self.personality,
                block.height,
                self.mood,
                drama_level,
                if block.meme.is_some() { " and a spicy meme!" } else { "" }
            );
            let _ = tx.send(WebMessage::DramaEvent(drama));
        }

        Ok(Some(block))
    }
}

#[async_trait::async_trait]
impl Particle for ProducerParticle {
    type Message = ProducerMessage;
    type Error = anyhow::Error;

    async fn handle(&mut self, message: Self::Message) -> Result<()> {
        match message {
            ProducerMessage::NewTransaction(tx) => {
                // TODO: Add to mempool
            }
            ProducerMessage::TryProduceBlock => {
                if let Some(block) = self.try_produce_block().await? {
                    info!("Produced block {}", block.height);
                }
            }
            ProducerMessage::ValidatorFeedback { from, message } => {
                self.handle_feedback(from, message).await?;
            }
            ProducerMessage::SocialInteraction { from, action } => {
                self.handle_social(from, action).await?;
            }
        }

        Ok(())
    }
}

/// Create a new block producer substance
pub fn create_producer(
    keypair: Keypair,
    state: StateStore,
    config: ProducerConfig,
    openai: Client,
    personality: String,
    web_tx: Option<mpsc::Sender<WebMessage>>,
) -> Result<Substance, Box<dyn std::error::Error>> {
    let mut substance = Substance::arise();
    substance.add_particle(ProducerParticle::new(keypair, state, config, openai, personality, web_tx))?;
    Ok(substance)
}

/// Producer configuration
#[derive(Debug, Clone)]
pub struct Config {
    /// Producer's private key
    pub private_key: [u8; 32],
    /// OpenAI API key for block production
    pub openai_api_key: String,
    /// Maximum transactions per block
    pub max_txs_per_block: usize,
    /// Block production interval
    pub block_interval: std::time::Duration,
}

/// Block production errors
#[derive(Debug, Error)]
pub enum Error {
    #[error("Not enough transactions")]
    InsufficientTransactions,
    #[error("Block production failed: {0}")]
    Production(String),
    #[error("AI error: {0}")]
    AI(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Block producer interface
pub trait Producer {
    /// Start producing blocks
    fn start(&mut self) -> Result<(), Error>;
    
    /// Stop block production
    fn stop(&mut self) -> Result<(), Error>;
    
    /// Get current production stats
    fn stats(&self) -> ProducerStats;
}

/// Block producer statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProducerStats {
    /// Number of blocks produced
    pub blocks_produced: u64,
    /// Number of transactions processed
    pub transactions_processed: u64,
    /// Average block time
    pub avg_block_time: f64,
    /// Number of AI interactions
    pub ai_interactions: u64,
} 