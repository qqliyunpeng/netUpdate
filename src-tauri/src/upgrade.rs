//! AQ300 update protocol.
//!
//! The device accepts a little-endian 32-byte header followed by a chunk of the
//! PAK file on TCP port 9000. After the last chunk it answers with a 36-byte
//! response whose final `u32` is the status.

use std::fs::File;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::config::{self, AppConfig};

pub const TCP_PORT: u16 = 9000;
pub const TCP_CONNECT_TIMEOUT_S: u64 = 12;
pub const TCP_CONNECT_ATTEMPTS: usize = 3;
pub const TCP_RECV_TIMEOUT_S: u64 = 120;
pub const PACKET_SIZE: usize = 4096;
pub const HEAD_SIZE: usize = 32;
pub const RESP_SIZE: usize = 36;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressUpdate {
    pub bytes: u64,
    pub total: u64,
    pub percent: f64,
}

pub async fn run(app: AppHandle, config: AppConfig) -> Result<String, String> {
    let path = config::resolve_pak_path(&app, &config)?;
    if !path.is_file() {
        let message = format!("升级失败: 文件不存在 {}", path.display());
        log(&app, message.clone());
        return Err(message);
    }

    let ip = config.ip.trim().to_string();
    if ip.is_empty() {
        let message = "升级失败: 主机 IP 为空".to_string();
        log(&app, message.clone());
        return Err(message);
    }

    log(&app, format!("使用文件: {}", path.display()));
    let display_path = path.to_string_lossy().to_string();
    let background_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || send_pak(&background_app, &path, &ip))
        .await
        .map_err(|error| format!("升级线程异常: {error}"))??;
    Ok(display_path)
}

fn connect(app: &AppHandle, ip: &str) -> Result<TcpStream, String> {
    let addresses = (ip, TCP_PORT)
        .to_socket_addrs()
        .map_err(|error| format!("无法解析主机地址 {ip}: {error}"))?
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        return Err(format!("无法解析主机地址 {ip}"));
    }

    let mut last_error = String::new();
    for attempt in 1..=TCP_CONNECT_ATTEMPTS {
        for address in &addresses {
            match TcpStream::connect_timeout(address, Duration::from_secs(TCP_CONNECT_TIMEOUT_S)) {
                Ok(stream) => {
                    let _ = stream.set_nodelay(true);
                    let _ = stream.set_write_timeout(Some(Duration::from_secs(TCP_RECV_TIMEOUT_S)));
                    let _ =
                        stream.set_read_timeout(Some(Duration::from_secs(TCP_CONNECT_TIMEOUT_S)));
                    log(app, format!("已连接到服务器: {ip}:{TCP_PORT}"));
                    return Ok(stream);
                }
                Err(error) => {
                    last_error = error.to_string();
                    log(app, format!("连接出错: {error} 尝试第 {attempt} 次..."));
                }
            }
        }
    }

    let message = format!("无法连接到服务器，已尝试 {TCP_CONNECT_ATTEMPTS} 次: {last_error}");
    log(app, message.clone());
    Err(message)
}

fn send_pak(app: &AppHandle, path: &Path, ip: &str) -> Result<(), String> {
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
    let mut stream = connect(app, ip)?;
    let mut buffer = vec![0_u8; PACKET_SIZE - HEAD_SIZE];
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

        let header = build_header(total, read, sequence);
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
            emit_progress(app, sent, total, percent);
        }
    }

    if sent != total {
        return Err(format!("未能完整发送文件（{sent}/{total} 字节）"));
    }

    log(app, "文件已发送完毕".to_string());
    log(app, "等待更新完成标志...".to_string());
    stream
        .set_read_timeout(Some(Duration::from_secs(TCP_RECV_TIMEOUT_S)))
        .map_err(|error| format!("设置接收超时失败: {error}"))?;

    let mut response = [0_u8; RESP_SIZE];
    stream
        .read_exact(&mut response)
        .map_err(|error| format!("等待设备响应失败: {error}"))?;
    let status = response_status(&response);
    if status == 0 {
        log(app, "升级成功".to_string());
        Ok(())
    } else {
        let message = format!("升级失败 (code={status})");
        log(app, message.clone());
        Err(message)
    }
}

fn build_header(total: u64, chunk_len: usize, sequence: u32) -> [u8; HEAD_SIZE] {
    let mut header = [0_u8; HEAD_SIZE];
    header[0..2].copy_from_slice(&0x9000_u16.to_le_bytes());
    header[2..4].copy_from_slice(&0_u16.to_le_bytes());
    header[4..8].copy_from_slice(&(total as u32).to_le_bytes());
    header[8..12].copy_from_slice(&(chunk_len as u32).to_le_bytes());
    header[12..16].copy_from_slice(&sequence.to_le_bytes());
    header[16..18].copy_from_slice(&2_u16.to_le_bytes());
    header[18..20].copy_from_slice(&0_u16.to_le_bytes());
    header[20..24].copy_from_slice(&0_u32.to_le_bytes());
    header[24..32].copy_from_slice(&0_u64.to_le_bytes());
    header
}

fn response_status(response: &[u8; RESP_SIZE]) -> u32 {
    u32::from_le_bytes(response[32..36].try_into().expect("status slice"))
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
        let header = build_header(0x11_22_33_44, 4064, 7);
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
        assert_eq!(response_status(&response), 42);
    }
}
