//! Minimal MQTT 3.1.1 subscriber over an async TCP stream (subscribe + publish RX).

use embedded_io_async::{Read, Write};
use heapless::Vec;

/// Errors from the lightweight MQTT client.
#[derive(Debug)]
pub enum MqttError {
    Io,
    Protocol,
    Connack(u8),
    Buffer,
}

/// MQTT 3.1.1 client focused on a single topic subscription.
pub struct MqttClient<T> {
    transport: T,
    read_buf: [u8; 256],
    read_len: usize,
}

impl<T: Read + Write> MqttClient<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            read_buf: [0; 256],
            read_len: 0,
        }
    }

    /// CONNECT then wait for CONNACK (return code 0).
    pub async fn connect(
        &mut self,
        client_id: &str,
        username: &str,
        password: &str,
        keep_alive_secs: u16,
    ) -> Result<(), MqttError> {
        let mut flags: u8 = 0x02; // clean session
        if !username.is_empty() {
            flags |= 0x80;
            if !password.is_empty() {
                flags |= 0x40;
            }
        }

        let mut variable: Vec<u8, 192> = Vec::new();
        push_mqtt_string(&mut variable, "MQTT")?;
        variable.push(0x04).map_err(|_| MqttError::Buffer)?; // protocol level 4
        variable.push(flags).map_err(|_| MqttError::Buffer)?;
        variable
            .push((keep_alive_secs >> 8) as u8)
            .map_err(|_| MqttError::Buffer)?;
        variable
            .push((keep_alive_secs & 0xff) as u8)
            .map_err(|_| MqttError::Buffer)?;
        push_mqtt_string(&mut variable, client_id)?;
        if !username.is_empty() {
            push_mqtt_string(&mut variable, username)?;
            if !password.is_empty() {
                push_mqtt_string(&mut variable, password)?;
            }
        }

        let mut pkt: Vec<u8, 256> = Vec::new();
        encode_packet(&mut pkt, 0x10, &variable)?;
        write_all(&mut self.transport, &pkt).await?;

        let (ty, payload) = self.read_packet().await?;
        if ty != 0x20 || payload.len() < 2 {
            return Err(MqttError::Protocol);
        }
        let code = payload[1];
        if code != 0 {
            return Err(MqttError::Connack(code));
        }
        Ok(())
    }

    /// SUBSCRIBE QoS1 to `topic`, wait for SUBACK.
    pub async fn subscribe(&mut self, topic: &str, packet_id: u16) -> Result<(), MqttError> {
        let mut variable: Vec<u8, 192> = Vec::new();
        variable
            .push((packet_id >> 8) as u8)
            .map_err(|_| MqttError::Buffer)?;
        variable
            .push((packet_id & 0xff) as u8)
            .map_err(|_| MqttError::Buffer)?;
        push_mqtt_string(&mut variable, topic)?;
        variable.push(0x01).map_err(|_| MqttError::Buffer)?; // requested QoS 1

        let mut pkt: Vec<u8, 256> = Vec::new();
        encode_packet(&mut pkt, 0x82, &variable)?; // SUBSCRIBE, flags=2
        write_all(&mut self.transport, &pkt).await?;

        loop {
            let (ty, payload) = self.read_packet().await?;
            if ty == 0x90 {
                if payload.len() >= 3 && payload[2] <= 0x02 {
                    return Ok(());
                }
                return Err(MqttError::Protocol);
            }
            if (ty & 0xf0) == 0x30 {
                self.maybe_puback(ty, &payload).await?;
            }
        }
    }

    /// Block until the next PUBLISH payload is received.
    pub async fn next_publish_payload(
        &mut self,
        out: &mut [u8],
    ) -> Result<usize, MqttError> {
        loop {
            let (ty, payload) = self.read_packet().await?;
            if (ty & 0xf0) == 0x30 {
                let qos = (ty >> 1) & 0x03;
                if payload.len() < 2 {
                    return Err(MqttError::Protocol);
                }
                let topic_len = u16::from_be_bytes([payload[0], payload[1]]) as usize;
                let mut idx = 2usize;
                if payload.len() < idx + topic_len {
                    return Err(MqttError::Protocol);
                }
                idx += topic_len;
                if qos > 0 {
                    if payload.len() < idx + 2 {
                        return Err(MqttError::Protocol);
                    }
                    let pid = u16::from_be_bytes([payload[idx], payload[idx + 1]]);
                    idx += 2;
                    if qos == 1 {
                        self.send_puback(pid).await?;
                    }
                }
                let body = &payload[idx..];
                let n = body.len().min(out.len());
                out[..n].copy_from_slice(&body[..n]);
                return Ok(n);
            }
        }
    }

    async fn send_puback(&mut self, packet_id: u16) -> Result<(), MqttError> {
        let pkt = [
            0x40,
            0x02,
            (packet_id >> 8) as u8,
            (packet_id & 0xff) as u8,
        ];
        write_all(&mut self.transport, &pkt).await
    }

    async fn maybe_puback(&mut self, ty: u8, payload: &[u8]) -> Result<(), MqttError> {
        let qos = (ty >> 1) & 0x03;
        if qos != 1 || payload.len() < 4 {
            return Ok(());
        }
        let topic_len = u16::from_be_bytes([payload[0], payload[1]]) as usize;
        let pid_at = 2 + topic_len;
        if payload.len() < pid_at + 2 {
            return Ok(());
        }
        let pid = u16::from_be_bytes([payload[pid_at], payload[pid_at + 1]]);
        self.send_puback(pid).await
    }

    async fn read_packet(&mut self) -> Result<(u8, Vec<u8, 192>), MqttError> {
        let first = self.read_u8().await?;
        let mut multiplier = 1u32;
        let mut remaining = 0u32;
        loop {
            let byte = self.read_u8().await?;
            remaining += u32::from(byte & 0x7f) * multiplier;
            if byte & 0x80 == 0 {
                break;
            }
            multiplier = multiplier.saturating_mul(128);
            if multiplier > 128 * 128 * 128 {
                return Err(MqttError::Protocol);
            }
        }
        if remaining > 192 {
            return Err(MqttError::Buffer);
        }
        let mut payload: Vec<u8, 192> = Vec::new();
        for _ in 0..remaining {
            payload
                .push(self.read_u8().await?)
                .map_err(|_| MqttError::Buffer)?;
        }
        Ok((first, payload))
    }

    async fn read_u8(&mut self) -> Result<u8, MqttError> {
        while self.read_len == 0 {
            let n = self
                .transport
                .read(&mut self.read_buf)
                .await
                .map_err(|_| MqttError::Io)?;
            if n == 0 {
                return Err(MqttError::Io);
            }
            self.read_len = n;
        }
        let b = self.read_buf[0];
        if self.read_len == 1 {
            self.read_len = 0;
        } else {
            self.read_buf.copy_within(1..self.read_len, 0);
            self.read_len -= 1;
        }
        Ok(b)
    }
}

