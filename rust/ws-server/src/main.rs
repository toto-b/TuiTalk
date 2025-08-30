mod wsserver;
mod database;
mod redis;

use openssl_sys as _;
use pq_sys as _;

use dotenvy::dotenv;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok(); 

    let server_handle = tokio::spawn(async move {
        wsserver::start_ws_server().await.expect("Server failed");
    });

    tokio::select! {
        _ = server_handle => println!("Server stopped"),
    }

    Ok(())
}
