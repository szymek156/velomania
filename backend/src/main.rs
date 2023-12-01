#[macro_use]
extern crate num_derive;
use std::{
    fs::File,
    io::{self, BufReader},
    path::{Path, PathBuf},
    sync::RwLock,
    thread,
    time::Duration,
};

use actix_web::{middleware, App, HttpServer};
use rustls::{Certificate, PrivateKey, ServerConfig};
use rustls_pemfile::{certs, pkcs8_private_keys};
use structopt::StructOpt;
use workout_state::WorkoutState;
use zwo_workout::ZwoWorkout;

use crate::ble_client::BleClient;
use anyhow::Result;
use cli::{UserCommands, WorkoutCommands};
use futures::StreamExt;
use indoor_bike_client::IndoorBikeFitnessMachine;
use indoor_bike_data_defs::ControlPointResult;
use signal_hook::consts::signal::*;
use signal_hook_async_std::Signals;
use tokio::{
    sync::{broadcast, mpsc},
    task,
};

mod bk_gatts_service;
mod ble_client;
mod cli;
mod common;
mod front;
mod indoor_bike_client;
mod indoor_bike_data_defs;
mod scalar_converter;
mod web_endpoints;
mod workout_state;
mod workout_state_ws;
mod zwo_workout;
mod zwo_workout_file;
#[macro_use]
extern crate log;

#[derive(StructOpt)]
struct Args {
    /// Workout .zwo file
    #[structopt(short, long, parse(from_os_str))]
    workout: PathBuf,

    #[structopt(short, long)]
    ftp_base: f64,
}

struct AppState {
    workout_state_tx: RwLock<Option<broadcast::Sender<WorkoutState>>>,
    control_workout_tx: mpsc::Sender<WorkoutCommands>,
}

// TODO: why not tokio::main?
#[actix_web::main]
async fn main() -> Result<()> {
    env_logger::init();

    let connect_to_trainer = true;

    let opt = Args::from_args();

    // Channel used by workout task to broadcast power value to be set - received by control_fit_machine, but also by frontend
    let (trainer_commands_tx, _command_rx) = tokio::sync::broadcast::channel(16);
    let (workout_state_tx, _rx) = tokio::sync::broadcast::channel(16);

    // Channel used to control workout, skip step, pause
    let (control_workout_tx, control_workout_rx) = tokio::sync::mpsc::channel(16);

    let app_state = actix_web::web::Data::new(AppState {
        workout_state_tx: RwLock::new(Some(workout_state_tx)),
        control_workout_tx,
    });

    register_signal_handler(trainer_commands_tx.clone());

    let (fit, bike_notifications, training_notifications, machine_status_notifications) = {
        if connect_to_trainer {
            let fit = connect_to_fit().await?;
            let bike_notifications = fit.subscribe_for_indoor_bike_notifications();
            let training_notifications = fit.subscribe_for_training_notifications();
            let machine_status_notifications = fit.subscribe_for_machine_notifications();

            (
                Some(fit),
                Some(bike_notifications),
                Some(training_notifications),
                Some(machine_status_notifications),
            )
        } else {
            // TODO: create fake data in the future
            (None, None, None, None)
        }
    };

    // Start workout task, will broadcast next steps
    let workout_join_handle = start_workout(
        trainer_commands_tx.clone(),
        app_state.clone(),
        control_workout_rx,
        opt.workout.as_path(),
        opt.ftp_base,
    )
    .await?;

    handle_user_input(app_state.control_workout_tx.clone());

    // // // Tui shows current step + data from trainer
    // let tui_join_handle = tokio::spawn(front::tui::show(
    //     _rx,
    //     bike_notifications,
    //     training_notifications,
    //     machine_status_notifications,
    // ));

    tokio::spawn(async move {
        if let Some(fit) = fit {
            control_fit_machine(fit, trainer_commands_tx.subscribe())
                .await
                .unwrap();
        } else {
            // Listen for sigterm
            let mut rx = trainer_commands_tx.subscribe();
            while let Ok(message) = rx.recv().await {
                if let UserCommands::Exit = message {
                    info!("Exit!");
                    break;
                }
            }
        };

        workout_join_handle.abort();
        // tui_join_handle.abort();
    });

    // Use HTTPS in order to upgrade to HTTP/2 - done automagically when possible by actix,
    // In actix-web H2C (HTTP2 without HTTPS) is not supported,
    // there is an issue opened for it for quite some time
    let _tls_conf = load_rustls_config();

    HttpServer::new(move || {
        // HttpServer accepts an application factory rather than an application instance.
        // An HttpServer constructs an application instance for EACH thread.
        // Therefore, application data must be constructed multiple times.
        // If you want to share data between different threads,
        // a shareable object should be used, e.g. Send + Sync.
        App::new()
            .wrap(middleware::Logger::default())
            .app_data(app_state.clone())
            .service(web_endpoints::workout_state_handle)
            .service(web_endpoints::web_socket_handle)
    })
    // TODO: wss does not work for some reason
    // .bind_rustls(("127.0.0.1", 2137), tls_conf)?
    .bind(("127.0.0.1", 2137))?
    .run()
    .await?;

    Ok(())
}

