use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientAction {
    Join,
    Leave,
    CreateRoom,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TalkProtocol {
    pub username: String,
    pub message: String,
    pub action: Option<ClientAction>,
    pub room_id: i32,
    pub unixtime: u64,
}

impl TalkProtocol {
    pub fn serialize(&self) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(self)
    }

    pub fn deserialize(bytes: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(bytes)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub mod native {
    use super::*;
    use futures_channel::mpsc::UnboundedReceiver;
    use futures_util::{
        SinkExt, StreamExt,
        stream::{SplitSink, SplitStream},
    };
    use tokio::net::TcpStream;
    use tokio_tungstenite::tungstenite::Error as WsError;
    use tokio_tungstenite::{
        MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Error,
        tungstenite::protocol::Message,
    };
    type WebStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

    pub async fn connect(
        url: String,
    ) -> Result<(SplitSink<WebStream, Message>, SplitStream<WebStream>), Error> {
        let stream = connect_async(url).await?.0;
        Ok(stream.split())
    }

    pub async fn sender_task(
        mut rx: UnboundedReceiver<TalkProtocol>,
        mut write: SplitSink<WebStream, Message>,
    ) {
        while let Some(msg) = rx.next().await {
            match bincode::serialize(&msg) {
                Ok(bin) => {
                    if let Err(e) = write.send(Message::Binary(bin)).await {
                        eprintln!("WebSocket send error: {:?}", e);
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("Serialization error: {:?}", e);
                }
            }
        }

        println!("Sender task ended");
    }

    pub async fn receiver_task(
        mut read: SplitStream<WebStream>,
        mut on_message: impl FnMut(TalkProtocol) + Send + 'static,
    ) -> Result<(), WsError> {
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Binary(bin)) => {
                    if let Ok(parsed) = TalkProtocol::deserialize(&bin) {
                        on_message(parsed);
                    }
                }
                Ok(Message::Text(text)) => {
                    // Optional: Handle text messages if you expect them
                    println!("Received text message: {}", text);
                }
                Ok(_) => {} // Ignore other message types
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

//----------------------------------------------------------------------------------------------------WASM----------------------------------------------------------------------------------------------------

pub mod wasm {
    use super::*;
    use gloo_net::websocket::{futures::WebSocket, Message as WsMessage};
    use wasm_bindgen_futures::spawn_local;
    use futures_util::{SinkExt, StreamExt};
    use thiserror::Error;

    #[derive(Error, Debug)]
    pub enum WsError {
        #[error("WebSocket connection failed: {0}")]
        ConnectionFailed(String),
        #[error("Send failed: {0}")]
        SendFailed(String),
    }

    pub struct WsConnection {
        ws_sender: futures_channel::mpsc::UnboundedSender<TalkProtocol>,
    }

    impl WsConnection {
        pub async fn connect(
            url: &str,
            mut on_message: impl FnMut(TalkProtocol) + 'static,
        ) -> Result<Self, WsError> {
            let ws = WebSocket::open(url)
                .map_err(|e| WsError::ConnectionFailed(e.to_string()))?;

            let (mut write, mut read) = ws.split();
            let (tx, mut rx) = futures_channel::mpsc::unbounded();

            spawn_local(async move {
                while let Some(msg) = rx.next().await {
                    match bincode::serialize(&msg) {
                        Ok(bin) => if let Err(e) = write.send(WsMessage::Bytes(bin)).await {
                            log::error!("Send error: {:?}", e);
                            break;
                        },
                        Err(e) => log::error!("Serialization error: {:?}", e),
                    }
                }
            });

            spawn_local(async move {
                while let Some(msg) = read.next().await {
                    if let Ok(WsMessage::Bytes(bin)) = msg {
                        if let Ok(parsed) = TalkProtocol::deserialize(&bin) {
                            on_message(parsed);
                        }
                    }
                }
            });

            Ok(Self { ws_sender: tx })
        }

        pub fn send(&self, msg: TalkProtocol) -> Result<(), WsError> {
            self.ws_sender
                .unbounded_send(msg)
                .map_err(|e| WsError::SendFailed(e.to_string()))?;
            Ok(())
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm::*; // Expose WASM API

#[cfg(not(target_arch = "wasm32"))]
pub use native::*; // Expose native API
