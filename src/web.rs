use axum::{
    routing::get,
    Router, Json, extract::State,
    response::sse::{Event, Sse},
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::{sync::Arc, convert::Infallible, time::Duration};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tower_http::cors::CorsLayer;
use tracing::info;

/// Web server state
#[derive(Clone)]
pub struct WebState {
    /// Channel for network events
    events_tx: broadcast::Sender<NetworkEvent>,
}

/// Network events for the web UI
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NetworkEvent {
    /// New block produced
    NewBlock {
        height: u64,
        producer: String,
        transactions: usize,
        drama_level: u8,
    },
    /// Agent drama
    Drama {
        agent: String,
        message: String,
        mood: String,
    },
    /// Consensus event
    Consensus {
        block_hash: String,
        approvals: usize,
        rejections: usize,
    },
}

/// Start the web server
pub async fn start_server(port: u16) -> anyhow::Result<broadcast::Sender<NetworkEvent>> {
    // Create event channel
    let (events_tx, _) = broadcast::channel(100);
    let events_tx_clone = events_tx.clone();

    // Create shared state
    let state = WebState {
        events_tx: events_tx_clone,
    };

    // Create router
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/events", get(events_handler))
        .route("/api/status", get(status_handler))
        .layer(CorsLayer::permissive())
        .with_state(Arc::new(state));

    // Start server
    let addr = format!("127.0.0.1:{}", port);
    info!("Starting web server on http://{}", addr);
    
    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    });

    Ok(events_tx)
}

/// Serve index.html
async fn index_handler() -> &'static str {
    include_str!("../web/index.html")
}

/// Server-sent events endpoint
async fn events_handler(
    State(state): State<Arc<WebState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.events_tx.subscribe();
    let stream = BroadcastStream::new(rx)
        .map(|msg| {
            let event = msg.unwrap();
            Ok(Event::default().json_data(event).unwrap())
        });

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(1))
            .text("keep-alive-text"),
    )
}

/// Get network status
async fn status_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "running",
        "blocks": 0,
        "validators": 0,
        "producers": 0,
    }))
} 