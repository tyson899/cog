pub mod runner;

// use chrono::{DateTime, Utc};
// use pyo3::prelude::*;
// use pyo3::types::{IntoPyDict, PyDict, PyTuple};
// use serde::{Deserialize, Serialize};
// use std::collections::{HashMap, HashSet};
// use std::env;
// use std::fs;
// use std::io::{BufRead, BufReader, Write};
// use std::path::Path;
// use std::path::PathBuf;
// use std::process::{Child, Command, Stdio};
// use std::sync::{Arc, Mutex};
// use tokio::sync::{mpsc, Semaphore};
// use tokio::task::JoinHandle;
// // Enums
// #[derive(Debug, Clone, PartialEq, Eq)]
// enum WorkerState {
//     New,
//     Starting,
//     Idle,
//     Processing,
//     Busy,
//     Defunct,
// }
// pub mod runner;

// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub enum Status {
//     Processing,
//     Succeeded,
//     Failed,
//     Canceled,
// }

// // Structs
// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct PredictionRequest {
//     pub id: String,
//     // Add other fields as needed
// }

// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct PredictionResponse {
//     pub id: String,
//     pub status: Status,
//     pub output: Option<String>,
//     pub logs: String,
//     pub started_at: DateTime<Utc>,
//     pub completed_at: Option<DateTime<Utc>>,
//     pub metrics: HashMap<String, f64>,
//     // Add other fields as needed
// }

// pub struct PredictionRunner {
//     state: Arc<Mutex<WorkerState>>,
//     predictions: Arc<Mutex<HashMap<String, (PredictionResponse, JoinHandle<PredictionResponse>)>>>,
//     predictions_in_flight: Arc<Mutex<HashSet<String>>>,
//     semaphore: Arc<Semaphore>,
//     concurrency: usize,
//     predictor: Arc<Mutex<Option<PyObject>>>,
//     // Add other fields as needed
// }

// // Implement PredictionRunner
// impl PredictionRunner {
//     pub fn new(concurrency: usize) -> Self {
//         PredictionRunner {
//             state: Arc::new(Mutex::new(WorkerState::New)),
//             predictions: Arc::new(Mutex::new(HashMap::new())),
//             predictions_in_flight: Arc::new(Mutex::new(HashSet::new())),
//             semaphore: Arc::new(Semaphore::new(concurrency)),
//             concurrency,
//             predictor: Arc::new(Mutex::new(None)),
//         }
//     }

//     pub async fn setup(&self) -> PyResult<()> {
//         let current_dir = std::env::current_dir()?;
//         let project_path = current_dir
//             .join("../test-integration/test_integration/fixtures/int-project")
//             .canonicalize()?;
//         let cog_path = current_dir.join("../python/cog").canonicalize()?;
//         let venv_path = current_dir.join("../.venv").canonicalize()?;

//         println!("Current directory: {:?}", current_dir);
//         println!("Project path: {:?}", project_path);
//         println!("Cog path: {:?}", cog_path);
//         println!("Venv path: {:?}", venv_path);

//         Python::with_gil(|py| {
//             let sys = py.import_bound("sys")?;

//             // Add cog_path to sys.path
//             sys.getattr("path")?
//                 .call_method1("insert", (0, cog_path.to_str().unwrap()))?;

//             // Add project_path to sys.path
//             sys.getattr("path")?
//                 .call_method1("insert", (0, project_path.to_str().unwrap()))?;

//             // Print sys.path for debugging
//             println!(
//                 "sys.path: {:?}",
//                 sys.getattr("path")?.extract::<Vec<String>>()?
//             );

//             // // Activate virtual environment
//             // activate_venv(py, &venv_path)?;

//             // match py.import_bound("cog") {
//             //     Ok(_) => println!("Successfully imported 'cog' module"),
//             //     Err(e) => {
//             //         println!("Failed to import 'cog' module: {:?}", e);
//             //         return Err(e);
//             //     }
//             // }

