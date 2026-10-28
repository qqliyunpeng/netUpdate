//! AQ300 update protocol.
//!
//! The device accepts a little-endian 32-byte header followed by a chunk of the
//! PAK file on TCP port 9000. After the last chunk it answers with a 36-byte
//! response whose final `u32` is the status.

use std::fs::File;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::config::{self, AppConfig};

pub const TCP_PORT: u16 = 9000;
pub const TCP_CONNECT_TIMEOUT_S: u64 = 12;
pub const TCP_CONNECT_ATTEMPTS: u32 = 3;
pub const TCP_RECV_TIMEOUT_S: u64 = 120;
pub const PACKET_SIZE: u32 = 4096;
pub const HEAD_SIZE: usize = 32;
pub const RESP_SIZE: usize = 36;
pub const MAGIC: u16 = 0x9000;
pub const PACKET_TYPE: u16 = 2;
pub const STATUS_OFFSET: usize = 32;
pub const MIN_PACKET_SIZE: u32 = HEAD_SIZE as u32 + 32;
pub const MAX_PACKET_SIZE: u32 = 1_048_576;
pub const MAX_CONNECT_TIMEOUT_S: u64 = 600;
pub const MAX_CONNECT_ATTEMPTS: u32 = 10;
pub const MAX_RECV_TIMEOUT_S: u64 = 3600;
const CONNECT_RETRY_DELAY_MS: u64 = 500;
const CONNECT_FAST_TIMEOUT_MS: u64 = 2000;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressUpdate {
    pub bytes: u64,
    pub total: u64,
    pub percent: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProtocolSettings {
    pub port: u16,
    pub packet_size: u32,
    pub magic: u16,
    pub packet_type: u16,
    pub status_offset: u32,
    pub connect_timeout_s: u64,
    pub connect_attempts: u32,
    pub recv_timeout_s: u64,
}

impl Default for ProtocolSettings {
    fn default() -> Self {
        Self {
            port: TCP_PORT,
            packet_size: PACKET_SIZE,
            magic: MAGIC,
            packet_type: PACKET_TYPE,
            status_offset: STATUS_OFFSET as u32,
            connect_timeout_s: TCP_CONNECT_TIMEOUT_S,
            connect_attempts: TCP_CONNECT_ATTEMPTS,
            recv_timeout_s: TCP_RECV_TIMEOUT_S,
        }
    }
}

impl ProtocolSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.port == 0 {
            return Err("协议端口必须在 1-65535 之间".to_string());
        }
        if !(MIN_PACKET_SIZE..=MAX_PACKET_SIZE).contains(&self.packet_size) {
            return Err(format!(
                "分包大小必须在 {MIN_PACKET_SIZE}-{MAX_PACKET_SIZE} 字节之间"
            ));
        }
        if self.connect_timeout_s == 0 || self.connect_timeout_s > MAX_CONNECT_TIMEOUT_S {
            return Err(format!("连接超时必须在 1-{MAX_CONNECT_TIMEOUT_S} 秒之间"));
        }
        if self.connect_attempts == 0 || self.connect_attempts > MAX_CONNECT_ATTEMPTS {
            return Err(format!("连接重试次数必须在 1-{MAX_CONNECT_ATTEMPTS} 之间"));
        }
        if self.recv_timeout_s == 0 || self.recv_timeout_s > MAX_RECV_TIMEOUT_S {
            return Err(format!("响应超时必须在 1-{MAX_RECV_TIMEOUT_S} 秒之间"));
        }
        if self.status_offset as usize + 4 > RESP_SIZE {
            return Err(format!("状态码偏移必须在 0-{} 之间", RESP_SIZE - 4));
        }
        Ok(())
    }

    pub fn payload_size(&self) -> usize {
        self.packet_size.saturating_sub(HEAD_SIZE as u32) as usize
    }
}

