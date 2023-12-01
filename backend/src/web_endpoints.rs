use std::time::Instant;

use crate::{workout_state_ws::WebSocketActor, AppState};
use actix_web::{
    get,
    web::{self, Data},
    Error, HttpRequest, HttpResponse, Responder,
};
use actix_web_actors::ws;
use futures::stream::StreamExt;

use tokio_stream::wrappers::BroadcastStream;

#[get("/hello")]
async fn hello() -> impl Responder {
    "HAI"
}

/// This is a stream endpoint, one line contains one workout state
/// In HTTP/1 it uses header <transfer-encoding: chunked
/// IN HTTP/2 uses DATA frames

#[get("/workout_state")]
async fn workout_state_handle(app_state: Data<AppState>) -> HttpResponse {
    let guard = app_state.workout_state_tx.read().unwrap();

    if let Some(workout_state) = guard.as_ref() {
        let stream = BroadcastStream::new(workout_state.subscribe());

        let stream = stream.map(|element| {
            let state = element?;
            let serialized = format!("{}\n", serde_json::to_string(&state)?);
            anyhow::Ok(actix_web::web::Bytes::from(serialized))
        });

        HttpResponse::Ok()
            // .content_type("application/json")
            .streaming(stream)
    } else {
        HttpResponse::BadRequest().finish()
    }
}

/// Opens a persistent connection with the client, provides all the data, workout state, trainer status
/// and accepts commands
#[get("/ws")]
async fn web_socket_handle(
    req: HttpRequest,
    stream: web::Payload,
    app_state: Data<AppState>,
) -> Result<HttpResponse, Error> {
    let guard = app_state.workout_state_tx.read().unwrap();

    if let Some(workout_state) = guard.as_ref() {
        let workout_state_rx = workout_state.subscribe();

        let actor = WebSocketActor {
            workout_state_rx,
            control_workout_tx: app_state.control_workout_tx.clone(),
            hb: Instant::now(),
        };

        info!("starting WS actor");
        // Performs ws handshake, and starts the actor
        ws::start(actor, &req, stream)
    } else {
        Ok(HttpResponse::BadRequest().finish())
    }
}
