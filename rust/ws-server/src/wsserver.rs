use std::{
    collections::HashMap, env, net::SocketAddr, sync::{Arc, Mutex}
};
use shared::{TalkProtocol, ClientAction::{Join,Leave,Send}};
use futures_channel::mpsc::{UnboundedSender, unbounded};
use futures_util::{StreamExt, future, pin_mut, stream::TryStreamExt};
use redis::{Commands, Connection, PubSubCommands, RedisResult, ToRedisArgs};
use tokio::sync::Mutex as TMutex;
use tokio::{
    net::{TcpListener, TcpStream},
    runtime::Handle,
};
use tokio_tungstenite::tungstenite::protocol::Message;

type SharedRedis = Arc<TMutex<Connection>>;
type Tx = UnboundedSender<Message>;
type PeerMap = Arc<Mutex<HashMap<SocketAddr, Tx>>>;

pub fn create_redis_connections() ->  Result<Connection, redis::RedisError> {
    let client = redis::Client::open("redis://0.0.0.0/")?;
    let publish_conn = client.get_connection()?;
    Ok(publish_conn)
}

pub async fn subscribe_to_room(room_id : i32) {
    let mut conn = create_redis_connections().expect("Redis connection pub/sub");
    let mut pubsub_con = conn.as_pubsub();
    pubsub_con.psubscribe("*").expect("Subscribe to channel");

    while let Ok(message) =pubsub_con.get_message() {
        let channel_name = message.get_channel_name(); 
        let payload : Vec<u8> = message.get_payload().expect("Binary payload");
        let deserialize_msg : TalkProtocol= bincode::deserialize(&payload).expect("Payload deserializing failed");
        println!("[REDIS] {} {:?}", channel_name, deserialize_msg);
    }
}

pub async fn handle_connection(
    peer_map: PeerMap,
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
    peer_map.lock().unwrap().insert(addr, tx);

    let (outgoing, incoming) = ws_stream.split();

    let broadcast_incoming = incoming.try_for_each(|msg| {
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
            addr, deserialize_msg.username,  deserialize_msg.action, deserialize_msg.clone().message.expect("Message")
        );

        let action = &deserialize_msg.action;

        if *action == Send {
            println!("Received send action");
            let peers = peer_map.lock().unwrap();

            // prevent echoing message
            let broadcast_recipients = peers
                .iter()
                .filter(|(peer_addr, _)| peer_addr != &&addr)
                .map(|(_, ws_sink)| ws_sink);

            // send to all on same backend
            for recp in broadcast_recipients {
                recp.unbounded_send(msg.clone()).unwrap(); // tx.send -> sending to all other future channels which are held by the other clients
            }

            // Publish to Redis
            let sr_clone = Arc::clone(&shared_redis);
            let msg_clone = deserialize_msg.clone();
            let msg_json = msg_clone.serialize().unwrap();

            tokio::spawn(async move {
                let mut conn = sr_clone.lock().await;
                let _: () = conn
                    .publish(
                        msg_clone.room_id,
                        msg_json,
                    )
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

    let receive_from_others = rx.map(Ok).forward(outgoing);

    pin_mut!(broadcast_incoming);
    future::select(broadcast_incoming, receive_from_others).await;

    println!("{} disconnected", &addr);
    peer_map.lock().unwrap().remove(&addr);
}

pub async fn start_ws_server() -> Result<(), std::io::Error> {
    let addr = env::args().nth(1).unwrap_or_else(|| "0.0.0.0:8080".to_string());

    tokio::spawn(async move {
        subscribe_to_room(0).await;
    });
    let state = PeerMap::new(Mutex::new(HashMap::new()));

    let try_socket = TcpListener::bind(&addr).await;
    let listener = try_socket.expect("Failed to bind");

    println!("Listening on: {}", addr);

    let redis_con = create_redis_connections().expect("Redis connection failed");
    let shared_con: SharedRedis = Arc::new(TMutex::new(redis_con));

    while let Ok((stream, addr)) = listener.accept().await {
        let rd_clone = Arc::clone(&shared_con);
        tokio::spawn(handle_connection(state.clone(), stream, addr, rd_clone));
        let metrics = Handle::current().metrics();

        let n = metrics.num_alive_tasks();
        println!("Server has {} active connections", n);
    }

    Ok(())
}
