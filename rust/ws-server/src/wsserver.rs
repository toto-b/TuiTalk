use crate::database::{
    connection::establish_connection,
    models::{NewMessage, NewUser},
    queries::{delete_user_by_uuid, insert_message, insert_user},
};
use crate::redis::*;
use diesel::PgConnection;
use futures_util::{SinkExt, StreamExt, future, stream::TryStreamExt};
use redis::{Commands, FromRedisValue};
use shared::{
    ClientAction::{Fetch, Join, Leave, Send},
    TalkProtocol,
};
use uuid::Uuid;
use std::{env, net::SocketAddr, sync::Arc};
use tokio::sync::Mutex as TMutex;
use tokio::sync::{mpsc::unbounded_channel, oneshot };
use tokio::sync::mpsc::UnboundedSender;
use tokio::{
    net::{TcpListener, TcpStream},
    runtime::Handle,
}; // custom module
use anyhow::{Result};

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
    tokio::spawn(subscribe_to_redis(tx, room_rx));

    // Process incoming messages
    let message_handler = async {
        incoming
            .try_for_each(|msg| async {
                let deserialize_msg: TalkProtocol = bincode::deserialize(&msg.into_data()).expect("deserializing");
                let _ = handle_message(deserialize_msg, &room_tx, &shared_redis, &pg_conn).await;
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
    shared_redis: &SharedRedis,
    pg_conn: &SharedPostgres,
) -> Result<()> {
    println!(
        "[SERVER] Received: [{}, {:?}, room={:?}]: {}",
        msg.username,
        msg.action,
        msg.room_id,
        msg.message.as_deref().unwrap_or("")
    );

    // Persist message to DB
    persist_message(pg_conn, &msg).await?;

    match msg.action {
        Join => handle_join(msg, room_tx, shared_redis, pg_conn).await,
        Leave => handle_leave(msg, shared_redis, pg_conn).await,
        // Fetch => handle_fetch(msg).await,
        _ => handle_normal_message(msg, shared_redis).await,
    }
}

async fn handle_join(
    msg: TalkProtocol,
    room_tx: &UnboundedSender<(i32, oneshot::Sender<()>)>,
    shared_redis: &SharedRedis,
    pg_conn: &SharedPostgres,
) -> Result<()> {
    println!("[SERVER] Room change to {}", msg.room_id);

    let (ack_tx, ack_rx) = oneshot::channel();
    room_tx.send((msg.room_id, ack_tx))?;

    // Wait for subscription confirmation
    ack_rx.await?;

    // Now publish safely
    publish_message(shared_redis, &msg).await?;
    persist_user(pg_conn, &msg).await?;

    Ok(())
}

async fn handle_leave(msg: TalkProtocol, shared_redis: &SharedRedis, pg_conn: &SharedPostgres) -> Result<()> {
    publish_message(shared_redis, &msg).await?;
    delete_user(pg_conn, &msg.uuid).await?;
    Ok(())
}

async fn handle_normal_message(msg: TalkProtocol, shared_redis: &SharedRedis) -> Result<()> {
    publish_message(shared_redis, &msg).await
}

// Helper functions for DB operations
async fn persist_message(pg_conn: &SharedPostgres, msg: &TalkProtocol) -> Result<()> {
    let mut conn = pg_conn.lock().await;
    insert_message(&mut conn, NewMessage {
        room_id: msg.room_id,
        message: msg.message.clone().unwrap(),
        time: msg.unixtime as i64,
        uuid: msg.uuid,
        username: msg.username.clone(),
    })?;
    Ok(())
}

async fn persist_user(pg_conn: &SharedPostgres, msg: &TalkProtocol) -> Result<()> {
    let mut conn = pg_conn.lock().await;
    insert_user(&mut conn, NewUser {
        room_id: msg.room_id,
        uuid: msg.uuid,
    })?;
    Ok(())
}

async fn delete_user(pg_conn: &SharedPostgres, user_uuid: &Uuid) -> Result<()> {
    let mut conn = pg_conn.lock().await;
    delete_user_by_uuid(&mut conn, *user_uuid)?;
    Ok(())
}

async fn publish_message(shared_redis: &SharedRedis, msg: &TalkProtocol) -> Result<()> {
    let mut conn = shared_redis.lock().await;
    let msg_json = msg.serialize()?;
    let _: () = conn.spublish(msg.room_id, msg_json)?;
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