pub async fn run(app: AppHandle, config: AppConfig) -> Result<String, String> {
    let ip = config.ip.trim().to_string();
    if ip.is_empty() {
        let message = "升级失败: 主机 IP 为空".to_string();
        log(&app, message.clone());
        return Err(message);
    }

    if let Err(error) = config.protocol.validate() {
        let message = format!("升级失败: {error}");
        log(&app, message.clone());
        return Err(message);
    }
    let protocol = config.protocol.clone();

    let path = config::resolve_pak_path(&app, &config)?;
    if !path.is_file() {
        let message = format!("升级失败: 文件不存在 {}", path.display());
        log(&app, message.clone());
        return Err(message);
    }

    log(&app, format!("使用文件: {}", path.display()));
    let display_path = path.to_string_lossy().to_string();
    let background_app = app.clone();
    // The upgrade connection doubles as the reachability check. A separate
    // probe would close a connection first, and single-connection devices can
    // then reject the real upgrade connection until their listener recovers.
    tauri::async_runtime::spawn_blocking(move || {
        send_pak(
            &path,
            &ip,
            &protocol,
            &mut |message| log(&background_app, message.to_string()),
            &mut |bytes, total, percent| emit_progress(&background_app, bytes, total, percent),
        )
    })
    .await
    .map_err(|error| format!("升级线程异常: {error}"))??;
    Ok(display_path)
}

fn connect<L>(ip: &str, protocol: &ProtocolSettings, log: &mut L) -> Result<TcpStream, String>
where
    L: FnMut(&str),
{
    let addresses = (ip, protocol.port)
        .to_socket_addrs()
        .map_err(|error| format!("无法解析主机地址 {ip}: {error}"))?
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        return Err(format!("无法解析主机地址 {ip}"));
    }

    let attempts = protocol.connect_attempts;
    let mut last_error = String::new();
    for attempt in 1..=attempts {
        let timeout = attempt_timeout(protocol, attempt);
        let started = Instant::now();
        log(&format!(
            "连接尝试 {attempt}/{attempts}（超时 {:.1} 秒）...",
            timeout.as_secs_f64()
        ));
        for address in &addresses {
            match TcpStream::connect_timeout(address, timeout) {
                Ok(stream) => {
                    let _ = stream.set_nodelay(true);
                    let _ = stream
                        .set_write_timeout(Some(Duration::from_secs(protocol.recv_timeout_s)));
                    let _ = stream
                        .set_read_timeout(Some(Duration::from_secs(protocol.connect_timeout_s)));
                    log(&format!(
                        "已连接到服务器: {ip}:{}（{:.2} 秒）",
                        protocol.port,
                        started.elapsed().as_secs_f64()
                    ));
                    return Ok(stream);
                }
                Err(error) => {
                    last_error = error.to_string();
                    log(&format!(
                        "连接出错: {error}（{:.2} 秒）尝试第 {attempt} 次...",
                        started.elapsed().as_secs_f64()
                    ));
                }
            }
        }
        if attempt < attempts {
            log(&format!("等待 {CONNECT_RETRY_DELAY_MS} ms 后重试..."));
            std::thread::sleep(Duration::from_millis(CONNECT_RETRY_DELAY_MS));
        }
    }

    let message = format!("无法连接到服务器，已尝试 {} 次: {last_error}", attempts);
    log(&message);
    Err(message)
}

/// Earlier retries use a short timeout so a device that is rebooting after an
/// update does not block the whole first attempt. The final attempt always uses
/// the configured timeout, which keeps slow-but-reachable networks working.
fn attempt_timeout(protocol: &ProtocolSettings, attempt: u32) -> Duration {
    let full = Duration::from_secs(protocol.connect_timeout_s);
    if attempt >= protocol.connect_attempts {
        return full;
    }
    let exponent = attempt.saturating_sub(1).min(3);
    let fast_ms = CONNECT_FAST_TIMEOUT_MS.saturating_mul(1_u64 << exponent);
    Duration::from_millis(fast_ms).min(full)
}

