mod wsserver;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let server_handle = tokio::spawn(async {
        wsserver::start_ws_server().await.expect("Server failed");
    });

    tokio::select! {
        _ = server_handle => println!("Server stopped"),
    }

    Ok(())
}
