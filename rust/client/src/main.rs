use futures_channel::mpsc::{UnboundedSender, unbounded};
pub use shared::native::{connect, receiver_task, sender_task};
use shared::{ClientAction, TalkProtocol};
use tokio::signal;
use tokio::io::AsyncReadExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = "ws://localhost:8080".to_string();

    let (tx, rx) = unbounded::<TalkProtocol>();

    let (write, read) = connect(url).await?;

    tokio::spawn(sender_task(rx, write));

    send_example_messages(tx.clone()).await;

    tokio::spawn(receiver_task(read, |msg| {
        println!("Received message: {:?}", msg);
    }));

    tokio::spawn({
        read_stdin(tx)
    });

    signal::ctrl_c().await?;
    println!("Shutting down...");

    Ok(())
}

async fn read_stdin(tx: UnboundedSender<TalkProtocol>) {
    let mut stdin = tokio::io::stdin();
    loop {
        let mut buf = vec![0; 1024];
        let n = match stdin.read(&mut buf).await {
            Err(_) | Ok(0) => break,
            Ok(n) => n,
        };

        buf.truncate(n);

        let stuff_str = String::from_utf8_lossy(&buf).to_string();

        let msg = TalkProtocol {
            username: "Stdin User".to_string(),
            action: None,
            room_id: 0,
            unixtime: 1,
            message: stuff_str
        };
        tx.unbounded_send(msg).unwrap();
    }
}

async fn send_example_messages(tx: UnboundedSender<TalkProtocol>) {
    let msg1 = TalkProtocol {
        username: "client".to_string(),
        message: "Hello server and others!".to_string(),
        action: None,
        room_id: 0,
        unixtime: 100,
    };
    tx.unbounded_send(msg1).unwrap();

    let msg2 = TalkProtocol {
        username: "client".to_string(),
        message: "I want to join room".to_string(),
        action: Some(ClientAction::Join),
        room_id: 42,
        unixtime: 100,
    };
    tx.unbounded_send(msg2).unwrap();
}
