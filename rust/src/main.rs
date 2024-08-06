use cog::runner::Runner;
use cog::server::Server;

use clap::{command, Parser};
use std::path::PathBuf;
use tracing::{debug, error, info};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short = 'C', long, default_value = ".")]
    project: PathBuf,

    #[arg(short = 'f', long, default_value = "./cog.yaml")]
    config: PathBuf,

    #[arg(short, long)]
    mode: Option<String>,

    #[arg(short = 'j', long)]
    concurrency: Option<usize>,

    #[arg(long)]
    upload_url: Option<String>,

    #[arg(long)]
    statistics: Option<String>,

    #[arg(long, default_value = "0.0.0.0")]
    host: String,

    #[arg(long, default_value = "8080")]
    port: u16,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("trace".parse::<EnvFilter>().unwrap())
        .without_time()
        .with_target(false)
        .init();

    let args = Args::parse();

    debug!("Parsed arguments:");
    debug!("  Project: {:?}", args.project);
    debug!("  Config: {:?}", args.config);
    debug!("  Mode: {:?}", args.mode);
    debug!("  Concurrency: {:?}", args.concurrency);
    debug!("  Upload URL: {:?}", args.upload_url);
    debug!("  Statistics: {:?}", args.statistics);
    debug!("  Host: {}", args.host);
    debug!("  Port: {}", args.port);

    let code = include_str!("../assets/async_predict.py");
    let runner = Runner::new(code.to_string(), 1);

    // Create and run the server
    let server = Server::new(args.host.clone(), args.port);
    info!("Running server on {}:{}", args.host, args.port);
    match server.run(runner).await {
        Ok(_) => {
            debug!("Server stopped gracefully");
        }
        Err(e) => {
            error!("Server error: {}", e);
            std::process::exit(1);
        }
    }
}
