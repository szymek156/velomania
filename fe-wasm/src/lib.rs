pub mod gui;
mod utils;

use utils::set_panic_hook;
use wasm_bindgen::prelude::*;
use web_sys::{ErrorEvent, MessageEvent, WebSocket};

use crate::gui::Gui;

#[wasm_bindgen]
pub struct State {
    // ws: WebSocket,
    web_handle: WebHandle,
}

// On message, Received Text:
// "{\"total_steps\":2,\"current_step_number\":1,\"total_workout_duration\":{\"secs\":4800,\"nanos\":0},\"next_step\":{\"IntervalsT\":{\"Repeat\":2,\"OnDuration\":1500,\"OffDuration\":600,\"OnPower\":0.73,\"OffPower\":0.52}},\"current_power_set\":102,\"ftp_base\":200.0,\"current_step\":{\"duration\":{\"secs\":600,\"nanos\":0},\"step\":{\"Warmup\":{\"Duration\":600,\"PowerLow\":0.5,\"PowerHigh\":0.55}},\"elapsed\":{\"secs\":125,\"nanos\":40731351}},\"current_interval\":null,\"workout_elapsed\":{\"secs\":125,\"nanos\":40730481}}"

#[wasm_bindgen]
impl State {
    #[wasm_bindgen(constructor)]
    pub fn new(backend_endpoint: &str) -> Result<State, JsValue> {
        // let ws = State::connect_to_backend(backend_endpoint)?;
        let web_handle = WebHandle::new();

        Ok(Self {
            //  ws,
            web_handle,
        })
    }

    pub async fn start_gui(&self, canvas_id: &str) -> Result<(), JsValue> {
        self.web_handle.start(canvas_id).await
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
pub fn init(backend_endpoint: &str) -> Result<State, JsValue> {
    set_panic_hook();

    State::new(backend_endpoint)
}

/// Your handle to the web app from JavaScript.
#[derive(Clone)]
#[wasm_bindgen]
pub struct WebHandle {
    runner: eframe::WebRunner,
}

#[wasm_bindgen]
impl WebHandle {
    /// Installs a panic hook, then returns.
    #[allow(clippy::new_without_default)]
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        // Redirect [`log`] message to `console.log` and friends:
        eframe::WebLogger::init(log::LevelFilter::Debug).ok();

        Self {
            runner: eframe::WebRunner::new(),
        }
    }

    /// Call this once from JavaScript to start your app.
    #[wasm_bindgen]
    pub async fn start(&self, canvas_id: &str) -> Result<(), wasm_bindgen::JsValue> {
        self.runner
            .start(
                canvas_id,
                eframe::WebOptions::default(),
                Box::new(|cc| Box::new(Gui::new(cc))),
            )
            .await
    }

    // The following are optional:

    /// Shut down eframe and clean up resources.
    #[wasm_bindgen]
    pub fn destroy(&self) {
        self.runner.destroy();
    }

    /// Example on how to call into your app from JavaScript.
    #[wasm_bindgen]
    pub fn example(&self) {
        if let Some(app) = self.runner.app_mut::<Gui>() {
            // app.example();
        }
    }

    /// The JavaScript can check whether or not your app has crashed:
    #[wasm_bindgen]
    pub fn has_panicked(&self) -> bool {
        self.runner.has_panicked()
    }

    #[wasm_bindgen]
    pub fn panic_message(&self) -> Option<String> {
        self.runner.panic_summary().map(|s| s.message())
    }

    #[wasm_bindgen]
    pub fn panic_callstack(&self) -> Option<String> {
        self.runner.panic_summary().map(|s| s.callstack())
    }
}