/// Stream a PAK to the device. The GUI and CLI share this implementation; only
/// the log and progress sinks differ.
pub fn send_pak<L, P>(
    path: &Path,
    ip: &str,
    protocol: &ProtocolSettings,
    log: &mut L,
    progress: &mut P,
) -> Result<u64, String>
where
    L: FnMut(&str),
    P: FnMut(u64, u64, f64),
{
    protocol.validate()?;

    let total = std::fs::metadata(path)
        .map_err(|error| format!("无法读取文件信息: {error}"))?
        .len();
    if total == 0 {
        return Err("升级包为空".to_string());
    }
    if total > u64::from(u32::MAX) {
        return Err("升级包超过协议支持的 4 GiB".to_string());
    }

    let mut file = File::open(path).map_err(|error| format!("无法打开升级包: {error}"))?;
    log(&format!("升级包已打开（{total} 字节），正在连接设备..."));
    let mut stream = connect(ip, protocol, log)?;
    let mut buffer = vec![0_u8; protocol.payload_size()];
    let mut sent = 0_u64;
    let mut sequence = 0_u32;
    let mut last_percent = -1_i32;

    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("读取升级包失败: {error}"))?;
        if read == 0 {
            break;
        }

        let header = build_header(total, read, sequence, protocol.magic, protocol.packet_type);
        stream
            .write_all(&header)
            .and_then(|_| stream.write_all(&buffer[..read]))
            .map_err(|error| format!("发送升级包失败: {error}"))?;

        sent += read as u64;
        sequence = sequence.wrapping_add(1);

        let percent = (sent as f64 / total as f64 * 100.0).min(100.0);
        let bucket = percent.floor() as i32;
        if bucket != last_percent {
            last_percent = bucket;
            progress(sent, total, percent);
        }
    }

    if sent != total {
        return Err(format!("未能完整发送文件（{sent}/{total} 字节）"));
    }

    log("文件已发送完毕");
    log("等待更新完成标志...");
    stream
        .set_read_timeout(Some(Duration::from_secs(protocol.recv_timeout_s)))
        .map_err(|error| format!("设置接收超时失败: {error}"))?;

    let mut response = [0_u8; RESP_SIZE];
    stream
        .read_exact(&mut response)
        .map_err(|error| format!("等待设备响应失败: {error}"))?;
    let status = response_status(&response, protocol.status_offset as usize);
    if status == 0 {
        log("升级成功");
        Ok(sent)
    } else {
        let message = format!("升级失败 (code={status})");
        log(&message);
        Err(message)
    }
}

fn build_header(
    total: u64,
    chunk_len: usize,
    sequence: u32,
    magic: u16,
    packet_type: u16,
) -> [u8; HEAD_SIZE] {
    let mut header = [0_u8; HEAD_SIZE];
    header[0..2].copy_from_slice(&magic.to_le_bytes());
    header[2..4].copy_from_slice(&0_u16.to_le_bytes());
    header[4..8].copy_from_slice(&(total as u32).to_le_bytes());
    header[8..12].copy_from_slice(&(chunk_len as u32).to_le_bytes());
    header[12..16].copy_from_slice(&sequence.to_le_bytes());
    header[16..18].copy_from_slice(&packet_type.to_le_bytes());
    header[18..20].copy_from_slice(&0_u16.to_le_bytes());
    header[20..24].copy_from_slice(&0_u32.to_le_bytes());
    header[24..32].copy_from_slice(&0_u64.to_le_bytes());
    header
}

fn response_status(response: &[u8; RESP_SIZE], offset: usize) -> u32 {
    u32::from_le_bytes(
        response[offset..offset + 4]
            .try_into()
            .expect("status slice"),
    )
}

fn emit_progress(app: &AppHandle, bytes: u64, total: u64, percent: f64) {
    let _ = app.emit(
        "upgrade-progress",
        ProgressUpdate {
            bytes,
            total,
            percent,
        },
    );
}

