// Simple Middleware to handle MQTT messages and store them in Redis
// Also provides a REST API to fetch the stored data
// Author: Ahmad Urabi

// Note that we are extensivly using serde_json::Value to read the payloads
// This is unsafe for production and should be replaced with properly
// structured data types

// In it's current state, the code only handles messages of a certain format, and will
// panic if the message is not in the expected format. Future versions should include
// a dynamic way to handle different message formats

// Also the current version isn't optimized for performance, as such you will see some
// obvious bottlenecks (Such as getting the redis client for every message, instead of a connection pool)

#[macro_use]
extern crate rocket;
extern crate redis;

mod drone_emulator;
mod message_handler;
mod redis_client;

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use std::{sync::Arc, thread};

use drone_emulator::{create_fake_data, DroneData};
use redis_client::StreamObj;
use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::Header;
use rocket::serde::json::Json;
use rocket::{Request, Response};
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS};

pub struct CORS;
// CORS control
#[rocket::async_trait]
impl Fairing for CORS {
    fn info(&self) -> Info {
        Info {
            name: "Add CORS headers to responses",
            kind: Kind::Response,
        }
    }

    async fn on_response<'r>(&self, _request: &'r Request<'_>, response: &mut Response<'r>) {
        response.set_header(Header::new("Access-Control-Allow-Origin", "*"));
        response.set_header(Header::new(
            "Access-Control-Allow-Methods",
            "POST, GET, PATCH, OPTIONS",
        ));
        response.set_header(Header::new("Access-Control-Allow-Headers", "*"));
        response.set_header(Header::new("Access-Control-Allow-Credentials", "true"));
    }
}

#[get("/")]
async fn index() -> Json<Vec<serde_json::Value>> {
    let v = redis_client::get_all_features().await.unwrap_or(vec![]);
    Json(v)
}

#[get("/stream")]
async fn get_stream_route() -> Json<Vec<StreamObj>> {
    let v = redis_client::get_stream("drone_data").await;
    Json(v)
}

#[rocket::main]
async fn main() {
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_signal = shutdown.clone();

    // Set up Ctrl+C handler
    ctrlc::set_handler(move || {
        println!("Shutting down...");
        shutdown_signal.store(true, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl+C handler");

    // MQTT setup
    let mut mqttoptions = MqttOptions::new("rumqtt-async", "localhost", 1883);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);
    client.subscribe("presence", QoS::AtMostOnce).await.unwrap();

    let server = rocket::build()
        .attach(CORS)
        .mount("/", routes![index, get_stream_route]);
    // Start Rocket server in a separate thread
    let rocket_handle = thread::spawn(move || {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let _ = server.launch().await;
        });
    });

    // Start MQTT event loop in a separate thread and end it when the shutdown signal is received
    let mqtt_client_handle = {
        let shutdown = shutdown.clone();
        thread::spawn(move || {
            tokio::runtime::Runtime::new().unwrap().block_on(async {
                while !shutdown.load(Ordering::SeqCst) {
                    if let Ok(notification) = eventloop.poll().await {
                        match notification {
                            Event::Incoming(i) => match i {
                                Incoming::Publish(p) => {
                                    let bytes = p.payload.to_vec();
                                    let payload = String::from_utf8(bytes).unwrap();
                                    let msg: serde_json::Value =
                                        serde_json::from_str(&payload).unwrap();
                                    message_handler::read_message(msg).await;
                                }
                                _ => {}
                            },
                            _ => {}
                        }
                    }
                }
            });
        })
    };

    let mqtt_publisher_handler = {
        let shutdown = shutdown.clone();
        thread::spawn(move || {
            let shutdown = shutdown.clone();
            let mut previous_state: Option<DroneData> = None;
            tokio::runtime::Runtime::new().unwrap().block_on(async {
                while !shutdown.load(Ordering::SeqCst) {
                    let data = create_fake_data(&previous_state).await;
                    previous_state = Some(data.clone());
                    let payload: String = serde_json::to_string(&data).unwrap();
                    client
                        .publish("topic/drone", QoS::AtMostOnce, false, payload)
                        .await
                        .unwrap();
                    thread::sleep(Duration::from_millis(500));
                }
            });
        })
    };

    while !shutdown.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(100));
    }

    mqtt_publisher_handler.join().unwrap();
    mqtt_client_handle.join().unwrap();
    rocket_handle.join().unwrap();
}
