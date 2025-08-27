use diesel::PgConnection;
use futures_channel::mpsc::{unbounded,  UnboundedSender};
use futures_util::{SinkExt, StreamExt, future, pin_mut, stream::TryStreamExt};
use redis::cluster_async::ClusterConnection as ClusterConnectionAsync;
use redis::{
    Commands, Connection, PushInfo,
    cluster::{ClusterClient, ClusterClientBuilder, ClusterConnection},
};
use redis::{from_owned_redis_value, from_redis_value, ToRedisArgs, Value};
use shared::{
    ClientAction::{Join, Leave, Send},
    TalkProtocol,
};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender as TUnboundedSender};
use std::{env, net::SocketAddr, sync::Arc};
use tokio::sync::{Mutex as TMutex, mpsc::UnboundedReceiver as TUnboundedReceiver};
use tokio::{
    net::{TcpListener, TcpStream},
    runtime::Handle,
};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::protocol::Message;

use crate::database::connection::establish_connection;

type SharedRedis = Arc<TMutex<ClusterConnection>>;
type SharedPostgres = Arc<TMutex<PgConnection>>;

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
                    let _ = tx
                        .send(Message::Binary(deserialized.serialize().unwrap().into()))
                        ;
                } else {
                    eprintln!("Failed to deserialize message from Redis");
                }
            }
            _ => println!("other"),
        }
    }
}


pub async fn xsubscribe_to_redis(tx: TUnboundedSender<Message>) {
    let r = create_redis_async_pubsub_connection().await;
    let (mut con, mut rx) = r.expect("Pubsub Connection");

    con.ssubscribe(0).await.expect("Subscribe to room");

    println!("[SERVER-SUB] before receiving");
    while let Some(message) = rx.recv().await {
        println!("[REDIS] type {:?}", message);
        match message.kind {
            redis::PushKind::SMessage => {
                let payload: Vec<u8> = extract_binary_payload_from_pmessage(message.data).unwrap();
                if let Ok(deserialized) = bincode::deserialize::<TalkProtocol>(&payload) {
                    println!("[REDIS] Received  {:?}", deserialized);
                    let _ = tx.send(Message::Binary(deserialized.serialize().unwrap().into()))
                    ;
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

    // track currently active room
    let mut current_room: Option<String> = None;

    // spawn background task to receive all messages
    let tx_clone = tx.clone();
    tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            println!("[REDIS] type {:?}", message);

            match message.kind {
                redis::PushKind::SMessage => {
                    if let Some(payload) = extract_binary_payload_from_pmessage(message.data) {
                        if let Ok(deserialized) = bincode::deserialize::<TalkProtocol>(&payload) {
                            println!("[REDIS] Received {:?}", deserialized);
                            let _ = tx_clone.send(
                                Message::Binary(deserialized.serialize().unwrap().into()),
                            );
                        } else {
                            eprintln!("Failed to deserialize message from Redis");
                        }
                    }
                }
                _ => println!("other"),
            }
        }
    });

    // listen for room changes
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

pub async fn handle_connection(raw_stream: TcpStream, addr: SocketAddr, shared_redis: SharedRedis, pg_conn: SharedPostgres) {
    println!("Incoming TCP connection from: {}", addr);

    let ws_stream = tokio_tungstenite::accept_async(raw_stream)
        .await
        .expect("Error during the websocket handshake occurred");
    println!("WebSocket connection established: {}", addr);

    let (tx, mut rx) = unbounded_channel();
    let (room_tx, room_rx) = tokio::sync::mpsc::unbounded_channel();

    let (outgoing, incoming) = ws_stream.split();

    // tokio::spawn(async move {
    //     subscribe_to_redis_pattern(tx).await;
    // });

    tokio::spawn(async move {
        subscribe_to_redis(tx,room_rx).await;
    });

    let process_msg = incoming.try_for_each(|msg| {
        let deserialize_msg: TalkProtocol = match bincode::deserialize(&msg.clone().into_data()) {
            Ok(msg) => msg,
            Err(e) => {
                let raw_data = msg.clone().into_data();

                eprintln!(
                    "[SERVER] Failed to deserialize message from {}.\n\
                        Error: {}\n\
                        Error type: {}\n\
                        Hex dump: {:02x?}",
                    addr,
                    e,
                    raw_data.len(),
                    &raw_data
                );

                if let Ok(s) = String::from_utf8(raw_data.to_vec()) {
                    eprintln!("Data as string: {:?}", s);
                }

                return future::ok(());
            }
        };

        println!(
            "[SERVER] Received a message from ip={}: [{}, {:?}, room={:?}]: {} ",
            addr,
            deserialize_msg.username,
            deserialize_msg.action,
            deserialize_msg.room_id,
            deserialize_msg.clone().message.expect("Message")
        );

        let action = &deserialize_msg.action;

        // Publish to Redis
        let sr_clone = Arc::clone(&shared_redis);
        let msg_clone = deserialize_msg.clone();
        let msg_json = msg_clone.serialize().unwrap();

        println!("[SERVER] In Send trying to publish");
        tokio::spawn(async move {
            let mut conn = sr_clone.lock().await;
            let result: Result<(), redis::RedisError> = conn
                .spublish(msg_clone.room_id, msg_json);
            match result {
                Ok(_) => println!("[SERVER] Successfully published to Redis"),
                Err(e) => eprintln!("[SERVER] Failed to publish to Redis: {}", e),
            }
        });

        if *action == Join {
            println!("[SERVER] joining {}",deserialize_msg.room_id);
                let _ = room_tx.send(deserialize_msg.room_id);
        } else if *action == Leave {
            // remove client from room
        } else {
            //invalid action
        }

        future::ok(())
    });

    let receive_from_redis = async move {
        let mut outgoing = outgoing;
        while let Some(msg) = rx.recv().await {
            if let Err(e) = outgoing.send(msg).await {
                eprintln!("Failed to send message to WebSocket: {}", e);
                break;
            }
        }
    };

    let (res1, _res2) = future::join(process_msg, receive_from_redis).await;
    if let Err(e) = res1 {
        eprintln!("process_msg failed: {:?}", e);
    }

    // tokio::select! {
    //     _ = process_msg => {
    //         println!("Message processing completed");
    //     },
    //     _ = receive_from_redis => {
    //         println!("Redis message forwarding completed");
    //     }
    // }

    println!("{} disconnected", &addr);
}

pub async fn start_ws_server() -> Result<(), std::io::Error> {
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:8080".to_string());

    let try_socket = TcpListener::bind(&addr).await;
    let listener = try_socket.expect("Failed to bind");

    println!("Listening on: {}", addr);

    let redis_con = create_redis_connection()
        .await
        .expect("Redis connection failed");
    let shared_con: SharedRedis = Arc::new(TMutex::new(redis_con));

    let pg_conn = Arc::new(TMutex::new(establish_connection()));

    while let Ok((stream, addr)) = listener.accept().await {
        let rd_clone = Arc::clone(&shared_con);
        let pg_clone = Arc::clone(&pg_conn);
        tokio::spawn(handle_connection(stream, addr, rd_clone, pg_clone));

        let metrics = Handle::current().metrics();

        let n = metrics.num_alive_tasks();
        println!("Server has {} active connections", n);
    }

    Ok(())
}
