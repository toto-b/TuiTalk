mod wsserver;
use std::sync::Arc;

use redis::Connection;
use tokio::sync::Mutex;
type SharedRedis = Arc<Mutex<Connection>>;

#[tokio::main]
async fn main() -> redis::RedisResult<()> {
    let server_handle = tokio::spawn(async move {
        wsserver::start_ws_server().await.expect("Server failed");
    });

    tokio::select! {
        _ = server_handle => println!("Server stopped"),
    }

    Ok(())
}
