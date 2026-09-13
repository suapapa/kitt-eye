//! USB Serial CLI parser and command definitions for K.I.T.T.-EYE.
//!
//! Pure `no_std` module with zero HAL dependencies, fully unit-testable on the host.

use heapless::String;
use crate::state::AgentState;
use crate::theme::MotionTheme;

/// Parsed MQTT broker connection information from URL or host[:port].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MqttBrokerUrl<'a> {
    pub host: &'a str,
    pub port: Option<u16>,
    pub user: Option<&'a str>,
    pub pass: Option<&'a str>,
}

/// Parsed CLI command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliCommand<'a> {
    /// Show help menu.
    Help,
    /// Query current status (Wi-Fi, MQTT, State, Theme).
    Status,
    /// Query current active state.
    GetState,
    /// Force state directly to one of `AgentState` variants (enables manual override).
    SetState(AgentState),
    /// Release manual override and return to MQTT event tracking.
    StateAuto,
    /// Change motion theme (`classic` or `colorful`) immediately.
    SetTheme(MotionTheme),
    /// Set Wi-Fi credentials (`ssid`, `pass`).
    SetWifi { ssid: &'a str, pass: &'a str },
    /// Set MQTT broker URL or host with optional port and auth.
    SetMqttBroker(MqttBrokerUrl<'a>),
    /// Set MQTT topic prefix.
    SetMqttTopic(&'a str),
    /// Set MQTT client ID.
    SetMqttClientId(&'a str),
    /// Query current configuration.
    GetConfig,
    /// Save configuration to persistent storage.
    Save,
    /// Reload configuration from persistent storage.
    Load,
    /// Reset configuration to compile-time defaults.
    ResetConfig,
    /// Trigger software reboot.
    Reboot,
}

/// Parse MQTT broker argument in URL or host[:port] format.
pub fn parse_mqtt_broker_arg<'a>(
    arg: &'a str,
    explicit_port: Option<&'a str>,
) -> Result<MqttBrokerUrl<'a>, &'static str> {
    let mut s = arg.trim();
    if s.is_empty() {
        return Err("empty broker address");
    }

    // Strip scheme if present (mqtt://, tcp://)
    if s.len() >= 7 && (eq_ignore_ascii(&s[..7], "mqtt://") || eq_ignore_ascii(&s[..7], "tcp://")) {
        s = &s[7..];
    }

    let mut user = None;
    let mut pass = None;

    // Check for user:pass@
    let host_port_part = if let Some((auth, remainder)) = s.split_once('@') {
        if let Some((u, p)) = auth.split_once(':') {
            user = Some(u);
            pass = Some(p);
        } else {
            user = Some(auth);
        }
        remainder
    } else {
        s
    };

    let mut host = host_port_part;
    let mut port = None;

    if let Some((h, p_str)) = host_port_part.rsplit_once(':') {
        if let Ok(p) = p_str.parse::<u16>() {
            host = h;
            port = Some(p);
        }
    }

    if let Some(exp_p_str) = explicit_port {
        let p = exp_p_str.parse::<u16>().map_err(|_| "invalid port number")?;
        port = Some(p);
    }

    if host.is_empty() {
        return Err("missing broker host");
    }

    Ok(MqttBrokerUrl {
        host,
        port,
        user,
        pass,
    })
}

/// Parse a single line of input into a `CliCommand`.
///
/// Leading/trailing whitespace is trimmed. Arguments separated by one or more spaces.
pub fn parse_line(input: &str) -> Result<CliCommand<'_>, &'static str> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("empty command");
    }

    let mut tokens = trimmed.split_whitespace();
    let first = tokens.next().ok_or("empty command")?;

    if eq_ignore_ascii(first, "help") || first == "?" {
        Ok(CliCommand::Help)
    } else if eq_ignore_ascii(first, "status") {
        Ok(CliCommand::Status)
    } else if eq_ignore_ascii(first, "config") {
        match tokens.next() {
            None | Some("show") => Ok(CliCommand::GetConfig),
            Some(_) => Err("usage: config [show]"),
        }
    } else if eq_ignore_ascii(first, "save") {
        Ok(CliCommand::Save)
    } else if eq_ignore_ascii(first, "load") {
        Ok(CliCommand::Load)
    } else if eq_ignore_ascii(first, "reset-config") || eq_ignore_ascii(first, "reset") {
        Ok(CliCommand::ResetConfig)
    } else if eq_ignore_ascii(first, "reboot") || eq_ignore_ascii(first, "restart") {
        Ok(CliCommand::Reboot)
    } else if eq_ignore_ascii(first, "theme") {
        let name = tokens.next().ok_or("usage: theme <classic|colorful>")?;
        let theme = parse_theme_arg(name)?;
        Ok(CliCommand::SetTheme(theme))
    } else if eq_ignore_ascii(first, "wifi") {
        let ssid = tokens.next().ok_or("usage: wifi <ssid> <pass>")?;
        let pass = tokens.next().unwrap_or("");
        Ok(CliCommand::SetWifi { ssid, pass })
    } else if eq_ignore_ascii(first, "mqtt_broker") || eq_ignore_ascii(first, "mqtt-broker") || eq_ignore_ascii(first, "broker") {
        let arg = tokens.next().ok_or("usage: mqtt_broker <host|url> [port]")?;
        let exp_port = tokens.next();
        let broker_url = parse_mqtt_broker_arg(arg, exp_port)?;
        Ok(CliCommand::SetMqttBroker(broker_url))
    } else if eq_ignore_ascii(first, "mqtt_topic") || eq_ignore_ascii(first, "mqtt-topic") {
        let prefix = tokens.next().ok_or("usage: mqtt_topic <topic_prefix>")?;
        Ok(CliCommand::SetMqttTopic(prefix))
    } else if eq_ignore_ascii(first, "mqtt_client_id") || eq_ignore_ascii(first, "mqtt-client_id") {
        let client_id = tokens.next().ok_or("usage: mqtt-client_id <client_id>")?;
        Ok(CliCommand::SetMqttClientId(client_id))
    } else if eq_ignore_ascii(first, "state") {
        let arg = match tokens.next() {
            Some(a) => a,
            None => return Ok(CliCommand::GetState),
        };
        if eq_ignore_ascii(arg, "auto") {
            Ok(CliCommand::StateAuto)
        } else if let Some(state) = parse_state_arg(arg) {
            Ok(CliCommand::SetState(state))
        } else {
            Err("invalid state (valid: idle, thinking, generating, executing_tool, waiting_input, done, error, auto)")
        }
    } else if eq_ignore_ascii(first, "set") {
        let sub = tokens.next().ok_or("usage: set <wifi|mqtt_broker|mqtt_topic|mqtt-client_id|theme> ...")?;
        if eq_ignore_ascii(sub, "theme") {
            let name = tokens.next().ok_or("usage: set theme <classic|colorful>")?;
            let theme = parse_theme_arg(name)?;
            Ok(CliCommand::SetTheme(theme))
        } else if eq_ignore_ascii(sub, "wifi") {
            let ssid = tokens.next().ok_or("usage: set wifi <ssid> <pass>")?;
            let pass = tokens.next().unwrap_or("");
            Ok(CliCommand::SetWifi { ssid, pass })
        } else if eq_ignore_ascii(sub, "mqtt_broker") || eq_ignore_ascii(sub, "mqtt-broker") || eq_ignore_ascii(sub, "broker") {
            let arg = tokens.next().ok_or("usage: set mqtt_broker <host|url> [port]")?;
            let exp_port = tokens.next();
            let broker_url = parse_mqtt_broker_arg(arg, exp_port)?;
            Ok(CliCommand::SetMqttBroker(broker_url))
        } else if eq_ignore_ascii(sub, "mqtt_topic") || eq_ignore_ascii(sub, "mqtt-topic") {
            let prefix = tokens.next().ok_or("usage: set mqtt_topic <topic_prefix>")?;
            Ok(CliCommand::SetMqttTopic(prefix))
        } else if eq_ignore_ascii(sub, "mqtt_client_id") || eq_ignore_ascii(sub, "mqtt-client_id") {
            let client_id = tokens.next().ok_or("usage: set mqtt-client_id <client_id>")?;
            Ok(CliCommand::SetMqttClientId(client_id))
        } else if eq_ignore_ascii(sub, "mqtt") {
            let target = tokens.next().ok_or("usage: set mqtt <broker|topic|client_id> ...")?;
            if eq_ignore_ascii(target, "broker") {
                let arg = tokens.next().ok_or("usage: set mqtt broker <host|url> [port]")?;
                let exp_port = tokens.next();
                let broker_url = parse_mqtt_broker_arg(arg, exp_port)?;
                Ok(CliCommand::SetMqttBroker(broker_url))
            } else if eq_ignore_ascii(target, "topic") || eq_ignore_ascii(target, "prefix") {
                let prefix = tokens.next().ok_or("usage: set mqtt topic <topic_prefix>")?;
                Ok(CliCommand::SetMqttTopic(prefix))
            } else if eq_ignore_ascii(target, "client_id") || eq_ignore_ascii(target, "client-id") {
                let client_id = tokens.next().ok_or("usage: set mqtt client_id <client_id>")?;
                Ok(CliCommand::SetMqttClientId(client_id))
            } else {
                Err("unknown target (valid: broker, topic, client_id)")
            }
        } else {
            Err("unknown set target (valid: wifi, mqtt_broker, mqtt_topic, mqtt-client_id, theme)")
        }
    } else {
        Err("unknown command. Type 'help' or '?' for available commands.")
    }
}

