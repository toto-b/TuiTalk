use wasm_bindgen::prelude::*;
use web_sys::console;
use shared::TalkProtocol;
use shared::ClientAction;
use shared::wasm::WsConnection;

#[wasm_bindgen(start)]
pub async fn start() -> Result<(), JsValue>{
    console_error_panic_hook::set_once();

    // Initialize logger (optional)
    console_log::init_with_level(log::Level::Debug).unwrap();

    // Connect to WebSocket
    let conn = WsConnection::connect(
        "ws://localhost:8080",
        |msg| {
            console::log_1(&format!("Received: {:?}", msg).into());
        },
    )
    .await
    .unwrap();

    // Send a message
    let msg = TalkProtocol {
        username: "alice".to_string(),
        message: "Hello from WASM!".to_string(),
        action: Some(ClientAction::Join),
        room_id: 1,
        unixtime: 100,
    };

    conn.send(msg).unwrap();
    Ok(())
}
