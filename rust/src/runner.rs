use futures::FutureExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    sync::{mpsc, oneshot},
    time::sleep,
};
use tower::Service;
use tracing::debug;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionRequest {
    pub id: Option<i32>,
    pub input: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionResponse {
    pub id: i32,
    pub result: PredictionResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PredictionResult {
    Succeeded(Value),
    Failed(String),
    Canceled,
}

pub struct Runner {
    code: String,
    ready: Option<oneshot::Receiver<()>>,
    tx: mpsc::Sender<(PredictionRequest, oneshot::Sender<PredictionResponse>)>,
}

impl Runner {
    pub fn new(code: String, concurrency: usize) -> Self {
        let (ready_tx, ready_rx) = oneshot::channel();
        let (tx, mut rx) = mpsc::channel(concurrency);
        let runner = Self {
            code,
            ready: Some(ready_rx),
            tx,
        };

        tokio::spawn(async move {
            // Simulate loading code for 1 second
            debug!("Simulating setup...");
            sleep(Duration::from_secs(1)).await;
            debug!("Setup complete!");
            let _ = ready_tx.send(());

            while let Some((request, response_tx)) = rx.recv().await {
                let response = process_request(&request);
                if response_tx
                    .send(PredictionResponse {
                        id: request.id.unwrap_or(0),
                        result: response,
                    })
                    .is_err()
                {
                    debug!("Failed to send response, client might have dropped");
                }
            }
            debug!("All senders dropped, exiting runner task");
        });

        runner
    }
}

impl Service<PredictionRequest> for Runner {
    type Response = PredictionResponse;
    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if let Some(ready) = &mut self.ready {
            match ready.poll_unpin(cx) {
                Poll::Ready(Ok(())) => self.ready = None,
                Poll::Ready(Err(_)) => return Poll::Ready(Err("Setup failed".into())),
                Poll::Pending => return Poll::Pending,
            }
        }
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: PredictionRequest) -> Self::Future {
        debug!("Calling runner with request: {:?}", request.clone());

        let tx = self.tx.clone();
        Box::pin(async move {
            let (response_tx, response_rx) = oneshot::channel();
            debug!("Sending request to runner: {:?}", request.clone());
            tx.send((request, response_tx))
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
            debug!("Sent request to runner");
            response_rx
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        })
    }
}

fn process_request(request: &PredictionRequest) -> PredictionResult {
    if let Some(num) = request.input.get("num").and_then(Value::as_i64) {
        PredictionResult::Succeeded(json!(num * 2))
    } else {
        PredictionResult::Failed("Invalid input".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_runner() {
        let mut runner = Runner::new("test code".to_string(), 1);

        // Wait for the service to be ready
        runner.ready().await.unwrap();

        for num in 0..=10 {
            let request = PredictionRequest {
                id: Some(num),
                input: json!({"num": num}),
            };

            let expected = num * 2;

            let response = runner.call(request).await.unwrap();
            match response.result {
                PredictionResult::Succeeded(output) => assert_eq!(output, json!(expected)),
                _ => panic!("Unexpected response: {:?}", response),
            }
        }
    }
}
