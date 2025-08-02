use serde::{Deserialize, Serialize};
use std::time::SystemTime;

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
    pub unixtime: SystemTime,
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
                    if let Err(e) = write.send(Message::binary(bin)).await {
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

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;
    use wasm_bindgen::prelude::*;
    use web_sys::{WebSocket, BinaryType};
    use js_sys::{Uint8Array, ArrayBuffer};
    use std::time::{UNIX_EPOCH, Duration};

    #[wasm_bindgen]
    pub struct WsClient {
        ws: WebSocket,
        _closures: Vec<Closure<dyn FnMut()>>,
    }

    #[wasm_bindgen]
    #[derive(Clone)]
    pub enum WasmClientAction {
        Join,
        Leave,
        CreateRoom,
    }

    #[wasm_bindgen]
    #[derive(Clone)]
    pub struct WasmTalkProtocol {
        username: String,
        message: String,
        room_id: i32,
        unixtime: f64,
        #[wasm_bindgen(skip)]
        action: Option<WasmClientAction>,
    }

    #[wasm_bindgen]
    impl WsClient {
        #[wasm_bindgen(constructor)]
        pub fn new(url: &str) -> Result<WsClient, JsValue> {
            let ws = WebSocket::new(url)?;
            ws.set_binary_type(BinaryType::Arraybuffer); // Fixed BinaryType usage
            
            let mut closures = vec![];
            
            let on_open = Closure::new(|| {
                web_sys::console::log_1(&"Connected!".into());
            });
            ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
            closures.push(on_open);

            Ok(WsClient { ws, _closures: closures })
        }

        #[wasm_bindgen(js_name = sendProtocol)]
        pub fn send_protocol(&self, protocol: &WasmTalkProtocol) -> Result<(), JsValue> {
            let native_protocol = TalkProtocol {
                username: protocol.username.clone(),
                message: protocol.message.clone(),
                action: protocol.action.as_ref().map(|a| match a {
                    WasmClientAction::Join => ClientAction::Join,
                    WasmClientAction::Leave => ClientAction::Leave,
                    WasmClientAction::CreateRoom => ClientAction::CreateRoom,
                }),
                room_id: protocol.room_id,
                unixtime: UNIX_EPOCH + Duration::from_secs_f64(protocol.unixtime),
            };

            let bytes = bincode::serialize(&native_protocol)
                .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))?;

            // Correct way to create Uint8Array from bytes
            let array = Uint8Array::new_with_length(bytes.len() as u32);
            array.copy_from(&bytes);
            
            self.ws.send_with_u8_array(&array.to_vec())
        }
    }
}
#[cfg(target_arch = "wasm32")]
pub use wasm::*; // Expose WASM API

#[cfg(not(target_arch = "wasm32"))]
pub use native::*; // Expose native API
