use futures::{ready, Future, FutureExt};
use std::pin::Pin;
use std::process::Child;
use std::task::{Context, Poll};
use tokio::sync::mpsc;

enum RunnerState {
    Setup,
    Ready,
    Busy(String),
}

pub struct Runner {
    state: RunnerState,
    process: Child,
    predict_rx: mpsc::Receiver<String>,
}

impl Runner {
    pub fn new(process: Child) -> (Self, mpsc::Sender<String>) {
        let (predict_tx, predict_rx) = mpsc::channel(100);
        (
            Runner {
                state: RunnerState::Setup,
                process,
                predict_rx,
            },
            predict_tx,
        )
    }

    pub async fn setup(&mut self) {
        // Perform setup logic here
        println!("simulating setup");
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        println!("setup complete");
    }

    pub async fn predict(&mut self, input: String) -> String {
        // Implement prediction logic here
        println!("simulating prediction for: {}", input);
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        println!("prediction complete");
        format!("Prediction for: {}", input)
    }
}

impl Future for Runner {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match &mut self.state {
                RunnerState::Setup => {
                    // Use ready! macro to handle the future
                    ready!(Box::pin(self.setup()).poll_unpin(cx));
                    self.state = RunnerState::Ready;
                }
                RunnerState::Ready => match self.predict_rx.poll_recv(cx) {
                    Poll::Ready(Some(input)) => {
                        self.state = RunnerState::Busy(input);
                    }
                    Poll::Ready(None) => return Poll::Ready(()),
                    Poll::Pending => return Poll::Pending,
                },
                RunnerState::Busy(input) => {
                    let input = std::mem::take(input);
                    let result = ready!(Box::pin(self.predict(input)).poll_unpin(cx));
                    println!("Prediction result: {}", result);
                    self.state = RunnerState::Ready;
                }
            }
        }
    }
}

/////////////////

pub struct Prediction {
    id: usize,
    done: bool,
}

impl Drop for Prediction {
    fn drop(&mut self) {
        if !self.done {
            tokio::spawn(report_failure(self.id));
        }
    }
}

async fn report_failure(_prediction_id: usize) {
    todo!()
}
