use redis::cluster::{ClusterClient, ClusterClientBuilder, ClusterConnection};
use redis::cluster_async::ClusterConnection as ClusterConnectionAsync;
use redis::{PushInfo, Value};
use shared::TalkProtocol;
use std::{env, sync::Arc};
use tokio::sync::mpsc::UnboundedSender as TUnboundedSender;
use tokio::sync::{Mutex as TMutex, mpsc::UnboundedReceiver as TUnboundedReceiver};
use tokio_tungstenite::tungstenite::protocol::Message;

pub type SharedRedis = Arc<TMutex<ClusterConnection>>;

pub async fn create_redis_async_pubsub_connection()
-> Result<(ClusterConnectionAsync, TUnboundedReceiver<PushInfo>), redis::RedisError> {
    let nodes = env::var("REDIS_NODES")
        .unwrap_or_else(|_| "localhost:7001,localhost:7002,localhost:7003".to_string());
    let node_urls: Vec<String> = nodes
        .split(',')
        .map(|s| format!("redis://{}/?protocol=3", s))
        .collect();

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let client = ClusterClientBuilder::new(node_urls)
        .use_protocol(redis::ProtocolVersion::RESP3)
        .push_sender(tx)
        .build()?;

    let connection = client
        .get_async_connection()
        .await
        .expect("Async Cluster Connection");

    Ok((connection, rx))
}

pub async fn create_redis_connection() -> Result<ClusterConnection, redis::RedisError> {
    let nodes = env::var("REDIS_NODES")
        .unwrap_or_else(|_| "localhost:7001,localhost:7002,localhost:7003".to_string());
    let node_urls: Vec<String> = nodes.split(',').map(|s| format!("redis://{}", s)).collect();

    let client = ClusterClient::new(node_urls).unwrap();
    let publish_conn = client.get_connection().expect("Redis Connection");
    Ok(publish_conn)
}

fn extract_binary_payload_from_pmessage(data: Vec<Value>) -> Option<Vec<u8>> {
    // PMessage data format: [pattern, channel, binary_payload]
    if data.len() >= 3 {
        if let Value::BulkString(binary_data) = &data[2] {
            return Some(binary_data.clone());
        }
    }
    None
}

fn extract_binary_payload_from_message(data: Vec<Value>) -> Option<Vec<u8>> {
    // PMessage data format: [pattern, channel, binary_payload]
    if data.len() >= 2 {
        if let Value::BulkString(binary_data) = &data[1] {
            return Some(binary_data.clone());
        }
    }
    None
}

pub async fn subscribe_to_redis_pattern(tx: TUnboundedSender<Message>) {
    let r = create_redis_async_pubsub_connection().await;
    let (mut con, mut rx) = r.expect("Pubusb Connection");

    let _ = con.psubscribe("*").await;

    // Subscribe to all channels for testing
    while let Some(message) = rx.recv().await {
        println!("[REDIS] type {:?}", message);
        match message.kind {
            redis::PushKind::PMessage => {
                let payload: Vec<u8> = extract_binary_payload_from_pmessage(message.data).unwrap();
                if let Ok(deserialized) = bincode::deserialize::<TalkProtocol>(&payload) {
                    println!("[REDIS] Received  {:?}", deserialized);
                    let _ = tx.send(Message::Binary(deserialized.serialize().unwrap().into()));
                } else {
                    eprintln!("Failed to deserialize message from Redis");
                }
            }
            _ => println!("other"),
        }
    }
}

pub async fn subscribe_to_redis(
    tx: TUnboundedSender<Message>,
    mut room_id_receiver: TUnboundedReceiver<i32>,
) {
    println!("[SERVER-SUB] Subbing to redis");

    // create one persistent redis connection for all rooms
    let r = create_redis_async_pubsub_connection().await;
    let (mut con, mut rx) = r.expect("Pubsub Connection");

    // spawn background task to receive all messages
    let tx_clone = tx.clone();
    tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            println!("[REDIS] type {:?}", message);

            match message.kind {
                redis::PushKind::SMessage => {
                    if let Some(payload) = extract_binary_payload_from_message(message.data) {
                        if let Ok(deserialized) = bincode::deserialize::<TalkProtocol>(&payload) {
                            println!("[REDIS] Received {:?}", deserialized);
                            let _ = tx_clone
                                .send(Message::Binary(deserialized.serialize().unwrap().into()));
                        } else {
                            eprintln!("Failed to deserialize message from Redis");
                        }
                    }
                }
                _ => println!("other"),
            }
        }
    });

    // track currently active room
    let mut current_room: Option<String> = None;

    // listen on channel for room changes
    while let Some(room_id) = room_id_receiver.recv().await {
        let channel = format!("{}", room_id);

        // unsubscribe from old room if there was one
        if let Some(old) = &current_room {
            println!("[SERVER-SUB] Unsubscribing from {}", old);
            let _ = con.sunsubscribe(old).await;
        }

        // subscribe to new room
        println!("[SERVER-SUB] Subscribing to {}", channel);
        con.ssubscribe(&channel).await.expect("SSUBSCRIBE failed");

        current_room = Some(channel);
    }
}
