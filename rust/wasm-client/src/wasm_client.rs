use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use gloo_net::websocket::futures::WebSocket;
use shared::wasm::*;
use shared::{TalkMessage, TalkProtocol};
use uuid::Uuid;
use wasm_bindgen_futures::js_sys;
use yew::prelude::*;
use web_sys::HtmlElement;
use gloo_utils::document;
use web_sys::wasm_bindgen::JsCast;
pub struct ChatClient {
    ws_sender: Option<UnboundedSender<TalkProtocol>>,
    messages: Vec<TalkProtocol>,
    input_text: String,
    username: String,
    room_id: i32,
    connected: bool,
    uuid: Uuid,
}

pub enum Msg {
    Connect,
    Disconnect,
    SendMessage,
    UpdateInput(String),
    UpdateUsername(String),
    UpdateRoomId(String),
    ReceivedMessage(TalkProtocol),
    ConnectionClosed,
}

impl Component for ChatClient {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        Self {
            ws_sender: None,
            messages: Vec::new(),
            input_text: String::new(),
            username: "user".to_string(),
            room_id: 1,
            connected: false,
            uuid: Uuid::new_v4(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
         if let Some(messages_container) = document().get_element_by_id("messages-container") {
            let messages_container = messages_container.dyn_into::<HtmlElement>().unwrap();
            messages_container.set_scroll_top(messages_container.scroll_height());
        }
        match msg {
            Msg::Connect => {
                if self.connected {
                    return false;
                }

                let url = format!("ws://localhost:9999/ws");
                match WebSocket::open(&url) {
                    Ok(ws) => {
                        let (write, read) = ws.split();
                        let (tx, rx) = unbounded();

                        // Store the sender
                        self.ws_sender = Some(tx.clone());
                        self.connected = true;

                        // Spawn sender task
                        let link = ctx.link().clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            sender_task(rx, write).await;
                            link.send_message(Msg::ConnectionClosed);
                        });

                        // Spawn receiver task
                        let link = ctx.link().clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            receiver_task(read, move |msg| {
                                link.send_message(Msg::ReceivedMessage(msg));
                            })
                            .await;
                        });

                        // Send join room message
                        if let Some(sender) = &self.ws_sender {
                            let join_msg = TalkProtocol::JoinRoom {
                                room_id: self.room_id,
                                uuid: self.uuid,
                                username: self.username.clone(),
                                unixtime: js_sys::Date::now() as u64,
                            };
                            let _ = sender.unbounded_send(join_msg);
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to connect: {:?}", e);
                    }
                }
                true
            }

            Msg::Disconnect => {
                if let Some(sender) = self.ws_sender.take() {
                    // Send leave room message
                    let leave_msg = TalkProtocol::LeaveRoom {
                        room_id: self.room_id,
                        uuid: self.uuid,
                        username: self.username.clone(),
                        unixtime: js_sys::Date::now() as u64,
                    };
                    let _ = sender.unbounded_send(leave_msg);

                    // Drop sender to close connection
                    drop(sender);
                }
                self.connected = false;
                true
            }

            Msg::SendMessage => {
                if let Some(sender) = &self.ws_sender {
                    if !self.input_text.trim().is_empty() {
                        let message = TalkProtocol::PostMessage {
                            message: TalkMessage {
                                uuid: self.uuid,
                                username: self.username.clone(),
                                text: self.input_text.clone(),
                                room_id: self.room_id,
                                unixtime: js_sys::Date::now() as u64,
                            },
                        };
                        let _ = sender.unbounded_send(message);
                        self.input_text.clear();
                    }
                }
                true
            }

            Msg::UpdateInput(text) => {
                self.input_text = text;
                true
            }

            Msg::UpdateUsername(username) => {
                if let Some(sender) = &self.ws_sender {
                    let change_msg = TalkProtocol::ChangeName {
                        uuid: self.uuid,
                        username: username.clone(),
                        old_username: self.username.clone(),
                        unixtime: js_sys::Date::now() as u64,
                    };
                    let _ = sender.unbounded_send(change_msg);
                }
                self.username = username;
                true
            }