fn parse_theme_arg(arg: &str) -> Result<MotionTheme, &'static str> {
    if eq_ignore_ascii(arg, "classic") || eq_ignore_ascii(arg, "red") {
        Ok(MotionTheme::Classic)
    } else if eq_ignore_ascii(arg, "colorful") || eq_ignore_ascii(arg, "color") || eq_ignore_ascii(arg, "rgb") {
        Ok(MotionTheme::Colorful)
    } else {
        Err("invalid theme (valid: classic, colorful)")
    }
}

fn parse_state_arg(arg: &str) -> Option<AgentState> {
    if eq_ignore_ascii(arg, "idle") {
        Some(AgentState::Idle)
    } else if eq_ignore_ascii(arg, "thinking") {
        Some(AgentState::Thinking)
    } else if eq_ignore_ascii(arg, "generating") {
        Some(AgentState::Generating)
    } else if eq_ignore_ascii(arg, "executing_tool") || eq_ignore_ascii(arg, "tool") {
        Some(AgentState::ExecutingTool)
    } else if eq_ignore_ascii(arg, "waiting_input") || eq_ignore_ascii(arg, "input") {
        Some(AgentState::WaitingInput)
    } else if eq_ignore_ascii(arg, "done") {
        Some(AgentState::Done)
    } else if eq_ignore_ascii(arg, "error") {
        Some(AgentState::Error)
    } else {
        None
    }
}

