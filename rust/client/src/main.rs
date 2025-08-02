use futures_channel::mpsc::{unbounded, UnboundedSender};
use std::time::SystemTime;
use tokio::signal;
pub use shared::native::{connect, receiver_task, sender_task };
use shared::{TalkProtocol, ClientAction};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = "ws://localhost:8080".to_string();

    // Create channel for sending messages
    let (tx, rx) = unbounded::<TalkProtocol>();

    // Connect to WebSocket server
    let (write, read) = connect(url).await?; 

    // Spawn sender task
    tokio::spawn(sender_task(rx, write));


    send_example_messages(tx.clone()).await;

    tokio::spawn(receiver_task(read, |msg| {
        println!("Received message: {:?}", msg);
    }));

    signal::ctrl_c().await?;
    println!("Shutting down...");


    Ok(())
}

async fn send_example_messages(tx: UnboundedSender<TalkProtocol>) {
    let msg1 = TalkProtocol {
        username: "client".to_string(),
        message: "Hello server!".to_string(),
        action: None,
        room_id: 0,
        unixtime: SystemTime::now(),
    };
    tx.unbounded_send(msg1).unwrap();

    let msg2 = TalkProtocol {
        username: "client".to_string(),
        message: "I want to join room".to_string(),
        action: Some(ClientAction::Join),
        room_id: 42,
        unixtime: SystemTime::now(),
    };
    tx.unbounded_send(msg2).unwrap();

}
