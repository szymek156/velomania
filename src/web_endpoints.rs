use std::{sync::Arc, time::Duration};

use crate::{
    workout_state::{PoorWorkoutState, WorkoutState, self},
    AppState,
};
use actix_web::{get, web::Data, Error, HttpResponse, Responder};
use futures::{
    future::ok,
    stream::{self, once, StreamExt},
};
use tokio::sync::broadcast::{Receiver, Sender};
use tokio_stream::wrappers::{errors::BroadcastStreamRecvError, BroadcastStream};

#[get("/hello")]
async fn hello() -> impl Responder {
    "HAI"
}

/// Client needs to read the chunks per line
#[get("/stream")]
async fn workout_stream(app_state: Data<AppState>) -> HttpResponse {
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
