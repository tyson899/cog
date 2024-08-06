use std::{future::pending, time};

use chrono::{DateTime, Utc};
use futures::stream;
use kameo::{
    actor::{ActorPool, ActorRef, UnboundedMailbox},
    error::BoxError,
    message::{Context, Message, StreamMessage},
    Actor,
};
use maplit::hashmap;
use pyo3::prelude::*;
use pyo3::types::{IntoPyDict, PyDict, PyTuple};
use pythonize::pythonize;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, Semaphore};
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};
use tokio_stream::StreamExt;
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;

const EXAMPLE_PREDICTOR: &'static str = include_str!("../assets/predict.py");

use pyo3::types::PyModule;

#[pyclass]
#[derive(Default)]
struct LoggingStdout;

#[pymethods]
impl LoggingStdout {
    fn write(&self, data: &str) {
        println!("stdout from python: {:?}", data);
    }
}

pub struct Runner {
    concurrency: usize,

    predictor: Option<PyObject>,
    stdout_tx: Option<mpsc::Sender<String>>,
    stderr_tx: Option<mpsc::Sender<String>>,

    pool: Option<ActorPool<Worker>>,
    // Add other fields as needed
}

impl Runner {
    pub fn new(concurrency: usize) -> Self {
        Runner {
            concurrency,
            predictor: None,
            stdout_tx: None,
            stderr_tx: None,
            pool: None,
        }
    }

    // fn spawn_stream_task(
    //     stream: PyObject,
    //     tx: mpsc::Sender<String>,
    //     stream_name: &'static str,
    // ) -> JoinHandle<()> {
    //     tokio::spawn(async move {
    //         loop {
    //             let output = Python::with_gil(|py| {
    //                 stream.call_method0(py, "getvalue")?.extract::<String>(py)
    //             });
    //             if let Ok(value) = output {
    //                 if !value.is_empty() {
    //                     tx.send(value).await.unwrap();
    //                     Python::with_gil(|py| {
    //                         stream.call_method1(py, "truncate", (0,))?;
    //                         stream.call_method1(py, "seek", (0,))?;
    //                         Ok::<_, PyErr>(())
    //                     })
    //                     .unwrap();
    //                 }
    //             }
    //             tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    //         }
    //     })
    // }
}

impl Actor for Runner {
    type Mailbox = UnboundedMailbox<Self>;

