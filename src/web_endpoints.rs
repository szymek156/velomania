use crate::AppState;
use actix_web::{get, web::Data, HttpResponse, Responder};
use futures::stream::StreamExt;

use tokio_stream::wrappers::BroadcastStream;

#[get("/hello")]
async fn hello() -> impl Responder {
    "HAI"
}

/// This is a stream endpoint, one line contains one workout state
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
