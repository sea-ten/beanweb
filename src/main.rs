//! Beanweb main entry point

use beanweb_api::start_server;
use beanweb_config::Config;
use beanweb_core::Ledger;
use beanweb_parser::DefaultBeancountParser;
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::runtime::Runtime;

#[derive(Parser, Debug)]
#[command(name = "beanweb")]
#[command(author = "Beanweb Contributors")]
#[command(version = "0.1.0")]
#[command(about = "A lightweight, high-performance Beancount web interface", long_about = None)]
struct Args {
    /// Configuration file path
    #[arg(short, long, default_value = "config.yaml")]
    config: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let args = Args::parse();
    let rt = Runtime::new()?;

    rt.block_on(async {
        let config = Config::load(args.config.clone())
            .expect("Failed to load configuration");

        eprintln!("[INFO] Config loaded: data path={}, main_file={}",
            config.data.path.to_string_lossy(), config.data.main_file);

        let parser = Arc::new(DefaultBeancountParser::default());
        let ledger = Arc::new(RwLock::new(Ledger::new(config.clone(), parser)));

        // Try to load the ledger if the data directory exists
        let data_path = config.data.path.join(&config.data.main_file);
        eprintln!("[INFO] Looking for ledger file: {}", data_path.to_string_lossy());

        if data_path.exists() {
            eprintln!("[INFO] Ledger file found, loading...");
            let mut ledger_guard = ledger.write().await;
            let result = ledger_guard.load(data_path).await;
            match result {
                Ok(_) => eprintln!("[INFO] Ledger loaded successfully"),
                Err(e) => eprintln!("[ERROR] Failed to load ledger: {:?}", e),
            }
        } else {
            eprintln!("[WARN] Ledger file not found: {}", data_path.display());
        }

        start_server(config, ledger).await
    });

    Ok(())
}
