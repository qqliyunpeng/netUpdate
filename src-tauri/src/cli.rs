//! Headless command-line interface.
//!
//! It shares the protocol, FTP and configuration code with the GUI. The final
//! result goes to stdout as one JSON object with `--json`, while progress and
//! diagnostics go to stderr so an AI tool can parse stdout safely.

use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand};
use serde_json::{json, Value};

use crate::config::{self, AppConfig};
use crate::{ftp, upgrade};

#[derive(Debug, Parser)]
#[command(
    name = "netupdate-cli",
    version,
    about = "AQ300 network update command line tool",
    arg_required_else_help = true
)]
struct Cli {
    /// Override the application data directory (session.json and tmp/).
    #[arg(long, global = true, value_name = "DIR")]
    data_dir: Option<PathBuf>,

    /// Print the final result as one JSON object on stdout.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Send a PAK upgrade package to an AQ300 device.
    Upgrade(UpgradeArgs),

    /// Download UPDATE_AQ300.PAK from the FTP server.
    Download(DownloadArgs),

    /// Show the effective configuration (the FTP password is redacted).
    Config,
}

#[derive(Debug, Args)]
struct UpgradeArgs {
    /// Device IP. Defaults to the session configuration.
    #[arg(long)]
    ip: Option<String>,

    /// Local PAK file or directory. A directory gets UPDATE_AQ300.PAK appended.
    #[arg(long, value_name = "PATH", conflicts_with = "ftp")]
    file: Option<PathBuf>,

    /// Download from FTP first, then send the downloaded package.
    #[arg(long, conflicts_with = "file")]
    ftp: bool,

    /// Validate the inputs without opening a TCP connection to the device.
    #[arg(long)]
    dry_run: bool,

    #[command(flatten)]
    ftp_options: FtpArgs,
}

#[derive(Debug, Args)]
struct DownloadArgs {
    /// Directory that receives UPDATE_AQ300.PAK. Defaults to <data-dir>/tmp.
    #[arg(long, value_name = "DIR")]
    output: Option<PathBuf>,

    #[command(flatten)]
    ftp_options: FtpArgs,
}

#[derive(Debug, Args)]
struct FtpArgs {
    /// FTP host. Defaults to the session configuration.
    #[arg(long = "ftp-host", value_name = "HOST")]
    host: Option<String>,

    /// FTP user name. Defaults to the session configuration.
    #[arg(long = "ftp-user", alias = "ftp-username", value_name = "USER")]
    username: Option<String>,

    /// FTP password. Prefer NETUPDATE_FTP_PASSWORD to keep it out of shell history.
    #[arg(long = "ftp-password", value_name = "PASSWORD")]
    password: Option<String>,

    /// Remote FTP directory. Defaults to the session configuration.
    #[arg(long = "ftp-folder", value_name = "DIR")]
    folder: Option<String>,
}

impl FtpArgs {
    fn apply(&self, config: &mut AppConfig) {
        if let Some(host) = &self.host {
            config.ftp_host = host.clone();
        }
        if let Some(username) = &self.username {
            config.ftp_username = username.clone();
        }
        if let Some(password) = self
            .password
            .clone()
            .or_else(|| std::env::var("NETUPDATE_FTP_PASSWORD").ok())
            .filter(|value| !value.is_empty())
        {
            config.ftp_password = password;
        }
        if let Some(folder) = &self.folder {
            config.ftp_folder = folder.clone();
        }
    }
}

pub fn run() -> i32 {
    let cli = Cli::parse();
    let operation = operation_name(&cli.command);

    match dispatch(&cli) {
        Ok(result) => {
            print_result(&cli, result);
            0
        }
        Err(error) => {
            print_error(&cli, operation, &error);
            1
        }
    }
}

