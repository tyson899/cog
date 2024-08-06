use anyhow::Result;
use cog::runner::Runner;
use pyo3::PyErr;
use pyo3_asyncio_0_21::tokio as pyo3_asyncio;
use tokio::time::{timeout, Duration};
use tower::Service;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

// #[tokio::main]
#[pyo3_asyncio::main]
async fn main() -> Result<(), PyErr> {
    tracing_subscriber::fmt()
        .with_env_filter("trace".parse::<EnvFilter>().unwrap())
        .without_time()
        .with_target(false)
        .init();

    let code = include_str!("../assets/async_predict.py");
    let mut service = Runner::new(code.to_string());

    let timeout = timeout(Duration::from_millis(2000), service.start()).await;
    if let Err(e) = timeout {
        error!("Service setup timed out: {e}");
        return Ok(());
        // return Err(anyhow::anyhow!("Service setup timed out: {e}"));
    }

    for i in 0..100 {
        // let payload = hashmap! {
        //     "num" => i.into(),
        // };
        let response = service.call("Hello, Tower!".into()).await?;
        info!("Response: {response}")
    }

    Ok(())
}
