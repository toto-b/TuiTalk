use crate::database::{
    connection::establish_connection,
    models::{NewMessage, NewUser},
    queries::*,
};
use crate::redis::*;
use anyhow::Result;
use diesel::PgConnection;
use futures_util::{SinkExt, StreamExt, stream::TryStreamExt};
use redis::Commands;
use shared::{TalkMessage, TalkProtocol};
use std::{env, net::SocketAddr, sync::Arc};
use tokio::sync::Mutex as TMutex;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::{mpsc::unbounded_channel, oneshot};
use tokio::{
    net::{TcpListener, TcpStream},
    runtime::Handle,
};
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

type SharedPostgres = Arc<TMutex<PgConnection>>;

pub async fn handle_connection(
    raw_stream: TcpStream,
    addr: SocketAddr,
    shared_redis: SharedRedis,
    pg_conn: SharedPostgres,
) -> Result<()> {
    println!("Incoming TCP connection from: {}", addr);

    let ws_stream = tokio_tungstenite::accept_async(raw_stream).await?;
    println!("WebSocket connection established: {}", addr);

    let (tx, mut rx) = unbounded_channel();
    let (room_tx, room_rx) = unbounded_channel::<(i32, oneshot::Sender<()>)>();

    let (mut outgoing, incoming) = ws_stream.split();

    // Spawn Redis subscriber
    tokio::spawn(subscribe_to_redis(tx.clone(), room_rx));

    // Process incoming messages
    let message_handler = async {
        incoming
            .try_for_each(|msg| async {
                let deserialize_msg: TalkProtocol =
                    bincode::deserialize(&msg.into_data()).expect("deserializing");
                let _ = handle_message(
                    deserialize_msg,
                    &room_tx,
                    tx.clone(),
                    &shared_redis,
                    &pg_conn,
                )
                .await;
                Ok(())
            })
            .await
    };

    // Forward Redis messages to WebSocket
    let redis_forwarder = async {
        while let Some(msg) = rx.recv().await {
            outgoing.send(msg).await?;
        }
        Ok(())
    };

    // Run both tasks concurrently
    tokio::select! {
        result = message_handler => result,
        result = redis_forwarder => result,
    }?;

    println!("{} disconnected", addr);
    Ok(())
}

async fn handle_message(
    msg: TalkProtocol,
    room_tx: &UnboundedSender<(i32, oneshot::Sender<()>)>,
    tx: UnboundedSender<Message>,
    shared_redis: &SharedRedis,
    pg_conn: &SharedPostgres,
) -> Result<()> {
    println!("[SERVER] Received {:?}", msg);
    match &msg {
        TalkProtocol::JoinRoom {
            room_id,
            uuid,
            username,
            unixtime,
        } => {
            handle_join(room_id, room_tx).await?;

            let response = TalkProtocol::UserJoined {
                uuid: *uuid,
                username: username.clone(),
                room_id: *room_id,
                unixtime: *unixtime,
            };
            publish_message(shared_redis, &response, room_id).await?;
            persist_user(pg_conn, room_id, uuid).await?;

            let message_to_persist = TalkMessage {
                uuid: *uuid,
                username: username.clone(),
                text: "".to_string(),
                room_id: *room_id,
                unixtime: *unixtime,
            };
            persist_message(
                pg_conn,
                &message_to_persist,
                response.to_i16().expect("TalkProtocol type conversion"),
            )
            .await?;
        }
        TalkProtocol::LeaveRoom {
            room_id,
            uuid,
            unixtime,
            username,
        } => {
            let response = TalkProtocol::UserLeft {
                uuid: *uuid,
                username: username.clone(),
                room_id: *room_id,
                unixtime: *unixtime,
            };
            publish_message(shared_redis, &response, room_id).await?;
            delete_user(pg_conn, uuid).await?;

            let message_to_persist = TalkMessage {
                uuid: *uuid,
                username: username.clone(),
                text: "".to_string(),
                room_id: *room_id,
                unixtime: *unixtime,
            };
            persist_message(
                pg_conn,
                &message_to_persist,
                response.to_i16().expect("TalkProtocol type conversion"),
            )
            .await?;
        }
        TalkProtocol::PostMessage { message } => {
            publish_message(shared_redis, &msg, &message.room_id).await?;
            persist_message(
                pg_conn,
                message,
                msg.to_i16().expect("TalkProtocol type conversion"),
            )
            .await?;
        }
        TalkProtocol::Fetch {
            room_id,
            limit,
            fetch_before,
        } => {
            let messages = handle_fetch(room_id, limit, fetch_before, pg_conn).await?;
            let response = TalkProtocol::History { text: messages };
            let _ = tx.send(Message::Binary(response.serialize().unwrap().into()));
        }
        TalkProtocol::ChangeName {
            uuid,
            username,
            unixtime,
            old_username,
        } => {
            let response = TalkProtocol::UsernameChanged {
                uuid: *uuid,
                username: username.clone(),
                old_username: old_username.clone(),
                unixtime: *unixtime,
            };

            let room_id = {
                let mut conn = pg_conn.lock().await;
                get_room_id_by_uuid(&mut conn, *uuid)
                    .ok()
                    .and_then(|results| results.first().map(|r| r.room_id))
            };

            if let Some(room_id) = room_id {
                publish_message(shared_redis, &response, &room_id).await?;
            }
        }
        // Server -> Client events typically don't need handling here
        TalkProtocol::UserJoined { .. }
        | TalkProtocol::UserLeft { .. }
        | TalkProtocol::History { .. }
        | TalkProtocol::UsernameChanged { .. }
        | TalkProtocol::LocalError { .. }
        | TalkProtocol::Error { .. } => {
            // These are usually sent from server to client, not received
            eprintln!("Unexpected server-to-client message received");
        }
    }
    Ok(())
}