async fn write_all<T: Write>(transport: &mut T, buf: &[u8]) -> Result<(), MqttError> {
    let mut offset = 0;
    while offset < buf.len() {
        let n = transport
            .write(&buf[offset..])
            .await
            .map_err(|_| MqttError::Io)?;
        if n == 0 {
            return Err(MqttError::Io);
        }
        offset += n;
    }
    Ok(())
}

fn push_mqtt_string<const N: usize>(buf: &mut Vec<u8, N>, s: &str) -> Result<(), MqttError> {
    let bytes = s.as_bytes();
    if bytes.len() > 0xffff {
        return Err(MqttError::Buffer);
    }
    buf.push((bytes.len() >> 8) as u8)
        .map_err(|_| MqttError::Buffer)?;
    buf.push((bytes.len() & 0xff) as u8)
        .map_err(|_| MqttError::Buffer)?;
    for b in bytes {
        buf.push(*b).map_err(|_| MqttError::Buffer)?;
    }
    Ok(())
}

fn encode_packet<const N: usize>(
    out: &mut Vec<u8, N>,
    fixed_type_flags: u8,
    variable: &[u8],
) -> Result<(), MqttError> {
    out.clear();
    out.push(fixed_type_flags).map_err(|_| MqttError::Buffer)?;
    let mut x = variable.len();
    loop {
        let mut encoded = (x % 128) as u8;
        x /= 128;
        if x > 0 {
            encoded |= 0x80;
        }
        out.push(encoded).map_err(|_| MqttError::Buffer)?;
        if x == 0 {
            break;
        }
    }
    for b in variable {
        out.push(*b).map_err(|_| MqttError::Buffer)?;
    }
    Ok(())
}

/// Build `{prefix}/active_state` into `out`.
pub fn active_state_topic(prefix: &str, out: &mut heapless::String<64>) -> Result<(), MqttError> {
    out.clear();
    out.push_str(prefix).map_err(|_| MqttError::Buffer)?;
    out.push_str("/active_state")
        .map_err(|_| MqttError::Buffer)?;
    Ok(())
}
