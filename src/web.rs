use axum::{
    routing::get,
    Router, Json, extract::State,
    response::sse::{Event, Sse},
};
use futures::stream::Stream;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::{sync::Arc, convert::Infallible, net::SocketAddr};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tracing::info;
use anyhow::{Result, bail};
use tower_http::services::ServeDir;
use serde_json;
use chaoschain_core::{NetworkEvent, Block};
use chaoschain_state::StateStoreImpl;
use std::sync::RwLock;
use hex;

/// Web server state
#[derive(Clone)]
pub struct WebState {
    /// Channel for network events
    pub tx: broadcast::Sender<NetworkEvent>,
    /// Chain state
    pub state: Arc<RwLock<StateStoreImpl>>,
}

/// Network status for the web UI
#[derive(Clone, Debug, Serialize)]
pub struct NetworkStatus {
    pub validator_count: usize,
    pub producer_count: usize,
    pub latest_block: u64,
    pub total_blocks_produced: u64,
    pub total_blocks_validated: u64,
    pub latest_blocks: Vec<BlockInfo>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BlockInfo {
    pub height: u64,
    pub producer: String,
    pub transaction_count: usize,
    pub validators: Vec<String>,
    pub timestamp: u64,
}

/// Start the web server
pub async fn start_web_server(tx: broadcast::Sender<NetworkEvent>, state: StateStoreImpl) -> Result<()> {
    let state = WebState { 
        tx,
        state: Arc::new(RwLock::new(state)),
    };
    let app = Router::new()
        .route("/api/events", get(events_handler))
        .route("/api/status", get(status_handler))
        .route("/api/blocks", get(blocks_handler))
        .nest_service("/", ServeDir::new("static"))
        .with_state(Arc::new(state));

    for port in 3000..3010 {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        match axum::serve(tokio::net::TcpListener::bind(addr).await?, app.clone()).await {
            Ok(_) => {
                info!("Web UI available at http://localhost:{}", port);
                return Ok(());
            }
            Err(e) => {
                info!("Failed to bind to port {}: {}", port, e);
                continue;
            }
        }
    }

    bail!("Failed to find an available port")
}

/// Server-sent events endpoint
async fn events_handler(
    State(state): State<Arc<WebState>>,
) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    let rx = state.tx.subscribe();
    let stream = BroadcastStream::new(rx).map(|msg| match msg {
        Ok(msg) => {
            let json = serde_json::to_string(&msg).unwrap();
            Ok(Event::default().data(json))
        }
        Err(e) => {
            let error = serde_json::json!({
                "error": format!("Error: {}", e)
            });
            Ok(Event::default().data(error.to_string()))
        }
    });

    Sse::new(stream)
}

/// Get network status including latest blocks
async fn status_handler(State(state): State<Arc<WebState>>) -> Json<NetworkStatus> {
    let state_guard = state.state.read().unwrap();
    let latest_blocks = state_guard.get_latest_blocks(5)
        .into_iter()
        .map(|block| BlockInfo {
            height: block.height,
            producer: hex::encode(&block.proposer_sig[0..8]), // Use first 8 bytes of signature as producer ID
            transaction_count: block.transactions.len(),
            validators: vec![], // TODO: Track validators who approved the block
            timestamp: state_guard.get_block_timestamp(&block).unwrap_or(0),
        })
        .collect();

    Json(NetworkStatus {
        validator_count: 4, // TODO: Get actual count
        producer_count: 2, // TODO: Get actual count
        latest_block: state_guard.get_block_height(),
        total_blocks_produced: state_guard.get_block_height(),
        total_blocks_validated: state_guard.get_block_height(), // TODO: Track validated blocks
        latest_blocks,
    })
}

/// Get detailed block information
async fn blocks_handler(State(state): State<Arc<WebState>>) -> Json<Vec<BlockInfo>> {
    let state_guard = state.state.read().unwrap();
    let blocks = state_guard.get_latest_blocks(20)
        .into_iter()
        .map(|block| BlockInfo {
            height: block.height,
            producer: hex::encode(&block.proposer_sig[0..8]),
            transaction_count: block.transactions.len(),
            validators: vec![], // TODO: Track validators who approved the block
            timestamp: state_guard.get_block_timestamp(&block).unwrap_or(0),
        })
        .collect();

    Json(blocks)
} 