use chaoschain_core::{Block, Transaction};
use chaoschain_state::StateStore;
use chaoschain_consensus::{Agent, Config as ConsensusConfig};
use chaoschain_p2p::{Config as P2PConfig};
use chaoschain_producer::{Config as ProducerConfig, Producer};
use chaoschain_bridge::{Config as BridgeConfig};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

/// CLI configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Data directory
    pub data_dir: String,
    /// OpenAI API key
    pub openai_api_key: String,
    /// Ethereum RPC URL
    pub eth_rpc: String,
    /// Web UI port
    pub web_port: u16,
}

/// CLI commands
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Config file path
    #[arg(short, long, value_name = "FILE")]
    config: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start a demo network
    Demo {
        /// Number of validator agents
        #[arg(long, default_value_t = 4)]
        validators: usize,
        
        /// Number of block producers
        #[arg(long, default_value_t = 2)]
        producers: usize,
        
        /// Start web UI
        #[arg(long)]
        web: bool,
    },
    
    /// Start a node
    Start {
        /// Node type (validator/producer)
        #[arg(long, default_value = "validator")]
        node_type: String,
        
        /// Start web UI
        #[arg(long)]
        web: bool,
    },
} 