pub fn log(app: &AppHandle, message: String) {
    let _ = app.emit("upgrade-log", message);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_matches_the_python_struct_layout() {
        let header = build_header(0x11_22_33_44, 4064, 7, 0x9000, 2);
        assert_eq!(header.len(), HEAD_SIZE);
        assert_eq!(u16::from_le_bytes(header[0..2].try_into().unwrap()), 0x9000);
        assert_eq!(
            u32::from_le_bytes(header[4..8].try_into().unwrap()),
            0x11_22_33_44
        );
        assert_eq!(u32::from_le_bytes(header[8..12].try_into().unwrap()), 4064);
        assert_eq!(u32::from_le_bytes(header[12..16].try_into().unwrap()), 7);
        assert_eq!(u16::from_le_bytes(header[16..18].try_into().unwrap()), 2);
    }

    #[test]
    fn response_status_is_the_final_u32() {
        let mut response = [0_u8; RESP_SIZE];
        response[32..36].copy_from_slice(&42_u32.to_le_bytes());
        assert_eq!(response_status(&response, STATUS_OFFSET), 42);
    }

    #[test]
    fn default_protocol_is_valid() {
        assert!(ProtocolSettings::default().validate().is_ok());
    }

    #[test]
    fn protocol_rejects_a_packet_without_payload_room() {
        let mut protocol = ProtocolSettings::default();
        protocol.packet_size = MIN_PACKET_SIZE - 1;
        assert!(protocol.validate().is_err());
    }

    #[test]
    fn protocol_rejects_a_status_offset_past_the_response() {
        let mut protocol = ProtocolSettings::default();
        protocol.status_offset = (RESP_SIZE - 3) as u32;
        assert!(protocol.validate().is_err());
    }

    #[test]
    fn configurable_header_fields_are_written() {
        let header = build_header(0, 0, 0, 0x1234, 7);
        assert_eq!(u16::from_le_bytes(header[0..2].try_into().unwrap()), 0x1234);
        assert_eq!(u16::from_le_bytes(header[16..18].try_into().unwrap()), 7);
    }

    #[test]
    fn a_legacy_config_gets_default_protocol_settings() {
        let config: AppConfig = serde_json::from_str(r#"{"ip":"192.168.1.10"}"#).unwrap();
        assert_eq!(config.protocol.packet_size, PACKET_SIZE);
        assert_eq!(config.protocol.port, TCP_PORT);
    }

    #[test]
    fn an_upgrade_opens_one_connection_and_reads_the_status() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("first connection");
            let mut request = [0_u8; HEAD_SIZE + 4];
            stream.read_exact(&mut request).expect("request");
            assert_eq!(&request[HEAD_SIZE..], b"PAK!");
            let response = [0_u8; RESP_SIZE];
            stream.write_all(&response).expect("response");
            std::thread::sleep(Duration::from_millis(50));
            listener.set_nonblocking(true).expect("nonblocking");
            assert!(
                listener.accept().is_err(),
                "an upgrade must not open a separate probe connection"
            );
        });

        let path = std::env::temp_dir().join(format!("netupdate-single-connection-{port}.pak"));
        std::fs::write(&path, b"PAK!").expect("write pak");
        let protocol = ProtocolSettings {
            port,
            connect_timeout_s: 1,
            connect_attempts: 1,
            recv_timeout_s: 5,
            ..ProtocolSettings::default()
        };

        let mut logs = Vec::new();
        let sent = send_pak(
            &path,
            "127.0.0.1",
            &protocol,
            &mut |message| logs.push(message.to_string()),
            &mut |_, _, _| {},
        )
        .expect("upgrade succeeds");
        let _ = std::fs::remove_file(&path);

        assert_eq!(sent, 4);
        server.join().expect("server thread");
        assert!(logs
            .iter()
            .any(|message| message.contains("已连接到服务器")));
    }
}
