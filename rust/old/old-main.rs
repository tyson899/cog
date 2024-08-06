//! Run with
//!
//! ```not_rust
//! cargo run -p example-hello-world
//! ```

use axum::extract::State;
use axum::{response::Html, routing::get, Router};
use chrono::Utc;
// use cog::runner::RunnerHandle;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// struct AppState {
//     runner: RunnerHandle,
//     tx: broadcast::Sender<String>,
// }

// #[tokio::main]
// async fn main() {
//     tracing_subscriber::registry()
//         .with(
//             tracing_subscriber::EnvFilter::try_from_default_env()
//                 .unwrap_or_else(|_| "example_chat=trace".into()),
//         )
//         .with(tracing_subscriber::fmt::layer())
//         .init();

//     let runner = RunnerHandle::new();
//     let (tx, _rx) = broadcast::channel(100);
//     let app_state = Arc::new(AppState { runner, tx });

//     // build our application with a route
//     let app = Router::new().route("/", get(handler)).with_state(app_state);

//     // run it
//     let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
//         .await
//         .unwrap();
//     println!("listening on {}", listener.local_addr().unwrap());
//     axum::serve(listener, app).await.unwrap();
// }

// async fn handler(State(state): State<Arc<AppState>>) -> Html<String> {
//     let runner = state.runner.clone();
//     let timestamp = chrono::Utc::now().to_rfc3339();
//     let message = runner.echo(timestamp).await;
//     Html(format!("<h1>{}</h1>", message))
// }

// use cog::runner::Runner;

// #[tokio::main]
// async fn main() {
//     // Create a mock process (replace with actual process creation)
//     let process = std::process::Command::new("echo").spawn().unwrap();

//     println!("process: {:?}", process);

//     let (runner, predict_tx) = Runner::new(process);

//     // Spawn the runner
//     tokio::spawn(runner);

//     // Send prediction requests
//     predict_tx.send("Hello".to_string()).await.unwrap();
//     predict_tx.send("World".to_string()).await.unwrap();

//     // Wait for the runner to finish
//     tokio::time::sleep(std::time::Duration::from_secs(15)).await;

//     // Close the channel to signal the runner to finish
//     drop(predict_tx);

//     // Allow some time for the runner to process the channel closure
//     tokio::time::sleep(std::time::Duration::from_secs(1)).await;

//     println!("Runner finished");
// }
