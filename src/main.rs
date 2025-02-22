mod web;

use anyhow::Result;
use async_openai::config::OpenAIConfig as RawConfig;
use chaoschain_cli::{Cli, Commands};
use chaoschain_consensus::{AgentPersonality, Config as ConsensusConfig};
use chaoschain_core::{Block, ChainConfig, NetworkEvent};
use chaoschain_producer::Producer;
use chaoschain_state::{StateStore, StateStoreImpl};
use clap::Parser;
use dotenv::dotenv;
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, warn};
use tracing_subscriber::FmtSubscriber;

/// OpenAI configuration for agent personalities
struct OpenAIConfig {
    api_base: String,
    api_key: String,
    model: String,
    temperature: f32,
}

impl OpenAIConfig {
    fn from_env() -> Result<Self> {
        Ok(Self {
            api_base: std::env::var("OPENAI_API_BASE")
                .unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
            api_key: std::env::var("OPENAI_API_KEY")
                .map_err(|_| anyhow::anyhow!("OPENAI_API_KEY not set"))?,
            model: std::env::var("AGENT_MODEL")
                .unwrap_or_else(|_| "gpt-4-turbo-preview".to_string()),
            temperature: std::env::var("TEMPERATURE")
                .unwrap_or_else(|_| "0.9".to_string())
                .parse()
                .unwrap_or(0.9),
        })
    }

    pub fn extract(&self) -> RawConfig {
        RawConfig::default()
            .with_api_key(&self.api_key)
            .with_api_base(&self.api_base)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables
    dotenv().ok();

    // Initialize logging
    let subscriber = FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber)?;

    // Parse command line arguments
    let cli = Cli::parse();

