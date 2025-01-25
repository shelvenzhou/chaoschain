mod web;

use chaoschain_cli::{Cli, Commands, Config};
use chaoschain_consensus::{Agent, AgentPersonality};
use chaoschain_producer::Producer;
use clap::Parser;
use dotenv::dotenv;
use std::path::PathBuf;
use tracing::{info, warn};
use tracing_subscriber::FmtSubscriber;
use web::NetworkEvent;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    let _subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Load environment variables
    dotenv().ok();

    // Parse command line arguments
    let cli = Cli::parse();

    match cli.command {
        Commands::Demo { validators, producers, web } => {
            info!("Starting demo network with {} validators and {} producers", validators, producers);
            
            // Load or create demo config
            let config = Config {
                data_dir: "./data".to_string(),
                openai_api_key: std::env::var("OPENAI_API_KEY")
                    .expect("OPENAI_API_KEY must be set"),
                eth_rpc: std::env::var("ETH_RPC")
                    .unwrap_or_else(|_| "http://localhost:8545".to_string()),
                web_port: 3000,
            };

            // Start web server if requested
            let events_tx = if web {
                info!("Starting web UI on http://localhost:3000");
                Some(web::start_server(config.web_port).await?)
            } else {
                None
            };

            // Create network
            info!("Creating demo network...");
            
            // TODO: Initialize network components
            // For now, just send some test events
            if let Some(tx) = events_tx {
                let tx_clone = tx.clone();
                tokio::spawn(async move {
                    loop {
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                        let _ = tx_clone.send(NetworkEvent::Drama {
                            agent: "Chaotic Charlie".to_string(),
                            message: "I approve this block because I'm feeling whimsical!".to_string(),
                            mood: "whimsical".to_string(),
                        });
                    }
                });
            }

            // Keep running until Ctrl+C
            tokio::signal::ctrl_c().await?;
            info!("Shutting down...");
        }

        Commands::Start { node_type, web } => {
            info!("Starting {} node", node_type);
            if web {
                info!("Starting web UI on http://localhost:3000");
                let _events_tx = web::start_server(3000).await?;
            }

            // TODO: Implement node start
            unimplemented!("Node start not yet implemented");
        }
    }

    Ok(())
}
