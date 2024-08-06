use anyhow::{Error, Result};
use tokio::sync::OnceCell;
use tokio::time::{sleep, timeout, Duration};
pub use tower::Service;

use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;

use maplit::hashmap;
use pyo3::prelude::*;
use pyo3::types::{IntoPyDict, PyDict, PyTuple};
use pyo3_asyncio_0_21::tokio as pyo3_asyncio;

use pythonize::pythonize;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use std::{
    convert::Infallible,
    fmt::{self, Display},
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

/// A request to the [EchoService], just wrapping a `String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PredictionRequest(String);

/// Delegate to the wrapped `String`.
impl Display for PredictionRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Anything that can be turned into a `String` can be turned into a [EchoRequest].
impl<T> From<T> for PredictionRequest
where
    T: Into<String>,
{
    fn from(text: T) -> Self {
        PredictionRequest(text.into())
    }
}

/// A response from the [EchoService], just wrapping a `String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PredictionResponse(String);

/// Delegate to the wrapped `String`.
impl Display for PredictionResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

use crate::probes::create_readiness_probe;
use crate::py::stream::redirect_streams;

/// Echo service, responding to an [EchoRequest] with an [EchoResponse] with the same content.
#[derive(Debug, Default)]
pub struct Runner {
    code: String,
    predictor: OnceCell<Py<PyAny>>,
    predict_function_is_coroutine: OnceCell<bool>,
}

impl Runner {
    pub fn new(code: String) -> Self {
        Runner {
            code: code,
            predictor: OnceCell::new(),
            predict_function_is_coroutine: OnceCell::new(),
        }
    }

    pub async fn start(&self) -> Result<()> {
        // Step 1: Redirect Python stdout/stderr streams to Rust
        debug!("Redirecting Python stdout/stderr streams to Rust");
        let (mut stdout_rx, mut stderr_rx) = Python::with_gil(|py| redirect_streams(py))?;
        tokio::spawn(async move {
            while let Some(message) = stdout_rx.recv().await {
                info!("Python stdout: {}", message);
            }
        });
        tokio::spawn(async move {
            while let Some(message) = stderr_rx.recv().await {
                info!("Python stderr: {}", message);
            }
        });

        // Step 2: Initialize module
        debug!("Initializing module");
        let module = Python::with_gil(|py| -> PyResult<Py<PyModule>> {
            let module = PyModule::from_code_bound(py, &self.code, "predict.py", "model")?;
            Ok(module.into())
        })?;

        // Step 3: Get setup function and check if it's a coroutine
        debug!("Getting setup function and checking if it's a coroutine");
        let setup = Python::with_gil(|py| -> PyResult<Option<(Py<PyAny>, Py<PyAny>)>> {
            // Create an instance of the Predictor class
            let predictor_class = module.getattr(py, "Predictor").map_err(|e| {
                error!("Failed to get Predictor class: {:?}", e);
                e
            })?;
            let predictor_instance = predictor_class.call0(py).map_err(|e| {
                error!("Failed to create Predictor instance: {:?}", e);
                e
            })?;
            self.predictor.set(predictor_instance.clone()).unwrap();

            let inspect = py.import_bound("inspect").map_err(|e| {
                error!("Failed to import inspect module: {:?}", e);
                e
            })?;
            let is_coroutine = inspect.getattr("iscoroutinefunction").map_err(|e| {
                error!("Failed to get iscoroutinefunction attribute: {:?}", e);
                e
            })?;

            let predict_function = predictor_instance.getattr(py, "predict").map_err(|e| {
                error!("Failed to get predict function: {:?}", e);
                e
            })?;
            let predict_function_is_coroutine = is_coroutine
                .call1((predict_function.clone(),))
                .map_err(|e| {
                    error!("Failed to check if predict function is coroutine: {:?}", e);
                    e
                })?;

            let setup_function = predictor_instance.getattr(py, "setup").map_err(|e| {
                error!("Failed to get setup function: {:?}", e);
                e
            })?;
            self.predict_function_is_coroutine
                .set(predict_function_is_coroutine.extract::<bool>()?)
                .unwrap();

            let setup_is_coroutine = is_coroutine
                .call1((setup_function.clone(),))
                .and_then(|result| result.extract::<bool>())
                .map_err(|e| {
                    error!("Failed to check if setup is coroutine: {:?}", e);
                    e
                })?;

            if setup_is_coroutine {
                Ok(Some((
                    setup_function.into_py(py),
                    predictor_instance.into_py(py),
                )))
            } else {
                debug!("Setup function is sync, calling now");
                setup_function.call0(py)?;
                Ok(None)
            }
        })?;

        if let Some((setup, predictor_instance)) = setup {
            debug!("Setup function is async");
            let setup_rust_future = Python::with_gil(|py| {
                let result = setup.call_method1(py, "setup", (predictor_instance,))?;
                pyo3_asyncio::into_future(result.extract(py)?)
            });
            match setup_rust_future {
                Ok(setup_rust_future) => {
                    debug!("Running setup function");
                    setup_rust_future
                        .await
                        .map_err(|e| error!("Error during async setup: {:?}", e))
                        .ok();
                    debug!("Setup function completed");
                }
                Err(e) => return Err(Error::msg(format!("Error during async setup: {:?}", e))),
            }
        } else {
            debug!("Setup function is sync");
        }

        // // Step 3: Import the predict module and set up the predictor
        // let setup_future = Python::with_gil(|py| -> PyResult<Option<PyObject>> {
        //     let predictor_module =
        //         PyModule::from_code_bound(py, &self.code, "predict.py", "model")?;
        //     let predictor_class = predictor_module.getattr("Predictor")?;
        //     let predictor_instance = predictor_class.call0()?;

        //     info!("Loaded predictor");

        //     let inspect = py.import_bound("inspect")?;
        //     let is_coroutine = inspect.getattr("iscoroutinefunction")?;

        //     let setup_function = predictor_instance.getattr("setup")?;
        //     let setup_is_coroutine = is_coroutine
        //         .call1((setup_function.clone(),))?
        //         .extract::<bool>()?;

        //     let predict_function = predictor_instance.getattr("predict")?;
        //     let predict_is_coroutine = is_coroutine
        //         .call1((predict_function.clone(),))?
        //         .extract::<bool>()?;

        //     self.predict_function
        //         .set(predict_function.into_py(py))
        //         .unwrap();
        //     self.predict_function_is_coroutine
        //         .set(predict_is_coroutine)
        //         .unwrap();

        //     if !setup_is_coroutine {
        //         setup_function.call0()?;
        //         Ok(None)
        //     } else {
        //         Ok(Some(setup_function.into_py(py)))
        //     }
        // })?;

        // // Step 4: Handle async setup if necessary
        // if let Some(setup_py_future) = setup_future {
        //     let setup_rust_future = Python::with_gil(|py| {
        //         pyo3_asyncio::into_future(setup_py_future.call0(py, (), None))
        //     })?;

        //     setup_rust_future
        //         .await
        //         .map_err(|e| Error::msg(format!("Error during async setup: {:?}", e)))?;
        // }

        // Step 5: Create readiness probe
        match create_readiness_probe() {
            Ok(true) => info!("Readiness probe created"),
            Ok(false) => info!("Not running in Kubernetes: disabling readiness probe."),
            Err(e) => error!("Failed to create readiness probe: {:?}", e),
        }

        Ok(())
    }
}

impl Service<PredictionRequest> for Runner {
    type Response = PredictionResponse;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.predictor.get().is_some() {
            Poll::Ready(Ok(()))
        } else {
            ctx.waker().wake_by_ref();
            Poll::Pending
        }
    }

    fn call(&mut self, req: PredictionRequest) -> Self::Future {
        let predictor = self.predictor.get().unwrap();
        let predict_function_is_coroutine = self.predict_function_is_coroutine.get().unwrap();

        if *predict_function_is_coroutine {
            debug!("Predict function is coroutine");

            debug!("Setup function is async");
            let predict_rust_future = Python::with_gil(|py| {
                let result =
                    predictor.call_method_bound(py, "predict", (predictor.clone(),), None)?;
                pyo3_asyncio::into_future(result.extract(py)?)
            });
            match predict_rust_future {
                Ok(predict_rust_future) => {
                    debug!("Running predict function");

                    let pin = Box::pin(async move { predict_rust_future });
                    // let ctx = &mut Context::from_waker(noop_waker_ref());
                    // pin.poll_unpin(ctx)
                }
                Err(e) => error!("Error during async setup: {:?}", e),
            }
        } else {
            warn!("Predict function is sync; UNIMPLEMENTED");
        }

        // let predict_function = self.predict_function.get().unwrap().clone();
        // Box::pin(async move {
        //     let predict_result = match self.predict_function_is_coroutine.get() {
        //         Some(true) => {
        //             let predict_rust_future = Python::with_gil(|py| {
        //                 let kwargs = PyDict::new_bound(py);
        //                 kwargs.set_item("num", 10);
        //                 pyo3_asyncio::into_future(predict_function.call_method_bound(
        //                     py,
        //                     (),
        //                     kwargs,
        //                 ))
        //             })?;

        //             predict_rust_future
        //                 .await
        //                 .map_err(|e| Error::msg(format!("Error during async setup: {:?}", e)))?;
        //         }
        //         Some(false) => {
        //             let kwargs = PyDict::new_bound(py);
        //             kwargs.set_item("num", 10);
        //             predict_function.call_bound(py, (), None)?;
        //         }
        //         None => {
        //             error!("No predict function found");
        //             Ok(PredictionResponse("NOT OK".to_string()))
        //         }
        //     };

        //     match predict_result {
        //         Ok(result) => {
        //             info!("Result: {:?}", result.extract::<i32>(py));
        //             Ok(PredictionResponse("OK".to_string()))
        //         }
        //         Err(e) => {
        //             error!("Error: {}", e);
        //             Ok(PredictionResponse("NOT OK".to_string()))
        //         }
        //     }
        // })
        Box::pin(async move { Ok(PredictionResponse("OK".to_string())) })
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use futures::task::noop_waker_ref;
//     use std::task;

//     #[tokio::test]
//     async fn test_poll_ready() {
//         let result = Runner::default().poll_ready(&mut task::Context::from_waker(noop_waker_ref()));
//         assert!(result.is_ready());
//         if let Poll::Ready(result) = result {
//             assert!(result.is_ok());
//         }
//     }

//     #[tokio::test]
//     async fn test_call() {
//         let response = Runner::default().call("Tower".into()).await;
//         assert!(response.is_ok());
//         let response = response.unwrap();
//         assert_eq!(response, EchoResponse("Tower".to_string()));
//     }
// }
