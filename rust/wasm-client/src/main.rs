use futures_channel::mpsc::{UnboundedSender, unbounded};
use futures_util::{SinkExt, StreamExt};
use gloo_net::websocket::futures::WebSocket;
use log::info;
use shared::{
    TalkProtocol,
    wasm::{receiver_task, sender_task},
};
use std::{sync::Arc, sync::Mutex};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

struct AppState {
    pub tx: UnboundedSender<TalkProtocol>,
    pub messages: Arc<Mutex<Vec<TalkProtocol>>>,
}

#[function_component]
fn App() -> Html {
    let username = use_state(|| "".to_string());
    let message = use_state(|| "".to_string());

    let on_input_username = {
        let username = username.clone();
        Callback::from(move |e: InputEvent| {
            let value = e
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value();
            username.set(value);
        })
    };

    let on_input_message = {
        let message = message.clone();
        Callback::from(move |e: InputEvent| {
            let value = e
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value();
            message.set(value);
        })
    };

    let on_send = { Callback::from(move |_| {}) };

    html! {
        <div>
            <div>
                <input type="text" oninput={on_input_username} />
                <input type="text" oninput={on_input_message} />
                <button onclick={on_send}>
                    { "Click me!" }
                </button>
                <p>{ format!("Username: {}", *username) }</p>
                <p>{ format!("Message: {}", *message) }</p>
            </div>
            /* ... your existing UI ... */

        </div>
    }
}

fn main() {
    let url = "ws://localhost:8080";
    let conn = WebSocket::open(url).unwrap();

    let (tx, rx) = unbounded::<TalkProtocol>();
    let (write, read) = conn.split();

    let messages = Arc::new(Mutex::new(Vec::<TalkProtocol>::new()));

    let msg_sender = Arc::clone(&messages);

    spawn_local(sender_task(rx, write));
    spawn_local(receiver_task(read, msg_sender));

    let msg1 = TalkProtocol {
        username: "wasm".to_string(),
        message: "--OLA--".to_string(),
        action: None,
        room_id: 0,
        unixtime: 100,
    };

    let _ = tx.unbounded_send(msg1);
    yew::Renderer::<App>::new().render();
}
