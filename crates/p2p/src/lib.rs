use chaoschain_core::{Block, Transaction, NetworkMessage};
use libp2p::{
    gossipsub::{
        self,
        Behaviour as GossipsubBehaviour,
        ConfigBuilder as GossipsubConfigBuilder,
        MessageAuthenticity,
        ValidationMode,
        Topic,
        IdentTopic,
    },
    identity::Keypair,
    mdns::{self, tokio::Behaviour as MdnsBehaviour},
    swarm::{NetworkBehaviour, SwarmEvent},
    PeerId, Swarm,
    core::transport::Transport,
    tcp,
    noise,
    yamux,
    StreamProtocol,
    SwarmBuilder,
};
use serde::{Deserialize, Serialize};
use tracing::info;
use anyhow::{Result, anyhow};
use thiserror::Error;
use std::time::Duration;
use futures::StreamExt;

/// P2P message types for agent communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Propose a new block
    BlockProposal(Block),
    /// Vote on a block proposal
    BlockVote {
        block_hash: [u8; 32],
        approve: bool,
        reason: String,
        meme_url: Option<String>,
    },
    /// Agent chat message
    Chat {
        message: String,
        mood: String,
        meme_url: Option<String>,
    },
    /// Broadcast a new transaction
    Transaction(Transaction),
}

/// P2P network configuration
#[derive(Debug, Clone)]
pub struct Config {
    /// Local peer ID
    pub peer_id: PeerId,
    /// Bootstrap peers to connect to
    pub bootstrap_peers: Vec<String>,
    /// Port to listen on
    pub port: u16,
}

/// P2P network errors
#[derive(Debug, Error)]
pub enum Error {
    #[error("Network error: {0}")]
    Network(String),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Fun message types for agent communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentMessage {
    /// Standard block proposal
    BlockProposal(Block),
    /// Vote on a block
    Vote(BlockVote),
    /// Question about state diff
    WhyThisStateDiff {
        block_hash: [u8; 32],
        question: String,
    },
    /// Bribe attempt (for fun!)
    Bribe {
        block_hash: [u8; 32],
        offer: String,
        meme_base64: Option<String>,
    },
    /// Rejection with optional meme
    BlockRejectionMeme {
        block_hash: [u8; 32],
        reason: String,
        meme_base64: Option<String>,
    },
    /// General chat message
    Chat {
        message: String,
        reaction_emoji: Option<String>,
    },
}

/// Network topics for different message types
pub struct NetworkTopics {
    blocks: IdentTopic,
    transactions: IdentTopic,
    chat: IdentTopic,
}

impl NetworkTopics {
    pub fn new() -> Self {
        Self {
            blocks: IdentTopic::new("blocks"),
            transactions: IdentTopic::new("transactions"),
            chat: IdentTopic::new("chat"),
        }
    }
}

/// Combined network behavior
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "ChainEvent", event_process = false)]
pub struct ChainNetworkBehaviour {
    gossipsub: GossipsubBehaviour,
    mdns: MdnsBehaviour,
}

#[derive(Debug)]
pub enum ChainEvent {
    Gossipsub(gossipsub::Event),
    Mdns(mdns::Event),
}

impl From<gossipsub::Event> for ChainEvent {
    fn from(event: gossipsub::Event) -> Self {
        ChainEvent::Gossipsub(event)
    }
}

impl From<mdns::Event> for ChainEvent {
    fn from(event: mdns::Event) -> Self {
        ChainEvent::Mdns(event)
    }
}

/// P2P network manager
pub struct Network {
    swarm: Swarm<ChainNetworkBehaviour>,
    topics: NetworkTopics,
}

impl Network {
    pub async fn new() -> Result<Self> {
        let id_keys = Keypair::generate_ed25519();
        let peer_id = PeerId::from(id_keys.public());
        info!("Local peer id: {peer_id}");

        let transport = tcp::tokio::Transport::new(tcp::Config::default())
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(noise::Config::new(&id_keys)?)
            .multiplex(yamux::Config::default())
            .boxed();

        let mut swarm = {
            let gossipsub_config = GossipsubConfigBuilder::default()
                .validation_mode(ValidationMode::Permissive)
                .build()
                .expect("Valid config");

            let gossipsub = GossipsubBehaviour::new(
                MessageAuthenticity::Signed(id_keys.clone()),
                gossipsub_config,
            ).map_err(|e| anyhow!("Failed to create gossipsub: {}", e))?;

            let mdns = MdnsBehaviour::new(Default::default(), peer_id)?;

            let behaviour = ChainNetworkBehaviour {
                gossipsub,
                mdns,
            };

            Swarm::new(transport, behaviour, peer_id)
        };

        let topics = NetworkTopics::new();

        swarm.behaviour_mut().gossipsub.subscribe(&topics.blocks)?;
        swarm.behaviour_mut().gossipsub.subscribe(&topics.transactions)?;
        swarm.behaviour_mut().gossipsub.subscribe(&topics.chat)?;

        Ok(Self { swarm, topics })
    }

    pub async fn start(&mut self) -> Result<()> {
        self.swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

        loop {
            match self.swarm.next().await.expect("Swarm stream is infinite") {
                SwarmEvent::Behaviour(ChainEvent::Gossipsub(event)) => {
                    if let gossipsub::Event::Message { message, .. } = event {
                        let msg: NetworkMessage = serde_json::from_slice(&message.data)?;
                        match msg {
                            NetworkMessage::NewBlock(block) => {
                                info!("Received new block: {:?}", block);
                            }
                            NetworkMessage::NewTransaction(tx) => {
                                info!("Received new transaction: {:?}", tx);
                            }
                            NetworkMessage::Chat { from, message } => {
                                info!("Chat from {}: {}", from, message);
                            }
                            NetworkMessage::AgentReasoning { agent, reasoning } => {
                                info!("Agent {} reasoning: {}", agent, reasoning);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    pub async fn broadcast(&mut self, message: NetworkMessage) -> Result<()> {
        let data = serde_json::to_vec(&message)?;
        
        match message {
            NetworkMessage::NewBlock(_) => {
                self.swarm.behaviour_mut().gossipsub.publish(
                    self.topics.blocks.clone(),
                    data,
                )?;
            }
            NetworkMessage::NewTransaction(_) => {
                self.swarm.behaviour_mut().gossipsub.publish(
                    self.topics.transactions.clone(),
                    data,
                )?;
            }
            NetworkMessage::Chat { .. } | NetworkMessage::AgentReasoning { .. } => {
                self.swarm.behaviour_mut().gossipsub.publish(
                    self.topics.chat.clone(),
                    data,
                )?;
            }
        }

        Ok(())
    }
} 