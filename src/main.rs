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
use std::sync::Arc;
use std::time::Duration;

use drone_emulator::{create_fake_data, DroneData};
use redis_client::StreamObj;
use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::Header;
use rocket::serde::json::Json;
use rocket::{Request, Response};
use rumqttc::{AsyncClient, Event, EventLoop, Incoming, MqttOptions, QoS};
use tokio::time::timeout;
// use dotenv::dotenv;

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
    let connection = redis_client::get_redis().await.unwrap();
    let v = redis_client::get_all_features(connection)
        .await
        .unwrap_or(vec![]);
    Json(v)
}

#[get("/stream")]
async fn get_stream_route() -> Json<Vec<StreamObj>> {
    let connection = redis_client::get_redis().await.unwrap();
    let v = redis_client::get_stream(connection, "drone_data").await;
    Json(v)
}

#[rocket::main]
async fn main() {
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_signal = shutdown.clone();

    let connection = redis_client::get_redis().await.unwrap();

    // Set up Ctrl+C handler
    ctrlc::set_handler(move || {
        println!("Shutting down...");
        shutdown_signal.store(true, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl+C handler");

    // MQTT setup
    let mut mqttoptions = MqttOptions::new(
        "rumqtt-async",
        std::env::var("MQTT_BROKER_HOST").unwrap_or("rabbitmq".to_owned()),
        std::env::var("MQTT_BROKER_PORT")
            .unwrap_or("1883".to_owned())
            .parse::<u16>()
            .unwrap_or(1883),
    );
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let (mut client, mut eventloop) = AsyncClient::new(mqttoptions, 10);
    client.subscribe("presence", QoS::AtMostOnce).await.unwrap();

    let mut attempts = 10;
    while attempts > 0 {
        if check_mqtt_connection(&mut client, &mut eventloop).await {
            println!("Connection is healthy");
            break;
        } else {
            println!("Connection check failed");
            attempts -= 1;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
    if attempts == 0 {
        panic!("Failed to connect to MQTT broker");
    }

    let figment = rocket::Config::figment().merge(("port", 3000));

    let server = rocket::custom(figment)
        .attach(CORS)
        .mount("/", routes![index, get_stream_route]);
    // Start Rocket server in a separate thread
    let rocket_handle = tokio::spawn(async move {
        println!("Starting Rocket server");
        let _ = server.launch().await;
    });

    // Start MQTT event loop in a separate thread and end it when the shutdown signal is received
    let mqtt_client_handle = {
        let shutdown = shutdown.clone();
        let connection = connection.clone();
        tokio::spawn(async move {
            println!("Starting MQTT client");

            while !shutdown.load(Ordering::SeqCst) {
                if let Ok(notification) = eventloop.poll().await {
                    match notification {
                        Event::Incoming(i) => match i {
                            Incoming::Publish(p) => {
                                // println!("Handling Input");
                                let bytes = p.payload.to_vec();
                                let payload = String::from_utf8(bytes).unwrap_or("".to_owned());

                                match serde_json::from_str::<serde_json::Value>(&payload) {
                                    Ok(msg) => {
                                        message_handler::read_message(connection.clone(), msg)
                                            .await;
                                    }
                                    Err(e) => {
                                        println!("Error reading message: {:?}", e);
                                    }
                                }
                            }
                            _ => {}
                        },
                        _ => {}
                    }
                }
            }
        })
    };

    let mqtt_publisher_handler = {
        let shutdown = shutdown.clone();
        let connection = connection.clone();
        tokio::spawn(async move {
            println!("Starting drone emulator");
            // let shutdown = shutdown.clone();
            let mut previous_state: Option<DroneData> = None;
            while !shutdown.load(Ordering::SeqCst) {
                // println!("Publishing data");
                let data = create_fake_data(connection.clone(), &previous_state).await;
                // println!("Data Complete");
                previous_state = Some(data.clone());
                let payload: String = serde_json::to_string(&data).unwrap_or("".to_owned());
                // println!("Publishing to topic");
                let client = client.clone();
                match client
                    .publish("topic/drone", QoS::AtLeastOnce, false, payload)
                    .await
                {
                    Ok(_) => {}
                    Err(e) => {
                        println!("Error publishing data: {:?}", e);
                    }
                }
                // println!("Data published, sleeping for 1 second");
                std::thread::sleep(Duration::from_secs(1));
            }
        })
    };

    while !shutdown.load(Ordering::SeqCst) {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    mqtt_publisher_handler.await.unwrap();
    mqtt_client_handle.await.unwrap();
    rocket_handle.await.unwrap();
}

async fn check_mqtt_connection(client: &mut AsyncClient, eventloop: &mut EventLoop) -> bool {
    let topic = "topic/healthcheck";
    let payload = "ping";

    match timeout(
        Duration::from_secs(5),
        client.publish(topic, QoS::AtLeastOnce, false, payload),
    )
    .await
    {
        Ok(Ok(_)) => {
            println!("Ping message published, waiting for pong...");
            loop {
                match timeout(Duration::from_secs(5), eventloop.poll()).await {
                    Ok(Ok(Event::Incoming(_))) => {
                        println!("Received pong from broker");
                        return true;
                    }
                    Ok(Ok(_)) => {
                        continue;
                    }
                    Ok(Err(e)) => {
                        println!("Error receiving pong: {:?}", e);
                        return false;
                    }
                    Err(_) => {
                        println!("Timed out waiting for pong");
                        return false;
                    }
                }
            }
        }
        Ok(Err(e)) => {
            println!("Error publishing ping message: {:?}", e);
            return false;
        }
        Err(_) => {
            println!("Timed out publishing ping message");
            return false;
        }
    }
}
