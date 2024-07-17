# Basic MQTT + redis middleware with rocket.rs

This is a simple example of how to use the [Rocket.rs](https://rocket.rs/) web framework with the [mqtt](https://crates.io/crates/rumqttc) and [redis](https://crates.io/crates/redis) crates to create a simple middleware that listens to a MQTT topic and stores the messages in a Redis database.

Note that we are extensivly using serde_json::Value to read the payloads This is unsafe for production and should be replaced with properly structured data types

In it's current state, the code only handles messages of a certain format, and will panic if the message is not in the expected format. Future versions should include a dynamic way to handle different message formats

Also the current version isn't optimized for performance, as such you will see some obvious bottlenecks (Such as getting the redis client for every message, instead of a connection pool)
