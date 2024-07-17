use crate::redis_client::{add_key, delete_key};

#[derive(PartialEq, Debug)]
enum MessageType {
    Create,
    Edit,
    Delete,
}

pub fn read_message(message: serde_json::Value) -> bool {
    let msg_type = get_type(&message);
    if msg_type.is_err() {
        return false;
    }
    let msg_type = msg_type.unwrap();

    let data = get_data(&message);
    if data.is_err() {
        return false;
    }
    let data = data.unwrap();

    println!("Data Type: {:?}", msg_type);
    println!("Data: {:?}", data);

    handle_message(msg_type, data);
    true
}

fn handle_message(msg_type: MessageType, data: serde_json::Value) {
    if msg_type == MessageType::Delete {
        delete_key(data.to_string());
        return;
    }
    let id = data["feature"]["id"].as_str().unwrap().to_string();
    let data = data.to_string();
    match msg_type {
        MessageType::Create => add_key(id, data),
        MessageType::Edit => add_key(id, data),
        _ => {}
    }
}

fn get_type(message: &serde_json::Value) -> Result<MessageType, String> {
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