async fn handle_join(
    room_id: &i32,
    room_tx: &UnboundedSender<(i32, oneshot::Sender<()>)>,
) -> Result<()> {
    let (ack_tx, ack_rx) = oneshot::channel();
    room_tx.send((*room_id, ack_tx))?;
    ack_rx.await?;
    Ok(())
}

async fn handle_fetch(
    room_id: &i32,
    limit: &i64,
    fetch_before: &u64,
    pg_conn: &SharedPostgres,
) -> Result<Vec<TalkProtocol>> {
    let mut conn = pg_conn.lock().await;
    let history = get_history(&mut conn, room_id, limit, fetch_before)?;
    let message_list: Vec<TalkProtocol> = history
        .iter()
        .map(|e| {
            TalkProtocol::from_i16(
                e.protocol_type,
                e.room_id,
                e.uuid,
                e.username.clone(),
                e.time as u64,
                e.message.clone(),
            )
            .expect("Type conversion to Talkprotocol from DB")
        })
        .collect();
    Ok(message_list)
}

// Helper functions for DB operations
async fn persist_message(
    pg_conn: &SharedPostgres,
    msg: &TalkMessage,
    protocol_type_message: i16,
) -> Result<()> {
    let mut conn = pg_conn.lock().await;
    insert_message(
        &mut conn,
        NewMessage {
            room_id: msg.room_id,
            message: msg.text.clone(),
            time: msg.unixtime as i64,
            uuid: msg.uuid,
            username: msg.username.clone(),
            protocol_type: protocol_type_message,
        },
    )?;
    Ok(())
}

async fn persist_user(pg_conn: &SharedPostgres, room_id: &i32, uuid: &Uuid) -> Result<()> {
    let mut conn = pg_conn.lock().await;
    insert_user(
        &mut conn,
        NewUser {
            uuid: *uuid,
            room_id: *room_id,
        },
    )?;
    Ok(())
}

async fn delete_user(pg_conn: &SharedPostgres, user_uuid: &Uuid) -> Result<()> {
    let mut conn = pg_conn.lock().await;
    delete_user_by_uuid(&mut conn, *user_uuid)?;
    Ok(())
}

async fn publish_message(
    shared_redis: &SharedRedis,
    msg: &TalkProtocol,
    room_id: &i32,
) -> Result<()> {
    let mut conn = shared_redis.lock().await;
    let msg_json = msg.serialize()?;
    let _: () = conn.spublish(room_id, msg_json)?;
    Ok(())
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
