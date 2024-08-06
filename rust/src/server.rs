use crate::runner::Runner;

use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{
    body::Body,
    error_handling::HandleError,
    error_handling::HandleErrorLayer,
    http::{Request, Response, StatusCode},
    BoxError, Router,
};
use axum::{Json, ServiceExt};
use futures::channel::mpsc::UnboundedReceiver;
use serde_json::json;
use std::sync::Arc;
use tokio_tower::pipeline;
use tower::Service;

use crate::runner::{PredictionRequest, PredictionResponse};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tracing::{debug, error, info};

use crate::connection::ChannelTransport;
use tokio::sync::mpsc;
use tokio::sync::Mutex;

pub struct Server {
    host: String,
    port: u16,
}

pub type RunnerClient = pipeline::Client<
    ChannelTransport<PredictionResponse, PredictionRequest>,
    tokio_tower::Error<ChannelTransport<PredictionResponse, PredictionRequest>, PredictionRequest>,
    PredictionRequest,
>;

#[derive(Clone, Debug)]
pub struct AppState {
    health: Health,
    client: Arc<Mutex<RunnerClient>>,
}

#[derive(Clone, Default, Debug, Deserialize, Serialize)]
pub enum Health {
    #[default]
    Starting,
    Ready,
}

impl Server {
    pub fn new(host: String, port: u16) -> Self {
        Self { host, port }
    }

    pub async fn run(&self, runner: Runner) -> Result<(), Box<dyn std::error::Error>> {
        let (tx_req, rx_req) = mpsc::unbounded_channel::<PredictionRequest>();
        let (tx_resp, rx_resp) = mpsc::unbounded_channel::<PredictionResponse>();
        let pair1 = ChannelTransport::new(rx_req, tx_resp);
        let pair2 = ChannelTransport::new(rx_resp, tx_req);

        let runner_handle =
            tokio::task::spawn(async move { pipeline::Server::new(pair1, runner).await });
        let client = RunnerClient::new(pair2);

        let state = AppState {
            health: Health::Ready,
            client: Arc::new(Mutex::new(client)),
        };

        let app = Router::new()
            .route("/", get(root))
            .route("/health", get(health))
            .route("/predict", post(predict))
            .with_state(state);
        // .layer(HandleErrorLayer::new(handle_error));

        let addr: SocketAddr = format!("{}:{}", self.host, self.port).parse()?;
        let listener = tokio::net::TcpListener::bind(&addr).await?;

        let server_future = axum::serve(listener, app);
        tokio::select! {
            _ = server_future => {
                error!("Server exited unexpectedly");
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Server exited unexpectedly",
                )));
            }
            _ = runner_handle => {
                error!("Runner exited unexpectedly");
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Runner exited unexpectedly",
                )));
            }
        }

        Ok(())
    }
}

async fn handle_error(err: BoxError) -> (StatusCode, String) {
    if err.is::<tower::timeout::error::Elapsed>() {
        (
            StatusCode::REQUEST_TIMEOUT,
            "Request took too long".to_string(),
        )
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Unhandled internal error: {err}"),
        )
    }
}
// basic handler that responds with a static string
async fn root() -> &'static str {
    "Hello, World!"
}

async fn health(State(state): State<AppState>) -> Json<Health> {
    Json(state.health)
}

use futures::future::poll_fn;

async fn predict(
    State(state): State<AppState>,
    Json(request): Json<PredictionRequest>,
) -> Result<Json<PredictionResponse>, StatusCode> {
    debug!("predict {:?}", request.clone());
    let mut client = state.client.lock().await;

    // Poll for readiness
    match poll_fn(|cx| client.poll_ready(cx)).await {
        Ok(()) => {
            // Client is ready, make the call
            match client.call(request).await {
                Ok(response) => Ok(Json(response)),
                Err(e) => {
                    error!("Error calling runner: {:?}", e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
        Err(_) => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}