fn load_rustls_config() -> rustls::ServerConfig {
    // init server config builder with safe defaults
    let config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth();

    // load TLS key/cert files
    let cert_file = &mut BufReader::new(File::open("backend/tls/cert.pem").unwrap());
    let key_file = &mut BufReader::new(File::open("backend/tls/key.pem").unwrap());

    // convert files to key/cert objects
    let cert_chain = certs(cert_file)
        .unwrap()
        .into_iter()
        .map(Certificate)
        .collect();
    let mut keys: Vec<PrivateKey> = pkcs8_private_keys(key_file)
        .unwrap()
        .into_iter()
        .map(PrivateKey)
        .collect();

    // exit if no keys could be parsed
    if keys.is_empty() {
        eprintln!("Could not locate PKCS 8 private keys.");
        std::process::exit(1);
    }

    config.with_single_cert(cert_chain, keys.remove(0)).unwrap()
}

/// Reads ZWO file, and sends commands according to it
async fn start_workout(
    trainer_commands_tx: tokio::sync::broadcast::Sender<UserCommands>,
    app_state: actix_web::web::Data<AppState>,
    mut control_workout_rx: tokio::sync::mpsc::Receiver<WorkoutCommands>,
    workout: &Path,
    ftp_base: f64,
) -> Result<tokio::task::JoinHandle<()>> {
    let mut workout = ZwoWorkout::new(workout, ftp_base).await?;

    let handle = tokio::spawn(async move {
        debug!("spawning workout task");

        let propagate_workout_state = tokio::time::interval(Duration::from_secs(1));
        tokio::pin!(propagate_workout_state);

        let workout_state_tx = {
            let guard = app_state.workout_state_tx.read().unwrap();

            guard.as_ref().cloned().unwrap()
        };

        trainer_commands_tx
            .send(UserCommands::StartWorkout)
            .unwrap();

        loop {
            tokio::select! {
                workout_step = workout.next() => {
                    // Next step is available
                    match workout_step {
                        Some(command) => {
                            debug!("Got command from workout: {command:?}");
                            debug!("workout {}/{}",
                                workout.workout_state.current_step_number,
                                workout.workout_state.total_steps);

                            debug!("workout {:?}", workout.current_step);
                            trainer_commands_tx.send(command).unwrap();
                        }
                        None => {
                            debug!("No more steps in workout, workout task exits");
                            trainer_commands_tx.send(UserCommands::Exit).unwrap();

                            break;
                        },
                    }
                }
                // TODO: this is a workaround, ideally there would be:
                //
                // Some(workout_state) = workout.workout_state.next() => {
                //     workout_state_tx.send(workout.workout_state.clone()).unwrap();
                // }
                // But BC complains that mut borrow is already held on workout,
                // figure something out here
                // TODO: Arc? Gets immutable borrow Nope, RefCell? Nope will panic during runtime
                // Mutex? Will deadlock
                // Do subscribe to the channel from the workout state?
                // Move workout state as separate actor, let workout communicate to state via channel
                // to update it
                _ = propagate_workout_state.tick() => {
                    debug!("Broadcast workout state {}/{}",
                        workout.workout_state.current_step_number,
                        workout.workout_state.total_steps);

                    workout.workout_state.update_ts();
                    workout_state_tx.send(workout.workout_state.clone()).unwrap();
                }
                Some(control)  = control_workout_rx.recv() => {
                    match control {
                        WorkoutCommands::Pause=> workout.pause(),
                        WorkoutCommands::Resume=> todo!(),
                        WorkoutCommands::SkipStep=> workout.skip_step(),
                        WorkoutCommands::Abort => {
                            trainer_commands_tx.send(UserCommands::Exit).unwrap();
                            break;
                        },
                    }
                }
            }
        }

        {
            // Workout completed, drop workout_state_tx, so all receivers will close
            // TODO: note if someone will clone workout_state_tx (which is possible - broadcast channel allows that)
            // that will break the whole idea - streams would not be closed until all tx instances are not dropped
            let mut guard = app_state.workout_state_tx.write().unwrap();
            let _ = guard.take();
        }
    });

    Ok(handle)
}

