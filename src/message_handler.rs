use crate::redis_client::{add_key, delete_key};

#[derive(PartialEq, Debug)]
enum MessageType {
    Create,
    Edit,
    Delete,
}

pub async fn read_message(
    con: redis::aio::MultiplexedConnection,
    message: serde_json::Value,
) -> bool {
    let msg_type = get_type(&message).await;
    if msg_type.is_err() {
        return false;
    }
    let msg_type = msg_type.unwrap();

    let data = get_data(&message);
    if data.is_err() {
        return false;
    }
    let data = data.unwrap();

    // println!("Data Type: {:?}", msg_type);
    // println!("Data: {:?}", data);

    handle_message(con.clone(), msg_type, data).await;
    true
}

async fn handle_message(
    con: redis::aio::MultiplexedConnection,
    msg_type: MessageType,
    data: serde_json::Value,
) {
    if msg_type == MessageType::Delete {
        delete_key(con.clone(), data.to_string().trim_matches('"').to_string()).await;
        return;
    }
    let id = data["feature"]["id"].as_str().unwrap().to_string();
    let data = data.to_string();
    match msg_type {
        MessageType::Create => add_key(con.clone(), id, data).await,
        MessageType::Edit => add_key(con.clone(), id, data).await,
        _ => {}
    }
}

async fn get_type(message: &serde_json::Value) -> Result<MessageType, String> {
    let msg_type = message["type"].as_str();
    if msg_type.is_none() {
        return Err("Message type not found".to_string());
    }
    match msg_type.unwrap() {
        "Create" => Ok(MessageType::Create),
        "Edit" => Ok(MessageType::Edit),
        "Delete" => Ok(MessageType::Delete),
        _ => Err("Invalid message type".to_string()),
    }
}

fn get_data(message: &serde_json::Value) -> Result<serde_json::Value, String> {
    let data = message["data"].clone();
    if data.is_null() {
        return Err("Data not found".to_string());
    }
    Ok(data)
}
