use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use web_sys::WebSocket;
use web_sys::{CloseEvent, ErrorEvent}; // Needed for `dyn_into()` and `unchecked_ref()`

use web_sys::{Event, HtmlElement, HtmlInputElement, window};

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let ws = WebSocket::new("ws://localhost:12345")?;

    // (WebSocket callbacks: onopen, onmessage, etc.) ...
    // [snipped for brevity]

    // Clone WebSocket so it can be used in the closure
    let ws_clone = ws.clone();

    // Access window and document
    let window = window().unwrap();
    let document = window.document().unwrap();

    // Get input and button elements
    let input = document
        .get_element_by_id("chat-input")
        .unwrap()
        .dyn_into::<HtmlInputElement>()?;
    let button = document
        .get_element_by_id("send-button")
        .unwrap()
        .dyn_into::<HtmlElement>()?;

    // Set onclick event handler
    let closure = Closure::wrap(Box::new(move |_event: Event| {
        let msg = input.value();
        if !msg.is_empty() {
            ws_clone.send_with_str(&msg).unwrap();
            web_sys::console::log_1(&format!("Sent: {}", msg).into());
            input.set_value(""); // clear input
        } else {
            web_sys::console::log_1(&"Message is empty".into());
        }
    }) as Box<dyn FnMut(_)>);

    let onerror = Closure::wrap(Box::new(move |e: ErrorEvent| {
        web_sys::console::error_1(&format!("WebSocket error: {:?}", e).into());
    }) as Box<dyn FnMut(_)>);
    ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
    onerror.forget();

    let onclose = Closure::wrap(Box::new(move |e: CloseEvent| {
        web_sys::console::log_1(
            &format!(
                "WebSocket closed (code: {}, reason: {}, was_clean: {})",
                e.code(),
                e.reason(),
                e.was_clean()
            )
            .into(),
        );
    }) as Box<dyn FnMut(_)>);
    ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
    onclose.forget();
    button.set_onclick(Some(closure.as_ref().unchecked_ref()));
    closure.forget(); // prevent it from being dropped

    Ok(())
}
