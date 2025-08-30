use crate::database::{
    connection::establish_connection,
    models::{NewMessage, NewUser},
    queries::{delete_user_by_uuid, insert_message, insert_user},
};
use crate::redis::*;
use diesel::PgConnection;
use futures_util::{SinkExt, StreamExt, future, stream::TryStreamExt};
use redis::Commands;
use shared::{
    ClientAction::{Fetch, Join, Leave, Send},
    TalkProtocol,
};
use std::{env, net::SocketAddr, sync::Arc};
use tokio::sync::Mutex as TMutex;
use tokio::sync::{mpsc::unbounded_channel, oneshot};
use tokio::{
    net::{TcpListener, TcpStream},
    runtime::Handle,
};

type SharedPostgres = Arc<TMutex<PgConnection>>;

pub async fn handle_connection(
    raw_stream: TcpStream,
    addr: SocketAddr,
    shared_redis: SharedRedis,
    pg_conn: SharedPostgres,
) {
    println!("Incoming TCP connection from: {}", addr);

    let ws_stream = tokio_tungstenite::accept_async(raw_stream)
        .await
        .expect("Error during the websocket handshake occurred");
    println!("WebSocket connection established: {}", addr);

    let (tx, mut rx) = unbounded_channel();
    let (room_tx, room_rx) = unbounded_channel::<(i32, oneshot::Sender<()>)>();

    let (outgoing, incoming) = ws_stream.split();

    tokio::spawn(async move {
        subscribe_to_redis(tx, room_rx).await;
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

        let msg_clone = deserialize_msg.clone();
        let msg_json = msg_clone.serialize().unwrap();

        let con = Arc::clone(&pg_conn);
        tokio::spawn(async move {
            let mut unlock = con.lock().await;
            if let Ok(query_result) = insert_message(
                &mut unlock,
                NewMessage {
                    room_id: msg_clone.room_id,
                    message: msg_clone.message.expect("persisting message"),
                    time: msg_clone.unixtime as i64,
                    uuid: msg_clone.uuid,
                    username: msg_clone.username,
                },
            ) {
                println!(
                    "[SERVER] Query Successful Message was persisted {}",
                    query_result
                );
            }
        });

        let action = &deserialize_msg.action;
        if *action == Join {
            println!("[SERVER] sending room change {}", deserialize_msg.room_id);

            let (ack_tx, ack_rx) = oneshot::channel();
            let _ = room_tx.send((deserialize_msg.room_id, ack_tx));
            let sr_clone = Arc::clone(&shared_redis);
            // Wait till Channels was switche and publish afterwards to redis
            tokio::spawn(async move {
                match ack_rx.await {
                    Ok(()) => {
                        // Publish to Redis
                        let mut conn = sr_clone.lock().await;
                        let result: Result<(), redis::RedisError> =
                            conn.spublish(msg_clone.room_id, msg_json);
                        match result {
                            Ok(_) => println!("[SERVER] Successfully published to Redis"),
                            Err(e) => eprintln!("[SERVER] Failed to publish to Redis: {}", e),
                        }
                    }
                    Err(_) => {
                        eprintln!("[SERVER] Subscription acknowledgment failed");
                    }
                };
                let _ = Ok::<(), tokio_tungstenite::tungstenite::Error>(());
            });

            // save uuid and room relationship
            let con = Arc::clone(&pg_conn);
            tokio::spawn(async move {
                let mut unlock = con.lock().await;
                if let Ok(query_result) = insert_user(
                    &mut unlock,
                    NewUser {
                        room_id: deserialize_msg.clone().room_id,
                        uuid: deserialize_msg.clone().uuid,
                    },
                ) {
                    println!(
                        "[SERVER] Query Successful User was persisted {}",
                        query_result
                    );
                }
            });
        } else if *action == Leave {
            // remove client from room
            let con = Arc::clone(&pg_conn);
            tokio::spawn(async move {
                let mut unlock = con.lock().await;
                if let Ok(query_result) = delete_user_by_uuid(&mut unlock, deserialize_msg.uuid) {
                    println!(
                        "[SERVER] Query Successful user roomstate was deleted {}",
                        query_result
                    );
                }
            });
        } else if *action == Fetch {
            println!("[SERVER] Received a fetch request");
        } else {
            // Normal messages - process immediately
            let msg_clone = deserialize_msg.clone();
            let msg_json = msg_clone.serialize().unwrap();

            let sr_clone = Arc::clone(&shared_redis);
            tokio::spawn(async move {
                let mut conn = sr_clone.lock().await;
                let result: Result<(), redis::RedisError> =
                    conn.spublish(msg_clone.room_id, msg_json);
                match result {
                    Ok(_) => println!("[SERVER] Successfully published to Redis"),
                    Err(e) => eprintln!("[SERVER] Failed to publish to Redis: {}", e),
                }
            });
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