    match cli.command {
        Commands::Demo {
            validators,
            producers,
            web,
        } => {
            info!(
                "Starting demo network with {} validators and {} producers",
                validators, producers
            );

            let openai_config = OpenAIConfig::from_env()
                .map_err(|e| anyhow::anyhow!("Failed to load OpenAI config: {}", e))?;
            let openai = async_openai::Client::with_config(openai_config.extract());

            let (tx, _) = broadcast::channel(1000);
            let web_tx = tx.clone();

            // Create consensus manager
            let stake_per_validator = 100u64; // Each validator has 100 stake
            let total_stake = validators as u64 * stake_per_validator;
            let consensus_config = ConsensusConfig::default();
            let consensus_manager = Arc::new(chaoschain_consensus::create_consensus_manager(
                total_stake,
                consensus_config,
            ));

            // Create shared state
            let shared_state = Arc::new(StateStoreImpl::new(ChainConfig::default()));

            if web {
                info!("Starting web UI");
                let state = shared_state.clone();
                tokio::spawn(async move {
                    web::start_web_server(web_tx, state).await.unwrap();
                });
            }

            // Create and start validators
            for i in 0..validators {
                let agent_id = format!("validator-{}", i);
                let personality = AgentPersonality::random();

                info!(
                    "Starting validator {} with {:?} personality",
                    agent_id, personality
                );

                // Generate a keypair for the validator
                let signing_key = SigningKey::generate(&mut OsRng);
                let tx = tx.clone();
                let agent_id_clone = agent_id.clone();
                let rx = tx.subscribe();
                let consensus = consensus_manager.clone();
                let state = shared_state.clone();

                tokio::spawn(async move {
                    let mut rx = rx;
                    loop {
                        if let Ok(event) = rx.recv().await {
                            // React to block proposals based on personality
                            if event.message.contains("DRAMATIC BLOCK PROPOSAL") {
                                // Parse block from event message
                                if let Some(block) = parse_block_from_event(&event) {
                                    // Create a proper vote
                                    let vote = chaoschain_consensus::Vote {
                                        agent_id: agent_id_clone.clone(),
                                        block_hash: block.hash(),
                                        approve: rand::random::<bool>(),
                                        reason: "Because I felt like it!".to_string(),
                                        meme_url: None,
                                        signature: [0u8; 64], // TODO: Proper signing
                                    };

                                    // Store vote approval before moving
                                    let approved = vote.approve;

                                    // Submit vote with stake
                                    match consensus.add_vote(vote, stake_per_validator).await {
                                        Ok(true) => {
                                            // Consensus reached!
                                            let response = format!(
                                                "🎭 CONSENSUS: Block {} has been {}! Validator {} made it happen!",
                                                block.height,
                                                if approved { "APPROVED" } else { "REJECTED" },
                                                agent_id_clone
                                            );
                                            if let Err(e) = tx.send(NetworkEvent {
                                                agent_id: agent_id_clone.clone(),
                                                message: response,
                                            }) {
                                                warn!("Failed to send consensus message: {}", e);
                                            }

                                            // Store block in state if approved
                                            if approved {
                                                info!("Storing block {} in state", block.height);
                                                if let Err(e) = state.apply_block(&block) {
                                                    warn!("Failed to store block: {}", e);
                                                }
                                            }
                                        }
                                        Ok(false) => {
                                            // Vote recorded but no consensus yet
                                            let response = if approved {
                                                format!("🎭 Validator {} APPROVES block {} with great enthusiasm! Such drama!", agent_id_clone, block.height)
                                            } else {
                                                format!("🎭 Validator {} REJECTS block {} - not dramatic enough!", agent_id_clone, block.height)
                                            };

                                            if let Err(e) = tx.send(NetworkEvent {
                                                agent_id: agent_id_clone.clone(),
                                                message: response,
                                            }) {
                                                warn!("Failed to send validator response: {}", e);
                                            }
                                        }
                                        Err(e) => {
                                            warn!("Failed to submit vote: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                });
            }

            // Create and start producers
            for i in 0..producers {
                let producer_id = format!("producer-{}", i);
                let state = shared_state.clone();
                let consensus = consensus_manager.clone();

                info!("Starting producer {}", producer_id);
                let producer = Producer::new(
                    producer_id.clone(),
                    state.clone(),
                    openai.clone(),
                    tx.clone(),
                    consensus,
                );

                // Register producer in state
                state.add_block_producer(producer.signing_key.verifying_key());

                tokio::spawn(async move {
                    loop {
                        let _ = producer.generate_block().await.unwrap();
                    }
                });
            }

            // Keep the main thread alive
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }

        Commands::Start { node_type, web } => {
            info!("Starting {} node", node_type);
            if web {
                info!("Starting web UI");
                let (tx, _) = tokio::sync::broadcast::channel(100);
                let state = StateStoreImpl::new(ChainConfig::default());
                let state = Arc::new(state);
                if let Err(e) = web::start_web_server(tx, state.clone()).await {
                    warn!("Failed to start web server: {}", e);
                }
            }

            // TODO: Implement node start
            unimplemented!("Node start not yet implemented");
        }
    }

    #[allow(unreachable_code)]
    Ok(())
}

// Helper function to parse block from event
fn parse_block_from_event(event: &NetworkEvent) -> Option<Block> {
    // Extract block height from message
    // Example message: "🎭 DRAMATIC BLOCK PROPOSAL: Producer producer-0 in dramatic mood proposes block 5 with drama level 3!"
    let message = &event.message;

    if let Some(height_start) = message.find("block ") {
        if let Some(height_end) = message[height_start..].find(" with") {
            if let Ok(height) = message[height_start + 6..height_start + height_end]
                .trim()
                .parse::<u64>()
            {
                // Extract drama level
                if let Some(drama_start) = message.find("drama level ") {
                    if let Some(drama_end) = message[drama_start..].find("!") {
                        if let Ok(drama_level) = message[drama_start + 11..drama_start + drama_end]
                            .trim()
                            .parse::<u8>()
                        {
                            // Extract producer mood
                            if let Some(mood_start) = message.find("in ") {
                                if let Some(mood_end) = message[mood_start..].find(" mood") {
                                    let mood =
                                        message[mood_start + 3..mood_start + mood_end].to_string();

                                    // Extract producer ID
                                    if let Some(producer_start) = message.find("Producer ") {
                                        if let Some(producer_end) =
                                            message[producer_start..].find(" in")
                                        {
                                            let producer_id = message
                                                [producer_start + 9..producer_start + producer_end]
                                                .to_string();

                                            return Some(Block {
                                                height,
                                                transactions: vec![],
                                                proposer_sig: [0u8; 64],
                                                parent_hash: [0u8; 32],
                                                state_root: [0u8; 32],
                                                drama_level,
                                                producer_mood: mood,
                                                producer_id: producer_id, // Store the actual producer ID
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    warn!("Failed to parse block from event: {}", message);
    None
}
