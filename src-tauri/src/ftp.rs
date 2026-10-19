//! FTP download used when the package is not available on a local disk.

use std::fs::File;
use std::io::Write;
use std::net::ToSocketAddrs;
use std::path::Path;
use std::time::Duration;

use suppaftp::{FtpError, FtpStream};
use tauri::AppHandle;

use crate::config::{self, AppConfig};
use crate::upgrade::log;

const FTP_CONNECT_TIMEOUT_S: u64 = 15;
const FTP_PORT: u16 = 21;

pub async fn download(app: AppHandle, config: AppConfig) -> Result<String, String> {
    let local_dir = config::tmp_dir(&app)?;
    let local_path = local_dir.join(config::DEFAULT_PAK_NAME);
    let background_app = app.clone();
    let background_path = local_path.clone();

    tauri::async_runtime::spawn_blocking(move || {
        download_blocking(&background_app, &config, &background_path)
    })
    .await
    .map_err(|error| format!("FTP 线程异常: {error}"))??;

    Ok(local_path.to_string_lossy().to_string())
}

fn download_blocking(app: &AppHandle, config: &AppConfig, local_path: &Path) -> Result<(), String> {
    if config.ftp_host.trim().is_empty() {
        return Err("FTP 主机地址为空".to_string());
    }

    let host = config.ftp_host.trim();
    log(app, format!("FTP: connecting to {host} ..."));
    let mut ftp = connect(host)?;

    let login = ftp
        .login(config.ftp_username.as_str(), config.ftp_password.as_str())
        .map_err(|error| format!("FTP 登录失败: {error}"));
    if let Err(error) = login {
        let _ = ftp.quit();
        return Err(error);
    }
    log(app, "FTP: login ok".to_string());

    let folder = config.ftp_folder.trim();
    if !folder.is_empty() && folder != "/" {
        ftp.cwd(folder)
            .map_err(|error| format!("FTP 切换目录失败: {error}"))?;
    }
    log(
        app,
        format!("FTP: downloading {} ...", config::DEFAULT_PAK_NAME),
    );

    let mut file = File::create(local_path)
        .map_err(|error| format!("无法创建本地文件 {}: {error}", local_path.display()))?;
    let mut downloaded = 0_u64;
    let mut next_report = 16_u64 * 1024 * 1024;

    let result = ftp.retr(config::DEFAULT_PAK_NAME, |reader| {
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = reader
                .read(&mut buffer)
                .map_err(FtpError::ConnectionError)?;
            if read == 0 {
                break;
            }
            file.write_all(&buffer[..read])
                .map_err(FtpError::ConnectionError)?;
            downloaded += read as u64;
            if downloaded >= next_report {
                log(
                    app,
                    format!("FTP: 已下载 {:.1} MiB", downloaded as f64 / 1_048_576.0),
                );
                next_report += 16_u64 * 1024 * 1024;
            }
        }
        Ok(())
    });

    if let Err(error) = result {
        let _ = ftp.quit();
        let _ = std::fs::remove_file(local_path);
        return Err(format!("FTP 下载失败: {error}"));
    }

    let _ = ftp.quit();
    log(app, format!("FTP: saved to {}", local_path.display()));
    Ok(())
}

fn connect(host: &str) -> Result<FtpStream, String> {
    let addresses = (host, FTP_PORT)
        .to_socket_addrs()
        .map_err(|error| format!("无法解析 FTP 主机 {host}: {error}"))?
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        return Err(format!("无法解析 FTP 主机 {host}"));
    }

    let mut last_error = String::new();
    for address in addresses {
        match FtpStream::connect_timeout(address, Duration::from_secs(FTP_CONNECT_TIMEOUT_S)) {
            Ok(ftp) => return Ok(ftp),
            Err(error) => last_error = error.to_string(),
        }
    }
    Err(format!("无法连接 FTP 服务器: {last_error}"))
}
