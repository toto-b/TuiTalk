use futures_channel::mpsc::{UnboundedSender, unbounded};
use futures_util::StreamExt;
use gloo_net::websocket::futures::WebSocket;
use shared::TalkProtocol;
use shared::wasm::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[function_component(App)]
pub fn app() -> Html {
    // Component state
    let username = use_state(|| String::new());
    let message = use_state(|| String::new());
    let messages = use_state(Vec::<TalkProtocol>::new);
    let tx = use_state(|| None::<UnboundedSender<TalkProtocol>>);

    // Initialize WebSocket connection once on component mount
    use_effect_with_deps(
        {
            let messages = messages.clone();
            let tx = tx.clone();

            move |_| {
                let ws = match WebSocket::open("ws://localhost:8080") {
                    Ok(ws) => ws,
                    Err(e) => {
                        log::error!("WebSocket connection failed: {:?}", e);
                        return;
                    }
                };

                let (write, read) = ws.split();
                let (tx_ws, rx) = unbounded::<TalkProtocol>();

                // Spawn sender task
                spawn_local({
                    async move {
                        sender_task(rx, write).await;
                    }
                });

                let messages = messages.clone();
                spawn_local(async move {
                    receiver_task(read, messages).await;
                });
                // Spawn receiver task
                // spawn_local({
                //     let messages = messages.clone();
                //     async move {
                //         let mut read = read;
                //         while let Some(msg) = read.next().await {
                //             match msg {
                //                 Ok(Message::Bytes(bin)) => {
                //                     if let Ok(parsed) = TalkProtocol::deserialize(&bin) {
                //                         let mut current = (*messages).clone();
                //                         current.push(parsed);
                //                         messages.set(current);
                //                     }
                //                 }
                //                 Ok(Message::Text(text)) => {
                //                     log::info!("Received text message: {}", text);
                //                 }
                //                 Err(e) => {
                //                     log::error!("WebSocket error: {:?}", e);
                //                 }
                //             }
                //         }
                //     }
                // });

                // Store the sender in component state
                tx.set(Some(tx_ws));

                // Cleanup on unmount
                {
                    log::info!("WebSocket cleanup");
                }
            }
        },
        (),
    );

    // Event handlers
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

    let on_send = {
        let tx = tx.clone();
        let username = username.clone();
        let message = message.clone();
        Callback::from(move |_| {
            if let Some(tx) = &*tx {
                let msg = TalkProtocol {
                    username: (*username).clone(),
                    message: (*message).clone(),
                    action: None,
                    room_id: 0,
                    unixtime: 0,
                };
                if let Err(e) = tx.unbounded_send(msg) {
                    log::error!("Failed to send message: {:?}", e);
                }
                message.set(String::new()); // Clear input after sending
            }
        })
    };

    // Render UI
    html! {
    <div class="app">
        <div class="chat-container">
            <div class="message-input">
                <input
                    type="text"
                    placeholder="Username"
                    value={(*username).clone()}
                    oninput={on_input_username}
                />
                <input
                    type="text"
                    placeholder="Type a message"
                    value={(*message).clone()}
                    oninput={on_input_message}
                />
                <button onclick={on_send}>
                    { "Send" }
                </button>
            </div>
            <div class="messages">
                <h2>{ "Messages" }</h2>
                <ul>
                    {(*messages).iter().map(|msg| {
                    html! {
                    <li key={msg.unixtime.to_string()}>
                        <strong>{ &msg.username }</strong> { &msg.message }
                    </li>
                    }
                    }).collect::<Html>()}
                </ul>
            </div>
        </div>
    </div>
    }
}

pub fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::Renderer::<App>::new().render();
}
