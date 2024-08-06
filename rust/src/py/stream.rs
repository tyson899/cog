use pyo3::prelude::*;
use tokio::sync::mpsc;

#[pyclass]
pub struct StandardStream {
    sender: mpsc::Sender<String>,
}

impl StandardStream {
    pub fn new(sender: mpsc::Sender<String>) -> Self {
        StandardStream { sender }
    }
}

#[pymethods]
impl StandardStream {
    pub(crate) fn write(&self, data: &str) {
        let _ = self.sender.try_send(data.to_string());
    }
}

pub fn redirect_streams(
    py: Python<'_>,
) -> PyResult<(mpsc::Receiver<String>, mpsc::Receiver<String>)> {
    let (stdout_tx, stdout_rx) = mpsc::channel::<String>(100);
    let (stderr_tx, stderr_rx) = mpsc::channel::<String>(100);

    // Set up Python loggers
    let stdout = StandardStream::new(stdout_tx.clone());
    let stderr = StandardStream::new(stderr_tx.clone());

    let sys = py.import_bound("sys")?;
    sys.setattr("stdout", stdout.into_py(py))?;
    sys.setattr("stderr", stderr.into_py(py))?;

    Ok((stdout_rx, stderr_rx))
}
