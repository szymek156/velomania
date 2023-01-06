use actix::prelude::*;
use actix_web_actors::ws;
use futures::StreamExt;
use tokio::sync::{broadcast, mpsc};
use tokio_stream::wrappers::BroadcastStream;

use std::time::{Duration, Instant};

use crate::{cli::WorkoutCommands, workout_state::WorkoutState};

///! Actor implementation for handling websocket endpoint for workout_state

/// How often heartbeat pings are sent
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);

/// How long before lack of client response causes a timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

pub struct WebSocketActor {
    pub workout_state_rx: broadcast::Receiver<WorkoutState>,
    pub control_workout_tx: mpsc::Sender<WorkoutCommands>,
    pub hb: Instant,
}

impl Actor for WebSocketActor {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("WS actor started");
        let workout_state_rx =
            BroadcastStream::new(self.workout_state_rx.resubscribe()).map(|msg| {
                let state = msg.unwrap();
                NewWorkoutState::from(state)
            });

        ctx.add_stream(workout_state_rx);

        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            // check client heartbeats
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                // heartbeat timed out
                warn!("Websocket Client heartbeat failed, disconnecting!");
                // stop actor
                ctx.stop();

                // don't try to send a ping
                return;
            }

            ctx.ping(b"");
        });
    }
}

// Messaging, definition of messages that goes to the actor from the App

#[derive(Message)]
#[rtype(result = "()")]
pub struct NewWorkoutState(WorkoutState);

impl From<WorkoutState> for NewWorkoutState {
    fn from(value: WorkoutState) -> Self {
        NewWorkoutState(value)
    }
}

impl StreamHandler<NewWorkoutState> for WebSocketActor {
    fn handle(&mut self, item: NewWorkoutState, ctx: &mut Self::Context) {
        // Push the workout state to the WebSocket as a text
        ctx.text(serde_json::to_string(&item.0).unwrap());
    }
}

/// WebSocket messages that comes from the client
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WebSocketActor {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        let msg = match msg {
            Err(e) => {
                error!("WS RX error {e}");
                ctx.stop();
                return;
            }
            Ok(msg) => msg,
        };

        trace!("WEBSOCKET MESSAGE: {msg:?}");
        match msg {
            ws::Message::Text(data) => {
                let input = data.trim().to_ascii_uppercase();

                match input.as_str() {
                    "S" => {
                        let tx = self.control_workout_tx.clone();
                        ctx.spawn(
                            async move {
                                tx.send(WorkoutCommands::SkipStep).await.unwrap();
                            }
                            .into_actor(self),
                        );
                    }
                    "Q" => {
                        // TODO: We are in async context, cannot call blocking_send in it, need to
                        // spawn a dedicated task for it, it sucks, note handle is not async, so cannot
                        // call .await either :\
                        // let _ = self
                        //     .control_workout_tx
                        //     .blocking_send(WorkoutCommands::Abort);

                        let tx = self.control_workout_tx.clone();
                        // TODO: no way to wait on a spawned handle, WTF!
                        ctx.spawn(
                            async move {
                                tx.send(WorkoutCommands::SkipStep).await.unwrap();
                            }
                            .into_actor(self),
                        );



                    }
                    other => {
                        warn!("Unexpected user input {other}");
                    }
                }
            }
            ws::Message::Binary(_) => todo!(),
            ws::Message::Continuation(_) => todo!(),
            ws::Message::Ping(msg) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            ws::Message::Pong(_) => {
                self.hb = Instant::now();
            }
            ws::Message::Close(_) => todo!(),
            ws::Message::Nop => todo!(),
        }
    }
}
