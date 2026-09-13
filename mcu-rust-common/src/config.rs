//! Compile-time configuration from `.env` / environment (via `build.rs`).

/// Wi-Fi SSID.
pub const SSID: &str = env!("SSID");
/// Wi-Fi password.
pub const PASS: &str = env!("PASS");

/// Motion theme name (`classic` = all-red state→motion map, `colorful` = vivid color-coded theme).
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

use heapless::String;
use crate::theme::MotionTheme;

/// Dynamic runtime configuration that can be updated via CLI or Flash.
#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    pub ssid: String<32>,
    pub pass: String<64>,
    pub motion_theme: MotionTheme,
    pub mqtt_broker: String<64>,
    pub mqtt_port: u16,
    pub mqtt_user: String<32>,
    pub mqtt_pass: String<64>,
    pub mqtt_topic_prefix: String<32>,
    pub mqtt_client_id: String<32>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self::from_env_defaults()
    }
}

impl RuntimeConfig {
    /// Initialize configuration with compile-time defaults.
    pub fn from_env_defaults() -> Self {
        let mut ssid = String::new();
        let _ = ssid.push_str(SSID);

        let mut pass = String::new();
        let _ = pass.push_str(PASS);

        let mut mqtt_broker = String::new();
        let _ = mqtt_broker.push_str(MQTT_BROKER);

        let mut mqtt_user = String::new();
        let _ = mqtt_user.push_str(MQTT_USER);

        let mut mqtt_pass = String::new();
        let _ = mqtt_pass.push_str(MQTT_PASS);

        let mut mqtt_topic_prefix = String::new();
        let _ = mqtt_topic_prefix.push_str(MQTT_TOPIC_PREFIX);

        let mut mqtt_client_id = String::new();
        let _ = mqtt_client_id.push_str(MQTT_CLIENT_ID);

        Self {
            ssid,
            pass,
            motion_theme: MotionTheme::from_name(MOTION_THEME),
            mqtt_broker,
            mqtt_port: mqtt_port(),
            mqtt_user,
            mqtt_pass,
            mqtt_topic_prefix,
            mqtt_client_id,
        }
    }

    /// Serialize configuration to a 512-byte buffer with Magic header and checksum.
    pub fn serialize(&self, buf: &mut [u8; 512]) {
        buf.fill(0);
        buf[0..4].copy_from_slice(b"KITT");
        buf[4] = 1; // Version
        buf[5] = if self.motion_theme == MotionTheme::Colorful { 1 } else { 0 };
        buf[6..8].copy_from_slice(&self.mqtt_port.to_le_bytes());

        let mut pos = 8;

        macro_rules! write_str {
            ($s:expr, $max_len:expr) => {
                let bytes = $s.as_bytes();
                let len = bytes.len().min($max_len) as u8;
                buf[pos] = len;
                pos += 1;
                buf[pos..pos + len as usize].copy_from_slice(&bytes[..len as usize]);
                pos += $max_len;
            };
        }

        write_str!(self.ssid, 32);
        write_str!(self.pass, 64);
        write_str!(self.mqtt_broker, 64);
        write_str!(self.mqtt_user, 32);
        write_str!(self.mqtt_pass, 64);
        write_str!(self.mqtt_topic_prefix, 32);
        write_str!(self.mqtt_client_id, 32);

        // Compute checksum over 0..pos
        let mut sum = 0u32;
        for &b in &buf[..pos] {
            sum = sum.wrapping_mul(31).wrapping_add(b as u32);
        }
        buf[pos..pos + 4].copy_from_slice(&sum.to_le_bytes());
    }

    /// Deserialize configuration from a buffer if Magic and Checksum are valid.
    pub fn deserialize(buf: &[u8]) -> Option<Self> {
        if buf.len() < 340 || &buf[0..4] != b"KITT" || buf[4] != 1 {
            return None;
        }

        let theme = if buf[5] == 1 { MotionTheme::Colorful } else { MotionTheme::Classic };
        let port = u16::from_le_bytes([buf[6], buf[7]]);

        let mut pos = 8;
        macro_rules! read_str {
            ($max_len:expr) => {{
                let len = buf[pos] as usize;
                pos += 1;
                if len > $max_len {
                    return None;
                }
                let s_bytes = &buf[pos..pos + len];
                pos += $max_len;
                let s = core::str::from_utf8(s_bytes).ok()?;
                let mut hs = String::new();
                let _ = hs.push_str(s);
                hs
            }};
        }

        let ssid = read_str!(32);
        let pass = read_str!(64);
        let mqtt_broker = read_str!(64);
        let mqtt_user = read_str!(32);
        let mqtt_pass = read_str!(64);
        let mqtt_topic_prefix = read_str!(32);
        let mqtt_client_id = read_str!(32);

        let mut sum = 0u32;
        for &b in &buf[..pos] {
            sum = sum.wrapping_mul(31).wrapping_add(b as u32);
        }
        let stored_sum = u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]);
        if sum != stored_sum {
            return None;
        }

        Some(Self {
            ssid,
            pass,
            motion_theme: theme,
            mqtt_broker,
            mqtt_port: port,
            mqtt_user,
            mqtt_pass,
            mqtt_topic_prefix,
            mqtt_client_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialization_roundtrip() {
        let mut cfg = RuntimeConfig::from_env_defaults();
        cfg.ssid.clear();
        let _ = cfg.ssid.push_str("CustomSSID");
        cfg.pass.clear();
        let _ = cfg.pass.push_str("SecretPass123");
        cfg.motion_theme = MotionTheme::Colorful;
        cfg.mqtt_port = 8883;

        let mut buf = [0u8; 512];
        cfg.serialize(&mut buf);

        let loaded = RuntimeConfig::deserialize(&buf).expect("deserialize failed");
        assert_eq!(loaded.ssid.as_str(), "CustomSSID");
        assert_eq!(loaded.pass.as_str(), "SecretPass123");
        assert_eq!(loaded.motion_theme, MotionTheme::Colorful);
        assert_eq!(loaded.mqtt_port, 8883);
        assert_eq!(loaded.mqtt_broker.as_str(), cfg.mqtt_broker.as_str());
    }

    #[test]
    fn test_corrupted_buffer() {
        let mut buf = [0u8; 512];
        assert!(RuntimeConfig::deserialize(&buf).is_none());

        let cfg = RuntimeConfig::from_env_defaults();
        cfg.serialize(&mut buf);
        // corrupt checksum
        buf[336] ^= 0xFF;
        assert!(RuntimeConfig::deserialize(&buf).is_none());
    }
}