    async fn on_start(&mut self, actor_ref: ActorRef<Self>) -> Result<(), BoxError> {
        // let stream = Box::pin(
        //     stream::repeat(1)
        //         .take(5)
        //         .throttle(time::Duration::from_secs(1)),
        // );
        // actor_ref.attach_stream(stream, "1st stream", "1st stream");

        // let stream = stream::repeat(1).take(5);
        // actor_ref.attach_stream(stream, "2nd stream", "2nd stream");

        let current_dir = std::env::current_dir()?;
        let cog_path = current_dir.join("../python/cog").canonicalize()?;
        let venv_path = current_dir.join("../.venv").canonicalize()?;

        debug!("Current directory: {:?}", current_dir);
        debug!("Cog path: {:?}", cog_path);
        debug!("Venv path: {:?}", venv_path);

        let result = Python::with_gil(|py| {
            let sys = py.import_bound("sys")?;

            // Add cog_path to sys.path
            sys.getattr("path")?
                .call_method1("insert", (0, cog_path.to_str().unwrap()))?;

            // Print sys.path for debugging
            println!(
                "sys.path: {:?}",
                sys.getattr("path")?.extract::<Vec<String>>()?
            );

            // // Activate virtual environment
            // activate_venv(py, &venv_path)?;

            // match py.import_bound("cog") {
            //     Ok(_) => println!("Successfully imported 'cog' module"),
            //     Err(e) => {
            //         println!("Failed to import 'cog' module: {:?}", e);
            //         return Err(e);
            //     }
            // }

            warn!("using hard-coded predictor");

            // Use PyModule::from_code_bound to import the predict module
            match PyModule::from_code_bound(py, EXAMPLE_PREDICTOR, "predict.py", "model") {
                Ok(predictor_module) => {
                    let predictor_class = predictor_module.getattr("Predictor")?;
                    let predictor_instance = predictor_class.call0()?;
                    self.predictor = Some(predictor_instance.into_py(py));

                    info!("Loaded predictor");
                    // // Create channels for stdout and stderr
                    // let (stdout_tx, mut stdout_rx) = mpsc::channel(100);
                    // let (stderr_tx, mut stderr_rx) = mpsc::channel(100);

                    // Create RustLogger instances
                    let stdout_logger = Py::new(py, LoggingStdout::default())?;
                    let stderr_logger = Py::new(py, LoggingStdout::default())?;

                    // Set up Python loggers
                    let sys = py.import_bound("sys")?;
                    sys.setattr("stdout", stdout_logger)?;
                    sys.setattr("stderr", stderr_logger)?;

                    // let logging = py.import_bound("logging")?;
                    //         py_run!(
                    //             py,
                    //             logging,
                    //             r#"
                    //     import logging
                    //     logging.basicConfig(stream=sys.stderr, level=logging.INFO)
                    // "#
                    //         );

                    // // Store the channel senders
                    // self.stdout_tx = Some(stdout_tx);
                    // self.stderr_tx = Some(stderr_tx);

                    // // Spawn tasks to handle received messages
                    // tokio::spawn(async move {
                    //     while let Some(msg) = stdout_rx.recv().await {
                    //         info!("stdout: {}", msg);
                    //     }
                    // });

                    // tokio::spawn(async move {
                    //     while let Some(msg) = stderr_rx.recv().await {
                    //         error!("stderr: {}", msg);
                    //     }
                    // });

                    let setup_function = self
                        .predictor
                        .as_ref()
                        .unwrap()
                        .clone_ref(py)
                        .getattr(py, "setup")?;

                    match setup_function.call0(py) {
                        Ok(_) => Ok(()),
                        Err(e) => {
                            error!("Error during setup: {:?}", e);
                            e.print(py);
                            Err(e)
                        }
                    }?;

                    let predict_function =
                        self.predictor.as_ref().unwrap().getattr(py, "predict")?;

                    self.pool = Some(ActorPool::new(self.concurrency, move || {
                        kameo::spawn(Worker {
                            predict_function: predict_function.clone(),
                        })
                    }));

                    Ok(())
                }
                Err(e) => {
                    error!("Failed to import model module: {:?}", e);
                    Err(e)
                }
            }
        });

        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

struct Worker {
    predict_function: PyObject,
}

impl Actor for Worker {
    type Mailbox = UnboundedMailbox<Self>;
    fn max_concurrent_queries() -> usize {
        1
    }

    async fn on_start(&mut self, actor_ref: ActorRef<Self>) -> Result<(), BoxError> {
        info!("Worker started");

        Ok(())
    }
}

struct Input {
    payload: HashMap<&'static str, Value>,
}

impl Message<Input> for Runner {
    type Reply = ();

    async fn handle(&mut self, msg: Input, _ctx: Context<'_, Self, Self::Reply>) -> Self::Reply {
        info!("Runner received message: {:?}", msg.payload);

        if let Some(pool) = &self.pool {
            // Use the execute method to assign the task to an available worker
            _ = match pool.get_worker().tell(msg).send() {
                Ok(_) => Ok(()),
                Err(e) => {
                    error!("Error sending message: {:?}", e);
                    Err(e)
                }
            };
        } else {
            error!("Worker pool is not initialized");
        }
    }
}

impl Message<Input> for Worker {
    type Reply = ();

    async fn handle(&mut self, msg: Input, ctx: Context<'_, Self, Self::Reply>) -> Self::Reply {
        info!(
            "Worker {} received message: {:?}",
            ctx.actor_ref().id(),
            msg.payload
        );

        // Call Python predictor
        let result = Python::with_gil(|py| {
            let kwargs = PyDict::new_bound(py);
            for (key, value) in &msg.payload {
                kwargs.set_item(key, pythonize(py, value).unwrap())?;
            }
            // Call the prediction function
            let result = self.predict_function.call_bound(py, (), Some(&kwargs))?;

            result.extract::<i32>(py)
        });

        match result {
            Ok(output) => {
                info!("Output: {:?}", output);
            }
            Err(e) => {
                error!("Error: {}", e);
            }
        }
    }
}

// impl Message<StreamMessage<i64, &'static str, &'static str>> for Runner {
//     type Reply = ();

//     async fn handle(
//         &mut self,
//         msg: StreamMessage<i64, &'static str, &'static str>,
//         _ctx: Context<'_, Self, Self::Reply>,
//     ) -> Self::Reply {
//         match msg {
//             StreamMessage::Next(amount) => {
//                 self.count += amount;
//                 info!("Count is {}", self.count);
//             }
//             StreamMessage::Started(s) => {
//                 info!("Started {s}");
//             }
//             StreamMessage::Finished(s) => {
//                 info!("Finished {s}");
//             }
//         }
//     }
// }

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter("trace".parse::<EnvFilter>().unwrap())
        .without_time()
        .with_target(false)
        .init();

    let runner = Runner::new(5);

    let addr = kameo::spawn(runner);

    for i in 0..100 {
        let payload = hashmap! {
            "num" => i.into(),
        };
        addr.ask(Input { payload: payload }).send().await.unwrap();
    }

    pending().await
}
