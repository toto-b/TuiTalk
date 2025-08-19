use futures_channel::mpsc::{UnboundedSender, unbounded};
use futures_util::{SinkExt, StreamExt, future, pin_mut, stream::TryStreamExt};
use redis::{aio::PubSub, Commands, Connection};
use shared::{
    ClientAction::{Join, Leave, Send},
    TalkProtocol,
};
use std::{
    env,
    net::SocketAddr,
    sync::{Arc},
};
use tokio::sync::Mutex as TMutex;
use tokio::{
    net::{TcpListener, TcpStream},
    runtime::Handle,
};
use tokio_tungstenite::tungstenite::protocol::Message;

type SharedRedis = Arc<TMutex<Connection>>;

pub async fn create_redis_async_connection() -> Result<PubSub, redis::RedisError> {
    let client = redis::Client::open("redis://0.0.0.0/")?;
    let publish_conn = client.get_async_pubsub().await.expect("Async Connection");
    Ok(publish_conn)
}

pub async fn create_redis_connection() -> Result<Connection, redis::RedisError> {
    let client = redis::Client::open("redis://0.0.0.0/")?;
    let publish_conn = client.get_connection().expect("Async Connection");
    Ok(publish_conn)
}

pub async fn subscribe_to_redis(mut tx: UnboundedSender<Message>) {
    let r = create_redis_async_connection().await;
    let mut pubsub = r.expect("Pubusb Connection");

    // Subscribe to all channels for testing
    let _ = pubsub.psubscribe("*").await;

    loop {
        let redis_msg = pubsub.on_message().next().await;
        if let Some(message) = redis_msg {
            let channel = message.get_channel_name();
            let payload: Vec<u8> = message.get_payload().expect("Binary payload");

            if let Ok(deserialized) = bincode::deserialize::<TalkProtocol>(&payload) {
                println!("[REDIS] Received on channel={}: {:?}", channel, deserialized);
                let _ = tx
                    .send(Message::Binary(deserialized.serialize().unwrap().into()))
                    .await;
            } else {
                eprintln!("Failed to deserialize message from Redis");
            }
        }
    }
}

pub async fn handle_connection(
    raw_stream: TcpStream,
    addr: SocketAddr,
    shared_redis: SharedRedis,
) {
    println!("Incoming TCP connection from: {}", addr);

    let ws_stream = tokio_tungstenite::accept_async(raw_stream)
        .await
        .expect("Error during the websocket handshake occurred");
    println!("WebSocket connection established: {}", addr);

    let (tx, rx) = unbounded();

    let (outgoing, incoming) = ws_stream.split();

    tokio::spawn(async move {
        subscribe_to_redis(tx).await;
    });

    let process_msg = incoming.try_for_each(|msg| {
        let deserialize_msg: TalkProtocol = match bincode::deserialize(&msg.clone().into_data()) {
            Ok(msg) => msg,
            Err(e) => {
                let raw_data = msg.clone().into_data();

                eprintln!(
                    "Failed to deserialize message from {}.\n\
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
            "Received a message from ip={}: [{}, {:?}]: {} ",
            addr,
            deserialize_msg.username,
            deserialize_msg.action,
            deserialize_msg.clone().message.expect("Message")
        );

        let action = &deserialize_msg.action;

        if *action == Send {
            println!("Received send action");

            // Publish to Redis
            let sr_clone = Arc::clone(&shared_redis);
            let msg_clone = deserialize_msg.clone();
            let msg_json = msg_clone.serialize().unwrap();

            tokio::spawn(async move {
                let mut conn = sr_clone.lock().await;
                let _: () = conn
                    .publish(msg_clone.room_id, msg_json)
                    .expect("Publish msg");
            });
        } else if *action == Join {
            //add room to client rooms
        } else if *action == Leave {
            // remove client from room
        } else {
            //invalid action
        }

        future::ok(())
    });

    let receive_from_redis = rx.map(Ok).forward(outgoing);

    pin_mut!(process_msg);
    future::select(process_msg, receive_from_redis).await;

    println!("{} disconnected", &addr);
}

pub async fn start_ws_server() -> Result<(), std::io::Error> {
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:8080".to_string());

    let try_socket = TcpListener::bind(&addr).await;
    let listener = try_socket.expect("Failed to bind");

    println!("Listening on: {}", addr);

    let redis_con = create_redis_connection().await.expect("Redis connection failed");
    let shared_con: SharedRedis = Arc::new(TMutex::new(redis_con));

    while let Ok((stream, addr)) = listener.accept().await {
        let rd_clone = Arc::clone(&shared_con);
        tokio::spawn(handle_connection(stream, addr, rd_clone));

        let metrics = Handle::current().metrics();

        let n = metrics.num_alive_tasks();
        println!("Server has {} active connections", n);
    }

    Ok(())
}
