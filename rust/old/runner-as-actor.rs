// use tokio::sync::{mpsc, oneshot};

// struct Runner {
//     receiver: mpsc::Receiver<Event>,
// }
// enum Event {
//     Echo {
//         message: String,
//         respond_to: oneshot::Sender<String>,
//     },
// }

// impl Runner {
//     fn new(receiver: mpsc::Receiver<Event>) -> Self {
//         Runner { receiver }
//     }

//     async fn handle(&mut self, event: Event) {
//         match event {
//             Event::Echo {
//                 message,
//                 respond_to,
//             } => {
//                 // The `let _ =` ignores any errors when sending.
//                 //
//                 // This can happen if the `select!` macro is used
//                 // to cancel waiting for the response.
//                 tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
//                 let _ = respond_to.send(format!("Echo: {}", message));
//             }
//         }
//     }
// }

// async fn run_runner(mut runner: Runner) {
//     while let Some(event) = runner.receiver.recv().await {
//         runner.handle(event).await;
//     }
// }

// #[derive(Clone)]
// pub struct RunnerHandle {
//     sender: mpsc::Sender<Event>,
// }

// impl RunnerHandle {
//     pub fn new() -> Self {
//         let (sender, receiver) = mpsc::channel(8);
//         let actor = Runner::new(receiver);
//         tokio::spawn(run_runner(actor));

//         Self { sender }
//     }

//     pub async fn echo(&self, message: String) -> String {
//         let (send, recv) = oneshot::channel();
//         let msg = Event::Echo {
//             message: message,
//             respond_to: send,
//         };

//         // Ignore send errors. If this send fails, so does the
//         // recv.await below. There's no reason to check the
//         // failure twice.
//         let _ = self.sender.send(msg).await;
//         recv.await.expect("Actor task has been killed")
//     }
// }
