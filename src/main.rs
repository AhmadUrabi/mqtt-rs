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

mod message_handler;
mod redis_client;

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use std::{sync::Arc, thread};

use rocket::serde::json::Json;
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS};

#[get("/")]
fn index() -> Json<Vec<serde_json::Value>> {
    let v = redis_client::get_all_features().unwrap_or(vec![]);
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

    let server = rocket::build().mount("/", routes![index]);
    // Start Rocket server in a separate thread
    let rocket_handle = thread::spawn(move || {
        rocket::tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async {
                let _ = server.launch().await;
            });
    });

    // Start MQTT event loop in a separate thread and end it when the shutdown signal is received
    let mqtt_handle = {
        let shutdown = shutdown.clone();
        thread::spawn(move || {
            rocket::tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async {
                    while !shutdown.load(Ordering::SeqCst) {
                        if let Ok(notification) = eventloop.poll().await {
                            match notification {
                                Event::Incoming(i) => match i {
                                    Incoming::Publish(p) => {
                                        let bytes = p.payload.to_vec();
                                        let payload = String::from_utf8(bytes).unwrap();
                                        let msg: serde_json::Value =
                                            serde_json::from_str(&payload).unwrap();
                                        message_handler::read_message(msg);
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

    while !shutdown.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(100));
    }

    mqtt_handle.join().unwrap();
    rocket_handle.join().unwrap();
}