            Msg::UpdateRoomId(room_str) => {
                if let Ok(room_id) = room_str.parse() {
                    self.room_id = room_id;
                }
                true
            }

            Msg::ReceivedMessage(msg) => {
                self.messages.push(msg);
                true
            }

            Msg::ConnectionClosed => {
                self.connected = false;
                self.ws_sender = None;
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let on_connect = ctx.link().callback(|_: MouseEvent| Msg::Connect);
        let on_disconnect = ctx.link().callback(|_: MouseEvent| Msg::Disconnect);
        let on_send = ctx.link().callback(|_: MouseEvent| Msg::SendMessage);
        let on_input = ctx.link().batch_callback(|e: InputEvent| {
            let input = e.target_unchecked_into::<web_sys::HtmlInputElement>();
            Some(Msg::UpdateInput(input.value()))
        });
        let on_username_change = ctx.link().batch_callback(|e: Event| {
            let input = e.target_unchecked_into::<web_sys::HtmlInputElement>();
            Some(Msg::UpdateUsername(input.value()))
        });
        let on_room_change = ctx.link().batch_callback(|e: Event| {
            let input = e.target_unchecked_into::<web_sys::HtmlInputElement>();
            Some(Msg::UpdateRoomId(input.value()))
        });

        html! {
            <div class="chat-client">
                <div class="chat-header">
                    <h1>{"Chat Client"}</h1>
                    <div class="connection-controls">
                        if !self.connected {
                            <button onclick={on_connect}>{"Connect"}</button>
                        } else {
                            <button onclick={on_disconnect}>{"Disconnect"}</button>
                        }
                    </div>
                </div>

                <div class="user-info">
                    <input
                        type="text"
                        value={self.username.clone()}
                        placeholder="Username"
                        onchange={on_username_change}
                    />
                    <input
                        type="number"
                        value={self.room_id.to_string()}
                        placeholder="Room ID"
                        onchange={on_room_change}
                    />
                </div>

                <div id="messages-container" class="messages">
                    {for self.messages.iter().map(|msg| self.render_message(msg))}
                </div>

                <div class="input-area">
                    <input
                        type="text"
                        value={self.input_text.clone()}
                        placeholder="Type a message..."
                        oninput={on_input}
                        onkeypress={ctx.link().batch_callback(|e: KeyboardEvent| {
                            if e.key() == "Enter" {
                                Some(Msg::SendMessage)
                            } else {
                                None
                            }
                        })}
                        disabled={!self.connected}
                    />
                    <button onclick={on_send} disabled={!self.connected}>
                        {"Send"}
                    </button>
                </div>
            </div>
        }
    }
}

impl ChatClient {
    fn render_message(&self, msg: &TalkProtocol) -> Html {
        match msg {
            TalkProtocol::PostMessage { message } => {
                let is_own_message = message.uuid == self.uuid;
                let message_class = if is_own_message { "message own" } else { "message other" };
                
                html! {
                    <div class={message_class}>
                        <div class="message-header">
                            <span class="username">{&message.username}</span>
                            <span class="time">{Self::format_time(message.unixtime)}</span>
                        </div>
                        <div class="message-text">{&message.text}</div>
                    </div>
                }
            }
            TalkProtocol::UserJoined { username, room_id, .. } => html! {
                <div class="system-message">
                    {format!("{} joined room {}", username, room_id)}
                </div>
            },
            TalkProtocol::UserLeft { username, room_id, .. } => html! {
                <div class="system-message">
                    {format!("{} left room {}", username, room_id)}
                </div>
            },
            TalkProtocol::UsernameChanged { username, old_username, .. } => html! {
                <div class="system-message">
                    {format!("{} changed name to {}", old_username, username)}
                </div>
            },
            TalkProtocol::Error { code, message } => html! {
                <div class="error-message">
                    {format!("Error {}: {}", code, message)}
                </div>
            },
            _ => html! {},
        }
    }
    fn format_time(unixtime: u64) -> String {
        let date = js_sys::Date::new(&js_sys::Date::new_0().constructor());
        date.set_time(unixtime as f64 * 1000.0);
        format!("{}:{}", date.get_hours(), date.get_minutes())
    }
}
