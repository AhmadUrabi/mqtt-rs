use redis::{AsyncCommands, RedisResult};
use serde::{Deserialize, Serialize};

pub async fn get_redis() -> Result<redis::aio::MultiplexedConnection, String> {
    let client = redis::Client::open(
        std::env::var("REDIS_CONNECTION").unwrap_or("redis://redis:6379".to_owned()),
    )
    .unwrap();
    // let mut con = client.get_connection().await?;
    let mut attempts = 10;
    while attempts > 0 {
        match client.get_multiplexed_tokio_connection().await {
            Ok(con) => return Ok(con),
            Err(_) => {
                attempts -= 1;
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
    }
    Err("Failed to connect to Redis".to_owned())
}

pub async fn get_all_features(
    mut con: redis::aio::MultiplexedConnection,
) -> Result<Vec<serde_json::Value>, String> {
    // let mut con = get_redis().await.unwrap();
    let keys: Vec<String> = con.smembers("AllFeatures").await.unwrap_or(vec![]);
    let mut res: Vec<serde_json::Value> = vec![];
    for i in keys {
        let value: String = con.get(i.as_str()).await.unwrap();
        res.push(serde_json::from_str(&value).unwrap());
    }
    Ok(res)
}

pub async fn add_key(mut con: redis::aio::MultiplexedConnection, id: String, value: String) {
    // let mut con = get_redis().await.unwrap();
    let _: () = con.set(&id, value).await.unwrap();
    let _: () = con.sadd("AllFeatures", &id).await.unwrap();
}

pub async fn delete_key(mut con: redis::aio::MultiplexedConnection, id: String) {
    println!("Deleting key: {}", id);
    // let mut con = get_redis().await.unwrap();
    match con.del::<String, ()>(id.clone()).await {
        Ok(_) => {}
        Err(_) => {
            println!("Key not found: {}", id);
            return;
        }
    }
    let _: () = con.srem("AllFeatures", id.clone()).await.unwrap();
}

pub async fn append_stream(
    mut con: redis::aio::MultiplexedConnection,
    id: String,
    value: (f64, f64),
) {
    // let mut con = get_redis().await.unwrap();
    // println!("Append Stream fn");
    match con
        .xadd::<String, &str, &str, f64, ()>(
            id.into(),
            "*",
            &[("Latitude", value.0), ("Longitude", value.1)],
        )
        .await
    {
        Ok(_) => println!("Successfully Appended"),
        Err(e) => println!("Error appending: {:?}", e),
    }
}

#[derive(Serialize, Deserialize)]
pub struct StreamObj {
    id: String,
    coordinates: Coordinates,
}

#[derive(Serialize, Deserialize)]
pub struct Coordinates {
    latitude: f64,
    longitude: f64,
}

type StreamData = (String, String, String, String);

pub async fn get_stream(mut con: redis::aio::MultiplexedConnection, id: &str) -> Vec<StreamObj> {
    // let mut con = get_redis().await.unwrap();
    let count = 50;
    // Get the ID of the last entry in the stream
    let last_entry: RedisResult<Vec<Vec<(String, Vec<StreamData>)>>> =
        con.xrevrange_count(id, "+", "-", 1).await;

    let last_id = match last_entry {
        Ok(entries) => entries
            .first()
            .unwrap()
            .first()
            .map(|(id, _)| id.clone())
            .unwrap_or_else(|| "0-0".to_string()),
        Err(_) => "0-0".to_string(),
    };

    // Get the last 50 entries
    let mut res: Vec<StreamObj> = vec![];
    let entries: RedisResult<Vec<Vec<(String, Vec<StreamData>)>>> =
        con.xrevrange_count(id, last_id, "-", count).await;

    match entries {
        Ok(entries) => {
            for entry in entries.iter().rev() {
                for (field, data) in entry.iter() {
                    for (_, latitude, _, longitude) in data.iter() {
                        res.push(StreamObj {
                            id: field.clone(),
                            coordinates: Coordinates {
                                latitude: latitude.parse::<f64>().unwrap(),
                                longitude: longitude.parse::<f64>().unwrap(),
                            },
                        });
                    }
                }
            }
        }
        Err(err) => {
            eprintln!("Error reading stream: {}", err);
        }
    }

    res
}

pub async fn reset_stream(mut con: redis::aio::MultiplexedConnection, id: &str) {
    // let mut con = get_redis().await.unwrap();
    let _: () = con.del(id).await.unwrap();
}
