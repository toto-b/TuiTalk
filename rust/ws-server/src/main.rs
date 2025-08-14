mod wsserver;
use std::sync::Arc;

use redis::Connection;
use tokio::sync::Mutex;
type SharedRedis = Arc<Mutex<Connection>>;

#[tokio::main]
async fn main() -> redis::RedisResult<()> {
    let client = redis::Client::open("redis://0.0.0.0/")?;
    let conn = client.get_connection()?;
    println!("Connected to Redis");
    let shared_con: SharedRedis = Arc::new(Mutex::new(conn));

    let server_handle = tokio::spawn(async move {
        let redis_clone = Arc::clone(&shared_con);
        wsserver::start_ws_server(redis_clone).await.expect("Server failed");
    });

    tokio::select! {
        _ = server_handle => println!("Server stopped"),
    }

    Ok(())
}
