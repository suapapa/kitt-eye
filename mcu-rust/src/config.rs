//! Compile-time configuration from `.env` / environment (via `build.rs`).

/// Wi-Fi SSID.
pub const SSID: &str = env!("SSID");
/// Wi-Fi password.
pub const PASS: &str = env!("PASS");

/// Motion theme name (`classic` = all-red state→motion map).
pub const MOTION_THEME: &str = env!("MOTION_THEME");

/// MQTT broker hostname or IPv4 literal.
pub const MQTT_BROKER: &str = env!("MQTT_BROKER");
/// MQTT TCP port (decimal string, parsed at runtime).
pub const MQTT_PORT: &str = env!("MQTT_PORT");
/// Optional MQTT username (empty = anonymous).
pub const MQTT_USER: &str = env!("MQTT_USER");
/// Optional MQTT password.
pub const MQTT_PASS: &str = env!("MQTT_PASS");
/// Topic prefix (active state topic = `{prefix}/active_state`).
pub const MQTT_TOPIC_PREFIX: &str = env!("MQTT_TOPIC_PREFIX");
/// MQTT client identifier.
pub const MQTT_CLIENT_ID: &str = env!("MQTT_CLIENT_ID");

/// Parse `MQTT_PORT` (defaults to 1883 on bad input).
pub fn mqtt_port() -> u16 {
    MQTT_PORT.parse().unwrap_or(1883)
}
