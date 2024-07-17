use redis::Commands;

fn get_redis() -> Result<redis::Connection, String> {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let con = client.get_connection().unwrap();

    Ok(con)
}

pub fn get_all_features() -> Result<Vec<serde_json::Value>, String> {
    let con = get_redis();
    let keys: Vec<String> = con.unwrap().smembers("AllFeatures").unwrap_or(vec![]);
    let mut res: Vec<serde_json::Value> = vec![];
    for i in keys {
        let temp_con = get_redis();
        let value: String = temp_con.unwrap().get(i.as_str()).unwrap();
        res.push(serde_json::from_str(&value).unwrap());
    }
    Ok(res)
}

pub fn add_key(id: String, value: String) {
    let con = get_redis();
    let _: () = con.unwrap().set(&id, value).unwrap();
    let con2 = get_redis();
    let _: () = con2.unwrap().sadd("AllFeatures", &id).unwrap();
}

pub fn delete_key(id: String) {
    let con = get_redis();
    let _: () = con.unwrap().del(id).unwrap();
}