fn eq_ignore_ascii(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .all(|(x, y)| x.to_ascii_lowercase() == y.to_ascii_lowercase())
}

/// Format the help string into a buffer.
pub fn write_help<const N: usize>(buf: &mut String<N>) {
    let help_text = 
"--- K.I.T.T.-EYE USB Serial Commands ---
  state                     - Query current state & mode
  state <NAME>              - Set state directly (override on)
                              [idle|thinking|generating|executing_tool|waiting_input|done|error]
  state auto                - Release override, resume MQTT
  theme <classic|colorful>  - Set LED motion theme
  wifi <ssid> <pass>        - Set Wi-Fi credentials
  mqtt_broker <url|host>    - Set MQTT broker:
                              e.g. mqtt://user:pass@172.16.0.100:1883
                              e.g. 192.168.1.50 1883
  mqtt_topic <prefix>       - Set MQTT topic prefix
  mqtt-client_id <id>       - Set MQTT client ID
  status                    - Show system status
  config                    - Show current configuration
  save                      - Save configuration to storage
  load                      - Reload configuration from storage
  reset                     - Reset configuration to defaults
  reboot                    - Reboot MCU to apply network settings
  help, ?                   - Show this help
";
    let _ = buf.push_str(help_text);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_help_and_status() {
        assert_eq!(parse_line("help"), Ok(CliCommand::Help));
        assert_eq!(parse_line("  ?  "), Ok(CliCommand::Help));
        assert_eq!(parse_line("status"), Ok(CliCommand::Status));
        assert_eq!(parse_line("STATUS"), Ok(CliCommand::Status));
    }

    #[test]
    fn test_state_commands() {
        assert_eq!(parse_line("state"), Ok(CliCommand::GetState));
        assert_eq!(parse_line("state auto"), Ok(CliCommand::StateAuto));
        assert_eq!(parse_line("state AUTO"), Ok(CliCommand::StateAuto));
        assert_eq!(parse_line("state thinking"), Ok(CliCommand::SetState(AgentState::Thinking)));
        assert_eq!(parse_line("state THINKING"), Ok(CliCommand::SetState(AgentState::Thinking)));
        assert_eq!(parse_line("state executing_tool"), Ok(CliCommand::SetState(AgentState::ExecutingTool)));
        assert_eq!(parse_line("state tool"), Ok(CliCommand::SetState(AgentState::ExecutingTool)));
        assert_eq!(parse_line("state waiting_input"), Ok(CliCommand::SetState(AgentState::WaitingInput)));
        assert_eq!(parse_line("state input"), Ok(CliCommand::SetState(AgentState::WaitingInput)));
        assert_eq!(parse_line("state done"), Ok(CliCommand::SetState(AgentState::Done)));
        assert_eq!(parse_line("state error"), Ok(CliCommand::SetState(AgentState::Error)));
        assert_eq!(parse_line("state idle"), Ok(CliCommand::SetState(AgentState::Idle)));
        assert!(parse_line("state unknown").is_err());
    }

    #[test]
    fn test_theme_commands() {
        assert_eq!(parse_line("theme classic"), Ok(CliCommand::SetTheme(MotionTheme::Classic)));
        assert_eq!(parse_line("theme colorful"), Ok(CliCommand::SetTheme(MotionTheme::Colorful)));
        assert_eq!(parse_line("set theme colorful"), Ok(CliCommand::SetTheme(MotionTheme::Colorful)));
        assert_eq!(parse_line("set theme red"), Ok(CliCommand::SetTheme(MotionTheme::Classic)));
        assert!(parse_line("theme foo").is_err());
    }

    #[test]
    fn test_config_commands() {
        assert_eq!(parse_line("wifi MySSID MyPass"), Ok(CliCommand::SetWifi { ssid: "MySSID", pass: "MyPass" }));
        assert_eq!(parse_line("set wifi MySSID MyPass"), Ok(CliCommand::SetWifi { ssid: "MySSID", pass: "MyPass" }));

        // URL formats
        assert_eq!(
            parse_line("mqtt_broker mqtt://mqtt:896351@172.16.0.100"),
            Ok(CliCommand::SetMqttBroker(MqttBrokerUrl {
                host: "172.16.0.100",
                port: None,
                user: Some("mqtt"),
                pass: Some("896351"),
            }))
        );
        assert_eq!(
            parse_line("mqtt_broker mqtt://user:pass@172.16.0.100:1884"),
            Ok(CliCommand::SetMqttBroker(MqttBrokerUrl {
                host: "172.16.0.100",
                port: Some(1884),
                user: Some("user"),
                pass: Some("pass"),
            }))
        );
        assert_eq!(
            parse_line("mqtt_broker 192.168.1.50 1883"),
            Ok(CliCommand::SetMqttBroker(MqttBrokerUrl {
                host: "192.168.1.50",
                port: Some(1883),
                user: None,
                pass: None,
            }))
        );

        // Topic and Client ID
        assert_eq!(parse_line("mqtt_topic my-topic"), Ok(CliCommand::SetMqttTopic("my-topic")));
        assert_eq!(parse_line("mqtt-topic my-topic"), Ok(CliCommand::SetMqttTopic("my-topic")));
        assert_eq!(parse_line("mqtt-client_id my-id"), Ok(CliCommand::SetMqttClientId("my-id")));
        assert_eq!(parse_line("mqtt_client_id my-id"), Ok(CliCommand::SetMqttClientId("my-id")));

        assert_eq!(parse_line("config"), Ok(CliCommand::GetConfig));
        assert_eq!(parse_line("config show"), Ok(CliCommand::GetConfig));
        assert_eq!(parse_line("save"), Ok(CliCommand::Save));
        assert_eq!(parse_line("load"), Ok(CliCommand::Load));
        assert_eq!(parse_line("reset"), Ok(CliCommand::ResetConfig));
        assert_eq!(parse_line("reset-config"), Ok(CliCommand::ResetConfig));
        assert_eq!(parse_line("reboot"), Ok(CliCommand::Reboot));
    }

    #[test]
    fn test_empty_and_unknown() {
        assert!(parse_line("").is_err());
        assert!(parse_line("   ").is_err());
        assert!(parse_line("nonexistent_command").is_err());
    }
}