/// Gets the commands (may be ZWO workout, or user input), and passes them to the fitness machine
async fn control_fit_machine(
    fit: IndoorBikeFitnessMachine,
    mut rx: broadcast::Receiver<UserCommands>,
) -> Result<()> {
    // Cannot set return type of async block, async closures are unstable

    fit.dump_service_info().await?;
    fit.get_features().await?;

    // TODO: Use select?
    // let _status_notifications = fit.subscribe_for_status_notifications();

    let mut cp_notifications = fit.subscribe_for_control_point_notifications();

    while let Ok(message) = rx.recv().await {
        match message {
            UserCommands::Exit => {
                info!("Control task exits");
                break;
            }
            UserCommands::SetResistance { resistance } => {
                fit.set_resistance(resistance).await?;
            }
            UserCommands::SetTargetPower { power } => {
                fit.set_power(power).await?;
            }
            UserCommands::StartWorkout => {
                fit.reset_status().await?;
            }
        }

        // Wait for CP notification response for above write request
        let resp = cp_notifications.recv().await?;
        match resp.request_status {
            ControlPointResult::Success => {
                debug!("Got ACK for request {resp:?}");
            }
            _ => {
                error!("Received NACK for request: {resp:?}");
            }
        }
    }

    fit.disconnect().await?;

    Ok(())
}

fn register_signal_handler(tx: tokio::sync::broadcast::Sender<UserCommands>) {
    task::spawn(async move {
        info!("Signal handler waits for events");

        let mut signals = Signals::new([SIGINT]).unwrap();

        match signals.next().await {
            Some(sig) => {
                warn!("Got signal {sig}");
                tx.send(UserCommands::Exit).unwrap();
            }
            None => unreachable!("Signals stream closed?"),
        }
    });
}

async fn connect_to_fit() -> Result<IndoorBikeFitnessMachine> {
    let ble = BleClient::new().await;
    // ble.connect_to_bc().await.unwrap();

    let fit = IndoorBikeFitnessMachine::new(&ble).await?;

    Ok(fit)
}

pub fn handle_user_input(tx: tokio::sync::mpsc::Sender<WorkoutCommands>) {
    // It's not recommended to handle user input using async.
    // Spawn dedicated thread instead.

    // dropped join handle detaches thread
    thread::spawn(move || {
        info!("Waiting for user input");
        loop {
            let mut buffer = String::new();
            let res = io::stdin().read_line(&mut buffer);

            if let Err(e) = res {
                error!("Got error while reading stdin {e}, exiting");
                tx.blocking_send(WorkoutCommands::Abort).unwrap();
                break;
            }

            let input = buffer.trim().to_ascii_uppercase();

            match input.as_str() {
                "S" => {
                    tx.blocking_send(WorkoutCommands::SkipStep).unwrap();
                }
                "Q" => {
                    let _ = tx.blocking_send(WorkoutCommands::Abort);
                    break;
                }
                other => {
                    warn!("Unexpected user input {other}");
                }
            }
        }
        info!("Waiting for user input leaves");
    });
}