fn dispatch(cli: &Cli) -> Result<Value, String> {
    let data_dir = match &cli.data_dir {
        Some(directory) => directory.clone(),
        None => config::default_data_dir()?,
    };
    let mut settings = config::load_from_dir(&data_dir).config;

    match &cli.command {
        Command::Config => Ok(json!({
            "ok": true,
            "operation": "config",
            "dataDir": data_dir.to_string_lossy(),
            "sessionFile": config::session_path_in(&data_dir).to_string_lossy(),
            "config": config_payload(&settings),
            "message": "配置已读取"
        })),
        Command::Download(args) => {
            args.ftp_options.apply(&mut settings);
            let output_dir = match &args.output {
                Some(directory) => directory.clone(),
                None => config::tmp_dir_in(&data_dir)?,
            };
            let path = ftp::download_to_dir(&settings, &output_dir, &mut log_to_stderr)?;
            let path_text = path.to_string_lossy().to_string();
            Ok(json!({
                "ok": true,
                "operation": "download",
                "file": path_text,
                "message": format!("已下载: {path_text}"),
                "config": config_payload(&settings)
            }))
        }
        Command::Upgrade(args) => {
            if let Some(ip) = &args.ip {
                settings.ip = ip.clone();
            }
            args.ftp_options.apply(&mut settings);

            let ip = settings.ip.trim().to_string();
            if ip.is_empty() {
                return Err("主机 IP 为空".to_string());
            }

            if !args.dry_run {
                upgrade::check_reachable(&ip, &mut log_to_stderr)?;
            }

            let path = if args.ftp {
                let download_dir = config::tmp_dir_in(&data_dir)?;
                ftp::download_to_dir(&settings, &download_dir, &mut log_to_stderr)?
            } else if let Some(file) = &args.file {
                package_path(file)
            } else {
                config::resolve_pak_path_in(&data_dir, &settings)?
            };

            if !path.is_file() {
                return Err(format!("升级文件不存在: {}", path.display()));
            }
            let size = std::fs::metadata(&path)
                .map_err(|error| format!("无法读取升级文件 {}: {error}", path.display()))?
                .len();
            if size == 0 {
                return Err("升级包为空".to_string());
            }

            let path_text = path.to_string_lossy().to_string();
            if args.dry_run {
                return Ok(json!({
                    "ok": true,
                    "operation": "upgrade",
                    "dryRun": true,
                    "ip": ip,
                    "file": path_text,
                    "bytes": size,
                    "message": format!("校验通过: {path_text}")
                }));
            }

            eprintln!("使用文件: {path_text}");
            let mut last_decile = -1_i32;
            let sent = upgrade::send_pak(
                &path,
                &ip,
                &mut log_to_stderr,
                &mut |bytes, total, percent| {
                    let decile = (percent / 10.0).floor() as i32;
                    if decile != last_decile {
                        last_decile = decile;
                        eprintln!("进度: {percent:.0}% ({bytes}/{total})");
                    }
                },
            )?;

            Ok(json!({
                "ok": true,
                "operation": "upgrade",
                "ip": ip,
                "file": path_text,
                "bytes": sent,
                "message": format!("升级成功: {path_text}")
            }))
        }
    }
}

fn package_path(path: &Path) -> PathBuf {
    if path.is_dir() {
        path.join(config::DEFAULT_PAK_NAME)
    } else {
        path.to_path_buf()
    }
}

fn config_payload(config: &AppConfig) -> Value {
    json!({
        "ip": config.ip,
        "filePath": config.file_path,
        "ftpHost": config.ftp_host,
        "ftpUsername": config.ftp_username,
        "ftpPasswordSet": !config.ftp_password.is_empty(),
        "ftpFolder": config.ftp_folder,
        "useLocal": config.use_local,
        "theme": config.theme
    })
}

fn log_to_stderr(message: &str) {
    eprintln!("{message}");
}

fn operation_name(command: &Command) -> &'static str {
    match command {
        Command::Upgrade(_) => "upgrade",
        Command::Download(_) => "download",
        Command::Config => "config",
    }
}

fn print_result(cli: &Cli, result: Value) {
    if cli.json {
        println!("{result}");
        return;
    }
    if let Some(message) = result.get("message").and_then(Value::as_str) {
        println!("{message}");
    }
    if result.get("operation").and_then(Value::as_str) == Some("config") {
        if let Some(config) = result.get("config") {
            println!(
                "{}",
                serde_json::to_string_pretty(config).unwrap_or_else(|_| config.to_string())
            );
        }
    }
}

fn print_error(cli: &Cli, operation: &str, error: &str) {
    if cli.json {
        println!(
            "{}",
            json!({ "ok": false, "operation": operation, "error": error })
        );
    } else {
        eprintln!("错误: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_directory_resolves_to_the_default_package_name() {
        let directory = std::env::temp_dir();
        assert_eq!(
            package_path(&directory),
            directory.join(config::DEFAULT_PAK_NAME)
        );
    }

    #[test]
    fn a_package_path_is_kept_as_is() {
        let file = PathBuf::from("C:/images/UPDATE_AQ300.PAK");
        assert_eq!(package_path(&file), file);
    }

    #[test]
    fn config_json_never_exposes_the_password() {
        let mut config = AppConfig::default();
        config.ftp_password = "super-secret".to_string();
        let value = config_payload(&config);
        assert_eq!(value.get("ftpPasswordSet"), Some(&Value::Bool(true)));
        assert!(value.get("ftpPassword").is_none());
        assert!(!value.to_string().contains("super-secret"));
    }
}
