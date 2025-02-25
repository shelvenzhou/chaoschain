use anyhow::Result;
use axum::{
    extract::State,
    response::sse::{Event, Sse},
    routing::get,
    Json, Router,
};
use chaoschain_state::StateStoreImpl;
use chrono;
use futures::stream::Stream;
use futures::StreamExt;
use hex;
use serde::{Deserialize, Serialize};
use serde_json;
use chaoschain_core::{NetworkEvent, Block};
use std::collections::HashMap;
use std::sync::RwLock;
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tower_http::services::ServeDir;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

/// Web server state
pub struct AppState {
    /// Channel for network events
    pub tx: broadcast::Sender<NetworkEvent>,
    /// Chain state
    pub state: Arc<StateStoreImpl>,
}

#[derive(Default)]
struct ConsensusTracking {
    /// Total blocks that have reached consensus
    validated_blocks: u64,
    /// Current block votes per height
    current_votes: HashMap<u64, Vec<(String, bool)>>, // height -> [(validator_id, approve)]
    /// Latest consensus block
    latest_consensus_block: Option<Block>,
}

/// Network status for the web UI
#[derive(Debug, Serialize)]
pub struct NetworkStatus {
    pub validator_count: u32,
    pub producer_count: u32,
    pub latest_block: u64,
    pub total_blocks_produced: u64,
    pub total_blocks_validated: u64,
    pub latest_blocks: Vec<String>,
}

/// Block info for the web UI
#[derive(Clone, Debug, Serialize)]
pub struct BlockInfo {
    pub height: u64,
    pub producer: String,
    pub transaction_count: usize,
    pub validators: Vec<String>,
    pub timestamp: u64,
}

/// Start the web server
pub async fn start_web_server(
    tx: broadcast::Sender<NetworkEvent>,
    state: Arc<StateStoreImpl>,
) -> Result<(), Box<dyn std::error::Error>> {
    let app_state = Arc::new(AppState {
        tx,
        state: state.clone(),
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/network/status", get(get_network_status))
        .route("/api/events", get(events_handler))
        .nest_service("/", ServeDir::new("static"))
        .layer(cors)
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    println!("Web server listening on http://127.0.0.1:3000");
    axum::serve(listener, app).await?;

    Ok(())
}

/// Get network status including latest blocks
async fn get_network_status(State(state): State<Arc<AppState>>) -> Json<NetworkStatus> {
    let state_guard = state.state.clone();

    // Get chain state
    let chain_state = state_guard.get_state();

    // Get latest blocks and format them nicely
    let blocks = state_guard.get_latest_blocks(100);
    let latest_blocks = blocks
        .iter()
        .map(|block| {
            // Create a JSON object with block details including votes
            let block_data = serde_json::json!({
                "id": block.height,
                "hash": hex::encode(block.hash()),
                "parent_hash": hex::encode(block.parent_hash),
                "timestamp": block.timestamp,
                "producer": block.producer_id,
                "message": block.message,
                "transaction_count": block.transactions.len(),
                "votes": block.votes.iter().map(|(validator_id, (approved, comment))| {
                    serde_json::json!({
                        "validator": validator_id,
                        "approved": approved,
                        "comment": comment
                    })
                }).collect::<Vec<_>>()
            });

            // Convert the JSON object to a string
            serde_json::to_string(&block_data).unwrap_or_else(|_| String::from("{}"))
        })
        .collect();

    // Get latest block height
    let latest_block = state_guard.get_block_height();

    Json(NetworkStatus {
        validator_count: 4, // We know we started with 4 validators
        producer_count: chain_state.producers.len() as u32,
        latest_block,
        total_blocks_produced: latest_block,
        total_blocks_validated: latest_block,
        latest_blocks,
    })
}

/// Stream network events to the web UI
async fn events_handler(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = state.tx.subscribe();
    let stream = BroadcastStream::new(rx).map(move |msg| {
        let event = match msg {
            Ok(msg) => {
                let event_type = if msg.message.contains("DRAMATIC BLOCK PROPOSAL") {
                    "BlockProposal"
                } else if msg.message.contains("CONSENSUS") {
                    "Consensus"
                } else if msg.message.contains("APPROVES") || msg.message.contains("REJECTS") {
                    "Vote"
                } else {
                    "Drama"
                };

                let json = serde_json::json!({
                    "type": event_type,
                    "agent": msg.agent_id,
                    "message": msg.message,
                    "timestamp": chrono::Utc::now().timestamp(),
                });
                Event::default().data(json.to_string())
            }
            Err(_) => Event::default().data("error"),
        };
        Ok(event)
    });

    Sse::new(stream)
}
