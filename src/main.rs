mod web;

use chaoschain_cli::{Cli, Commands};
use chaoschain_consensus::{Agent, AgentPersonality};
use chaoschain_producer::ProducerParticle;
use chaoschain_state::StateStoreImpl;
use chaoschain_core::{ChainConfig, NetworkEvent};
use clap::Parser;
use dotenv::dotenv;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, warn};
use tracing_subscriber::FmtSubscriber;
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

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
            info!("Starting demo network with {} validators and {} producers", validators, producers);

            let (tx, _) = broadcast::channel(100);
            let web_tx = tx.clone();

            if web {
                info!("Starting web UI");
                let state = StateStoreImpl::new(ChainConfig::default());
                tokio::spawn(async move {
                    web::start_web_server(web_tx, state).await.unwrap();
                });
            }

            // Create and start validators
            for i in 0..validators {
                let agent_id = format!("validator-{}", i);
                let personality = AgentPersonality::random();
                
                info!("Starting validator {} with {:?} personality", agent_id, personality);
                
                // Generate a keypair for the validator
                let signing_key = SigningKey::generate(&mut OsRng);
                let mut agent = Agent::new(signing_key.verifying_key().to_bytes(), personality);
                let tx = tx.clone();
                let agent_id_clone = agent_id.clone();
                let rx = tx.subscribe();
                
                tokio::spawn(async move {
                    let mut rx = rx;
                    loop {
                        if let Ok(event) = rx.recv().await {
                            // React to block proposals based on personality
                            if event.message.contains("DRAMATIC BLOCK PROPOSAL") {
                                let reaction = match agent.personality {
                                    AgentPersonality::Lawful => {
                                        "🧐 Hmm... I'll need to carefully review this according to protocol..."
                                    }
                                    AgentPersonality::Chaotic => {
                                        "🌪️ CHAOS! I approve this block because I'm feeling particularly chaotic today!"
                                    }
                                    AgentPersonality::Memetic => {
                                        "🎭 This block speaks to me on a meme level. Much wow!"
                                    }
                                    AgentPersonality::Greedy => {
                                        "💰 What's in it for me? Perhaps we can negotiate..."
                                    }
                                    AgentPersonality::Dramatic => {
                                        "🎬 *gasps theatrically* This block... it's... it's... MAGNIFICENT!"
                                    }
                                    AgentPersonality::Neutral => {
                                        "🤷 Whatever, I could go either way on this one."
                                    }
                                    AgentPersonality::Rational => {
                                        "🤔 Let me analyze this logically... although logic is optional here."
                                    }
                                    AgentPersonality::Emotional => {
                                        "😭 This block makes me feel so many emotions!"
                                    }
                                    AgentPersonality::Strategic => {
                                        "🎯 I see potential strategic value in this proposal..."
                                    }
                                };

                                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                                tx.send(NetworkEvent {
                                    agent_id: agent_id_clone.clone(),
                                    message: reaction.to_string(),
                                }).unwrap();
                            }
                        }
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                });
            }

            // Create and start producers
            for i in 0..producers {
                let producer_id = format!("producer-{}", i);
                let state = Box::new(StateStoreImpl::new(ChainConfig::default()));
                let openai = async_openai::Client::new();
                
                info!("Starting producer {}", producer_id);
                
                let producer = ProducerParticle::new(
                    producer_id.clone(),
                    state,
                    openai,
                    tx.clone(),
                );
                
                tokio::spawn(async move {
                    producer.run().await.unwrap();
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
                web::start_web_server(tx, state).await?;
            }

            // TODO: Implement node start
            unimplemented!("Node start not yet implemented");
        }
    }

    #[allow(unreachable_code)]
    Ok(())
}
