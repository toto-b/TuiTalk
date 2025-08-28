mod wsserver;
mod database;
mod redis;

use openssl_sys as _;
use pq_sys as _;

use ::redis::RedisResult;
use dotenvy::dotenv;

#[tokio::main]
async fn main() -> RedisResult<()> {
    dotenv().ok(); 

    let server_handle = tokio::spawn(async move {
        wsserver::start_ws_server().await.expect("Server failed");
    });

    tokio::select! {
        _ = server_handle => println!("Server stopped"),
    }

    Ok(())
}