//             // Read the content of predict.py
//             let predict_path = project_path.join("predict.py");
//             let predict_code = fs::read_to_string(predict_path).map_err(|e| {
//                 PyErr::new::<pyo3::exceptions::PyIOError, _>(format!(
//                     "Failed to read predict.py: {}",
//                     e
//                 ))
//             })?;

//             // Use PyModule::from_code_bound to import the predict module
//             match PyModule::from_code_bound(py, &predict_code, "predict.py", "model") {
//                 Ok(predictor_module) => {
//                     let predictor_class = predictor_module.getattr("Predictor")?;
//                     let predictor_instance = predictor_class.call0()?;
//                     *self.predictor.lock().unwrap() = Some(predictor_instance.into_py(py));
//                     println!("Successfully imported 'model' module");
//                     Ok(())
//                 }
//                 Err(e) => {
//                     println!("Failed to import model module: {:?}", e);
//                     Err(e)
//                 }
//             }
//         })
//     }

//     pub async fn predict(
//         &self,
//         request: PredictionRequest,
//     ) -> Result<PredictionResponse, Box<dyn std::error::Error>> {
//         let (tx, mut rx) = mpsc::channel(1);
//         let predictions = self.predictions.clone();
//         let predictions_in_flight = self.predictions_in_flight.clone();
//         let semaphore = self.semaphore.clone();
//         let request_id = request.id.clone();
//         let predictor = self.predictor.clone();

//         let handle = tokio::spawn(async move {
//             let _permit = semaphore.acquire().await.unwrap();
//             predictions_in_flight
//                 .lock()
//                 .unwrap()
//                 .insert(request.id.clone());

//             let mut response = PredictionResponse {
//                 id: request.id.clone(),
//                 status: Status::Processing,
//                 output: None,
//                 logs: String::new(),
//                 started_at: Utc::now(),
//                 completed_at: None,
//                 metrics: HashMap::new(),
//             };

//             // Call Python predictor
//             let result = Python::with_gil(|py| {
//                 let predictor = predictor.lock().unwrap().as_ref().unwrap().clone_ref(py);
//                 let predict_function = predictor.getattr(py, "predict")?;
//                 let kwargs = [("num", 42)].into_py_dict_bound(py);
//                 predict_function
//                     .call_bound(py, (), Some(&kwargs))?
//                     .extract::<i32>(py)
//             });

//             match result {
//                 Ok(output) => {
//                     response.output = Some(output.to_string());
//                     response.status = Status::Succeeded;
//                 }
//                 Err(e) => {
//                     response.logs = format!("Error: {}", e);
//                     response.status = Status::Failed;
//                 }
//             }

//             response.completed_at = Some(Utc::now());

//             predictions_in_flight.lock().unwrap().remove(&request.id);
//             tx.send(response.clone()).await.unwrap();
//             response
//         });

//         let response = rx.recv().await.unwrap();
//         self.predictions
//             .lock()
//             .unwrap()
//             .insert(request_id, (response.clone(), handle));

//         Ok(response)
//     }

//     pub async fn cancel(&self, prediction_id: &str) -> Result<(), Box<dyn std::error::Error>> {
//         // Implement cancellation logic
//         Ok(())
//     }

//     pub async fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
//         // Implement shutdown logic
//         Ok(())
//     }
// }

// fn activate_venv(py: Python, venv_path: &PathBuf) -> PyResult<()> {
//     let site = py.import_bound("site")?;
//     let venv_site_packages = venv_path
//         .join("lib")
//         .join("python3.x")
//         .join("site-packages");
//     site.call_method1("addsitedir", (venv_site_packages.to_str().unwrap(),))?;

//     let sys = py.import_bound("sys")?;
//     let old_sys_path = sys.getattr("path")?.extract::<Vec<String>>()?;
//     let new_sys_path: Vec<String> = old_sys_path
//         .into_iter()
//         .filter(|p| !p.contains("site-packages"))
//         .collect();
//     sys.setattr("path", new_sys_path)?;

//     Ok(())
// }
