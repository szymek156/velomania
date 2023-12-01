mod utils;

use utils::set_panic_hook;
use wasm_bindgen::prelude::*;
use web_sys::{ErrorEvent, MessageEvent, WebSocket};

#[wasm_bindgen]
pub struct State {
    ws: WebSocket,
}

#[wasm_bindgen]
impl State {
    pub fn new(backend_endpoint: &str) -> Result<State, JsValue> {
        let ws = State::connect_to_backend(backend_endpoint)?;

        Ok(Self { ws })
    }

    fn connect_to_backend(backend_endpoint: &str) -> Result<WebSocket, JsValue> {
        console_log!("Connecting to a {backend_endpoint}");
        let ws = WebSocket::new(&backend_endpoint)?;

        // let (msg_tx, mut msg_rx) = tokio::sync::mpsc::channel(10);

        // Connect to an echo server
        let onmessage_callback = {
            // let msg_tx = msg_tx.clone();
            Closure::<dyn FnMut(_)>::new(move |e: MessageEvent| {
                if let Ok(txt) = e.data().dyn_into::<js_sys::JsString>() {
                    console_log!("On message, Received Text: {:?}", txt);
                    // if let Err(e) = msg_tx.blocking_send(Ok(txt)) {
                    //     console_log!("On message, Failed to relay the event: {e:?}");
                    // }
                } else {
                    console_log!("On message, received non-string data");
                }
            })
        };

        ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
        onmessage_callback.forget();

        let onerror_callback = {
            // let msg_tx = msg_tx.clone();
            Closure::<dyn FnMut(_)>::new(move |e: ErrorEvent| {
                console_log!("error event: {:?}", e);
                // if let Err(e) = msg_tx.blocking_send(Err(e)) {
                //     console_log!("On error, Failed to relay the event: {e:?}");
                // }
            })
        };

        ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
        onerror_callback.forget();

        let onopen_callback = {
            let cloned_ws = ws.clone();
            Closure::<dyn FnMut()>::new(move || {
                console_log!("socket opened");
                // match cloned_ws.send_with_str(&message) {
                //     Ok(_) => console_log!("message successfully sent"),
                //     Err(err) => console_log!("error sending message: {:?}", err),
                // }
            })
        };
        ws.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
        onopen_callback.forget();

        // let msg = msg_rx.recv().await.expect("channel closed")?;
        // console_log!("Waiting for message...{msg:?}");
        Ok(ws)
    }
}

#[wasm_bindgen]
pub fn init() -> Result<(), JsValue> {
    set_panic_hook();

    Ok(())
}
