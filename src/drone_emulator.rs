use serde::{Deserialize, Serialize};
use std::time::UNIX_EPOCH;

use crate::redis_client::{append_stream, reset_stream};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DroneData {
    tid: String,
    bid: String,
    timestamp: i32, // Change to Timestamp Type
    data: DroneStateData,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct DroneStateData {
    // rainfall: f64,
    // wind_speed: f64,
    // environment_temperature: f64,
    // temperature: f64,
    latitude: f64,
    longitude: f64,
    rotation: f64,
}

pub async fn create_fake_data(
    con: redis::aio::MultiplexedConnection,
    previous_state: &Option<DroneData>,
) -> DroneData {
    // println!("Creating Fake Data");
    if previous_state.is_none() {
        reset_stream(con.clone(), "drone_data").await;
        return DroneData {
            tid: "TID".to_string(),
            bid: "BID".to_string(),
            timestamp: 0,
            data: DroneStateData {
                // rainfall: 0.0,
                // wind_speed: 0.0,
                // environment_temperature: 0.0,
                // temperature: 0.0,
                latitude: 31.9544,
                longitude: 35.9106,
                rotation: 0.0,
            },
        };
    }

    let mut old = previous_state.clone().unwrap();

    let rng_x = rand::random::<f32>();
    let rng_y = rand::random::<f32>();
    // let y: f64 = rng.gen();
    // let direction = if y > 0.5 { 1.0 } else { -1.0 };

    let delta_x = rng_x * 0.0001;
    let delta_y = rng_y * 0.0001;
    let ration: f32 = delta_y / delta_x;
    let rotation = ration.atan();

    old.timestamp = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    old.data.latitude += delta_y as f64;
    old.data.longitude += delta_x as f64;
    old.data.rotation = rotation as f64;

    // println!("Appending to Stream");
    append_stream(
        con.clone(),
        "drone_data".to_owned(),
        (old.data.latitude, old.data.longitude),
    )
    .await;
    // println!("Appended to Stream, returning data");
    return old;
}
