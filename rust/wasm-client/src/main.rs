use futures_channel::mpsc::{UnboundedSender, unbounded};
use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use gloo_net::websocket::{Message, futures::WebSocket};
use shared::{wasm::{receiver_task, sender_task}, TalkProtocol};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[function_component]
fn App() -> Html {
    let username = use_state(|| "".to_string());
    let message = use_state(|| "".to_string());

    let url = "ws://localhost:8080";
    let conn = WebSocket::open(url).unwrap();

    let (tx, rx) = unbounded::<TalkProtocol>();

    let (write, read) = conn.split();

    spawn_local(sender_task(rx, write));
    spawn_local(receiver_task(read, |cb_msg| {
        
    }));

    let msg1 = TalkProtocol {
        username: "client".to_string(),
        message: "Hello server and others!".to_string(),
        action: None,
        room_id: 0,
        unixtime: 100,
    };
    let _ = tx.unbounded_send(msg1);

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

    let on_send = { Callback::from(move |_| {

    }) };

    html! {
        <div>
            <input type="text" oninput={on_input_username} />
            <input type="text" oninput={on_input_message} />
            <button onclick={on_send}>
            { "Click me!" }
        </button>
            <p>{ format!("Username: {}", *username) }</p>
            <p>{ format!("Message: {}", *message) }</p>
        </div>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
