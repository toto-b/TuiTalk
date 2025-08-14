use std::{
    collections::HashMap,
    env,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use shared::{TalkProtocol};

use futures_channel::mpsc::{unbounded, UnboundedSender};
use futures_util::{future, pin_mut, stream::TryStreamExt, StreamExt};

use tokio::{net::{TcpListener, TcpStream}, runtime::Handle};
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio::sync::Mutex as TMutex;
use redis::{Commands, Connection};
type SharedRedis = Arc<TMutex<Connection>>;

type Tx = UnboundedSender<Message>;
type PeerMap = Arc<Mutex<HashMap<SocketAddr, Tx>>>;


pub async fn handle_connection(peer_map: PeerMap, raw_stream: TcpStream, addr: SocketAddr, shared_redis: SharedRedis) {
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
        println!("Received a message from ip={}: [{}]: {}", addr, deserialize_msg.username, deserialize_msg.message);
        let peers = peer_map.lock().unwrap();

        let broadcast_recipients = peers.iter().filter(|(peer_addr, _)| peer_addr != &&addr).map(|(_, ws_sink)| ws_sink);

        for recp in broadcast_recipients {
            recp.unbounded_send(msg.clone()).unwrap(); // tx.send -> sending to all other future channels which are held by the other clients
        }  

        // Publish to Redis
        // Use this pattern for high-volume publishing
        let sr_clone = Arc::clone(&shared_redis);
        let msg_clone = deserialize_msg.clone();
        tokio::spawn(async move {
            let mut conn = sr_clone.lock().await;
            let _ : () = conn.publish("peter", msg_clone.message).unwrap();
        });

        future::ok(())
    });

    let receive_from_others = rx.map(Ok).forward(outgoing);

    pin_mut!(broadcast_incoming, receive_from_others);
    future::select(broadcast_incoming, receive_from_others).await;

    println!("{} disconnected", &addr);
    peer_map.lock().unwrap().remove(&addr);
}




pub async fn start_ws_server(shared_redis: SharedRedis) ->  Result<(), std::io::Error>  {

    let addr = env::args().nth(1).unwrap_or_else(|| "0.0.0.0:8080".to_string());
    let state = PeerMap::new(Mutex::new(HashMap::new()));

    let try_socket = TcpListener::bind(&addr).await;
    let listener = try_socket.expect("Failed to bind");

    println!("Listening on: {}", addr);

    while let Ok((stream, addr)) = listener.accept().await {
        let rd_clone = Arc::clone(&shared_redis);
        tokio::spawn(handle_connection(state.clone(), stream, addr, rd_clone));
        let metrics = Handle::current().metrics();

        let n = metrics.num_alive_tasks();
        println!("Server has {} active connections", n);
    }

    Ok(())
}
