use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ClientAction {
    Join,
    Leave,
    Send,
    Fetch,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TalkProtocol {
    pub uuid: Uuid,
    pub username: String,
    pub message: Option<String>,
    pub action: ClientAction,
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
    use super::TalkProtocol;
    use futures_channel::mpsc::UnboundedReceiver;
    use futures_util::SinkExt;
    use futures_util::StreamExt;
    // use futures_util::lock::Mutex;
    use futures_util::stream::{SplitSink, SplitStream};
    use gloo_net::websocket::Message;
    use gloo_net::websocket::futures::WebSocket;
    use gloo_utils::errors::JsError;
    use log::Level;
    use log::info;
    use yew::prelude::*;

    pub fn connect_websocket(url: &str) -> Result<WebSocket, JsError> {
        WebSocket::open(url)
    }

    pub async fn sender_task(
        mut rx: UnboundedReceiver<TalkProtocol>,
        mut write: SplitSink<WebSocket, Message>,
    ) {
        while let Some(msg) = rx.next().await {
            match bincode::serialize(&msg) {
                Ok(bin) => {
                    if let Err(_e) = write.send(Message::Bytes(bin)).await {
                        break;
                    }
                }
                Err(e) => panic!("Sending message failed {}", e),
            }
        }

        println!("Sender task ended");
    }

    pub async fn receiver_task(
        mut read: SplitStream<WebSocket>,
        messages: UseStateHandle<Vec<TalkProtocol>>,
    ) {
        let messages = messages.clone();
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Bytes(bin)) => {
                    if let Ok(parsed) = TalkProtocol::deserialize(&bin) {
                        let mut current = (*messages).clone();
                        current.push(parsed.clone());
                        messages.set(current);

                        let _ = console_log::init_with_level(Level::Debug);
                        if let Some(parsed_msg) = parsed.message {
                            info!("Received bytes message: {}", parsed_msg);
                        }
                    }
                }
                Ok(Message::Text(text)) => {
                    // Optional: Handle text messages if you expect them
                    println!("Received text message: {}", text);
                }
                Err(_e) => (),
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm::*; // Expose WASM API

#[cfg(not(target_arch = "wasm32"))]
pub use native::*; // Expose native API
